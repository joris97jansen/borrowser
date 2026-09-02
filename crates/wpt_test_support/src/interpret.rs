use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use conformance_test_support::{
    CapabilityFeatureId, CapabilityRequirement, EngineCapabilityKind, EnvironmentProfileId,
    EnvironmentRequirementKind, GenericAssertionRequirement, GenericHarnessRequirement,
    GenericResourceRequirement, HarnessFeatureId, RequirementTag, ResourceProfileId,
    SourceEnvironmentRequirement, SourceRequirementsBuilder,
};
use external_test_provenance::UpstreamPath;

use crate::html_metadata::{
    HtmlElementMetadata, HtmlMetadataError, ParsedHtmlMetadata, parse_html_metadata,
};
use crate::model::{
    InterpretationEvidence, InterpretedWptRecord, WPT_MAX_REFERENCE_DEPTH, WPT_MAX_REFERENCE_EDGES,
    WPT_MAX_REFERENCE_NODES, WptAutomationRequirement, WptFuzzyMetadata,
    WptInterpretationLimitation, WptReadinessRequirement, WptReferenceEdge, WptReferenceGraph,
    WptReferenceRelation, WptResourceDetail, WptServerRequirement, WptSourceForm,
};
use crate::registry::{
    ValidatedWptSourceSet, WptRegistryError, WptSourceRecord, read_declared_file,
};
use crate::source_metadata::{
    ValidatedWptSourceMetadata, WptRecordMetadata, WptSourceMetadataError,
    validate_record_metadata_evidence,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WptInterpretationError {
    Registry(WptRegistryError),
    Html(HtmlMetadataError),
    UnsupportedAuthoredForm,
    MissingReference,
    ReferenceCycle,
    ReferenceDepthExceeded,
    ReferenceNodeLimitExceeded,
    ReferenceEdgeLimitExceeded,
    InvalidReferencePath,
    InvalidGenericProjection,
    SourceMetadata(WptSourceMetadataError),
}
impl From<WptRegistryError> for WptInterpretationError {
    fn from(value: WptRegistryError) -> Self {
        Self::Registry(value)
    }
}
impl From<HtmlMetadataError> for WptInterpretationError {
    fn from(value: HtmlMetadataError) -> Self {
        Self::Html(value)
    }
}
impl From<WptSourceMetadataError> for WptInterpretationError {
    fn from(value: WptSourceMetadataError) -> Self {
        Self::SourceMetadata(value)
    }
}

#[derive(Default)]
struct HtmlFacts {
    scripts: Vec<ScriptFact>,
    references: Vec<(WptReferenceRelation, String)>,
    fuzzy: Vec<String>,
    reftest_wait: bool,
    help_links: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ScriptFact {
    src: Option<String>,
    kind: ScriptKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ScriptKind {
    ClassicExecutable,
    ModuleExecutable,
    InertDataBlock,
    NotYetClassifiable,
}

pub fn interpret_wpt_source_set(
    repository_root: &Path,
    set: &ValidatedWptSourceSet,
    metadata: &ValidatedWptSourceMetadata,
) -> Result<Vec<InterpretedWptRecord>, WptInterpretationError> {
    let mut output = Vec::new();
    for record in set.records() {
        match interpret_wpt_record(repository_root, set, metadata, record) {
            Ok(value) => output.push(value),
            Err(WptInterpretationError::UnsupportedAuthoredForm) => {
                output.push(InterpretedWptRecord::not_yet_classifiable(
                    record.id().clone(),
                    InterpretationEvidence::new(
                        "unsupported-authored-form",
                        "integrity-verified source has no AG8-supported authored form",
                    ),
                ))
            }
            Err(WptInterpretationError::Html(error)) => {
                output.push(InterpretedWptRecord::malformed(
                    record.id().clone(),
                    InterpretationEvidence::new(
                        "unimportable-html-source",
                        format!("bounded HTML metadata interpretation failed: {error:?}"),
                    ),
                ))
            }
            Err(error) => return Err(error),
        }
    }
    output.sort_by(|a, b| a.source_record_id().cmp(b.source_record_id()));
    Ok(output)
}

pub fn interpret_wpt_record(
    repository_root: &Path,
    set: &ValidatedWptSourceSet,
    metadata: &ValidatedWptSourceMetadata,
    record: &WptSourceRecord,
) -> Result<InterpretedWptRecord, WptInterpretationError> {
    let source = set
        .file_by_id(record.source_file_id())
        .ok_or(WptInterpretationError::UnsupportedAuthoredForm)?;
    let path = source.identity().path();
    let bytes = read_declared_file(repository_root, source)?;
    let record_metadata =
        metadata
            .record(record.id())
            .ok_or(WptInterpretationError::SourceMetadata(
                WptSourceMetadataError::PopulationMismatch,
            ))?;
    if is_wdspec_path(path) {
        return interpret_wdspec(record, source, record_metadata);
    }
    if !path.as_str().ends_with(".html") {
        return Err(WptInterpretationError::UnsupportedAuthoredForm);
    }
    let metadata = parse_html_metadata(&bytes)?;
    let facts = html_facts(&metadata);
    let source_form = if facts
        .scripts
        .iter()
        .any(|value| value.is_executable_resource("testharness.js"))
    {
        WptSourceForm::TestHarness
    } else if !facts.references.is_empty() {
        WptSourceForm::Reftest
    } else {
        deferred_html_form(path)
    };
    let graph_result = if source_form == WptSourceForm::Reftest {
        Some(build_reference_graph(repository_root, set, path, &facts))
    } else {
        None
    };
    let mut builder = SourceRequirementsBuilder::new();
    let mut automation = Vec::new();
    let mut readiness = Vec::new();
    let mut servers = Vec::new();
    let mut resources = Vec::new();
    let mut evidence = vec![
        InterpretationEvidence::new("source-path", path.as_str()),
        InterpretationEvidence::new("source-sha256", source.identity().sha256().to_hex()),
    ];
    let has_script = facts.scripts.iter().any(ScriptFact::is_executable);
    if has_script {
        builder
            .requirement_tag(RequirementTag::RequiresJs)
            .capability(
                CapabilityRequirement::new(EngineCapabilityKind::JavaScriptExecution, None)
                    .map_err(|_| WptInterpretationError::InvalidGenericProjection)?,
            );
    }
    if source_form == WptSourceForm::TestHarness {
        automation.push(WptAutomationRequirement::TestHarnessJavascript);
        builder.harness(GenericHarnessRequirement::SubsystemAdapter(
            harness_feature("testharness-execution")?,
        ));
        evidence.push(InterpretationEvidence::new(
            "authored-script",
            "/resources/testharness.js",
        ));
    }
    if matches!(
        source_form,
        WptSourceForm::Manual
            | WptSourceForm::Visual
            | WptSourceForm::CrashTest
            | WptSourceForm::PrintReftest
    ) {
        builder.harness(GenericHarnessRequirement::SourceFormatInterpreter(
            harness_feature(source_form.as_str().trim_end_matches("-deferred"))?,
        ));
    }
    if source_form == WptSourceForm::NotYetClassifiable {
        builder.harness(GenericHarnessRequirement::SourceFormatInterpreter(
            harness_feature("wpt-authored-form-classification")?,
        ));
    }
    if facts
        .scripts
        .iter()
        .any(|value| value.is_executable_resource("testdriver.js"))
    {
        automation.push(WptAutomationRequirement::TestDriver);
        builder.harness(GenericHarnessRequirement::SubsystemAdapter(
            harness_feature("testdriver-automation")?,
        ));
    }
    if facts
        .scripts
        .iter()
        .any(|script| script.kind == ScriptKind::NotYetClassifiable)
    {
        readiness.push(WptReadinessRequirement::UnclassifiedScriptType);
        builder.harness(GenericHarnessRequirement::SourceFormatInterpreter(
            harness_feature("html-script-type-classification")?,
        ));
    }
    if source_form == WptSourceForm::Reftest {
        builder
            .requirement_tag(RequirementTag::RequiresPixelComparison)
            .harness(GenericHarnessRequirement::ComparisonSurface(
                harness_feature("wpt-raster-comparison")?,
            ))
            .environment(SourceEnvironmentRequirement::new(
                EnvironmentRequirementKind::PixelCaptureEnvironment,
                environment_profile("deterministic-raster")?,
            ))
            .assertion(GenericAssertionRequirement::RasterComparison);
        if facts.references.len() > 1 {
            builder.assertion(GenericAssertionRequirement::MultipleReferenceAssertion);
        }
        if graph_result.as_ref().is_some_and(|result| {
            result
                .as_ref()
                .is_err_and(|error| reference_limitation(error).is_some())
        }) {
            builder.harness(GenericHarnessRequirement::SourceFormatInterpreter(
                harness_feature("bounded-wpt-reference-graph")?,
            ));
        }
    }
    if facts.reftest_wait {
        readiness.push(WptReadinessRequirement::ReftestWait);
        builder.assertion(GenericAssertionRequirement::DynamicReadiness);
        evidence.push(InterpretationEvidence::new("html-class", "reftest-wait"));
    }
    if path.as_str().ends_with(".sub.html") {
        servers.push(WptServerRequirement::Substitution);
        builder.resource(GenericResourceRequirement::ServerBehavior {
            profile: resource_profile("wpt-substitution")?,
        });
        resources.push(WptResourceDetail::WptServer);
        evidence.push(InterpretationEvidence::new(
            "wpt-filename-flag",
            ".sub.html",
        ));
    }
    let closure = set
        .files()
        .iter()
        .filter(|file| file.parents().contains(record.id()) && file.id() != source.id())
        .collect::<Vec<_>>();
    if !closure.is_empty() {
        let closure_id = resource_profile(record.id().as_str())?;
        builder.resource(GenericResourceRequirement::PinnedLocalStatic {
            closure: closure_id,
        });
        for file in closure {
            resources.push(WptResourceDetail::PinnedLocal(
                file.identity().path().clone(),
            ));
        }
    }
    if resources.is_empty() {
        if source_form == WptSourceForm::TestHarness {
            builder.resource(GenericResourceRequirement::ServerBehavior {
                profile: resource_profile("wpt-harness-resources")?,
            });
            resources.push(WptResourceDetail::WptServer);
        } else {
            builder.resource(GenericResourceRequirement::SelfContained);
            resources.push(WptResourceDetail::SelfContained);
        }
    }
    for link in &facts.help_links {
        evidence.push(InterpretationEvidence::new("help-link", link));
    }
    validate_record_metadata_evidence(record_metadata, &evidence)?;
    if has_script && record_metadata.no_js().is_some() {
        return Err(WptInterpretationError::SourceMetadata(
            WptSourceMetadataError::ContradictoryNoJs,
        ));
    }
    let feature_areas =
        apply_source_metadata(record_metadata, &mut builder, &mut servers, &mut resources)?;
    let requirements = builder
        .build()
        .map_err(|_| WptInterpretationError::InvalidGenericProjection)?;
    let (graph, limitation) = match graph_result {
        None => (None, None),
        Some(Ok(graph)) => (Some(graph), None),
        Some(Err(error)) => match reference_limitation(&error) {
            Some(limitation) => (None, Some(limitation)),
            None => return Err(error),
        },
    };
    let mut interpreted = InterpretedWptRecord::new(
        record.id().clone(),
        source_form,
        feature_areas,
        requirements,
        graph,
        automation,
        readiness,
        servers,
        resources,
        evidence,
    );
    if let Some(limitation) = limitation {
        interpreted = interpreted.with_bounded_import_limitation(
            limitation,
            InterpretationEvidence::new("bounded-reference-interpretation", limitation.as_str()),
        );
    }
    Ok(interpreted)
}

fn interpret_wdspec(
    record: &WptSourceRecord,
    source: &crate::model::WptSourceFile,
    metadata: &WptRecordMetadata,
) -> Result<InterpretedWptRecord, WptInterpretationError> {
    let mut builder = SourceRequirementsBuilder::new();
    builder
        .requirement_tag(RequirementTag::RequiresBrowserRuntimeFeature)
        .capability(
            CapabilityRequirement::new(
                EngineCapabilityKind::BrowserRuntimeFeature,
                Some(capability_feature("webdriver")?),
            )
            .map_err(|_| WptInterpretationError::InvalidGenericProjection)?,
        )
        .harness(GenericHarnessRequirement::SubsystemAdapter(
            harness_feature("webdriver-automation")?,
        ))
        .resource(GenericResourceRequirement::SelfContained);
    let mut servers = Vec::new();
    let mut resources = vec![WptResourceDetail::SelfContained];
    let evidence = vec![
        InterpretationEvidence::new("wpt-authored-form", "webdriver/tests Python wdspec"),
        InterpretationEvidence::new("source-path", source.identity().path().as_str()),
        InterpretationEvidence::new("source-sha256", source.identity().sha256().to_hex()),
    ];
    validate_record_metadata_evidence(metadata, &evidence)?;
    let feature_areas =
        apply_source_metadata(metadata, &mut builder, &mut servers, &mut resources)?;
    Ok(InterpretedWptRecord::new(
        record.id().clone(),
        WptSourceForm::WdSpec,
        feature_areas,
        builder
            .build()
            .map_err(|_| WptInterpretationError::InvalidGenericProjection)?,
        None,
        vec![WptAutomationRequirement::WebDriverSession],
        Vec::new(),
        servers,
        resources,
        evidence,
    ))
}

fn apply_source_metadata(
    metadata: &WptRecordMetadata,
    builder: &mut SourceRequirementsBuilder,
    servers: &mut Vec<WptServerRequirement>,
    resources: &mut Vec<WptResourceDetail>,
) -> Result<Vec<CapabilityFeatureId>, WptInterpretationError> {
    let mut feature_areas = Vec::new();
    if metadata.no_js().is_some() {
        builder.requirement_tag(RequirementTag::NoJs);
    }
    for area in metadata.feature_areas() {
        feature_areas.push(area.id().clone());
        if let Some(kind) = area.capability_kind() {
            builder.requirement_tag(kind.requirement_tag()).capability(
                CapabilityRequirement::new(kind, Some(area.id().clone()))
                    .map_err(|_| WptInterpretationError::InvalidGenericProjection)?,
            );
        }
    }
    for capability in metadata.capabilities() {
        builder
            .requirement_tag(capability.requirement().kind().requirement_tag())
            .capability(capability.requirement().clone());
    }
    for requirement in metadata.server_requirements() {
        servers.push(requirement.requirement());
    }
    for controlled in metadata.controlled_http() {
        builder.resource(GenericResourceRequirement::ControlledHttp {
            profile: controlled.profile().clone(),
        });
        resources.push(WptResourceDetail::WptServer);
    }
    Ok(feature_areas)
}

fn build_reference_graph(
    repository_root: &Path,
    set: &ValidatedWptSourceSet,
    root: &UpstreamPath,
    root_facts: &HtmlFacts,
) -> Result<WptReferenceGraph, WptInterpretationError> {
    let mut edges = Vec::new();
    let mut fuzzy = root_facts
        .fuzzy
        .iter()
        .cloned()
        .map(|value| WptFuzzyMetadata::new(root.clone(), value))
        .collect::<Vec<_>>();
    let mut queue = VecDeque::from([(root.clone(), root_facts.references.clone(), 1_usize)]);
    let mut expanded = BTreeSet::new();
    let mut nodes = BTreeSet::from([root.clone()]);
    let root_parents = set
        .file_by_path(root)
        .ok_or(WptInterpretationError::MissingReference)?
        .parents();
    while let Some((from, references, depth)) = queue.pop_front() {
        validate_reference_bounds(depth, nodes.len(), edges.len())?;
        if !expanded.insert(from.clone()) {
            continue;
        }
        for (relation, href) in references {
            validate_reference_bounds(depth, nodes.len(), edges.len() + 1)?;
            let target = resolve_reference(&from, &href)?;
            nodes.insert(from.clone());
            nodes.insert(target.clone());
            validate_reference_bounds(depth, nodes.len(), edges.len() + 1)?;
            let file = set
                .file_by_path(&target)
                .ok_or(WptInterpretationError::MissingReference)?;
            if !file
                .parents()
                .iter()
                .any(|parent| root_parents.contains(parent))
            {
                return Err(WptInterpretationError::MissingReference);
            }
            edges.push(WptReferenceEdge::new(
                from.clone(),
                relation,
                target.clone(),
            ));
            if target.as_str().ends_with(".html") && !expanded.contains(&target) {
                let child = html_facts(&parse_html_metadata(&read_declared_file(
                    repository_root,
                    file,
                )?)?);
                fuzzy.extend(
                    child
                        .fuzzy
                        .iter()
                        .cloned()
                        .map(|value| WptFuzzyMetadata::new(target.clone(), value)),
                );
                queue.push_back((target, child.references, depth + 1));
            }
        }
    }
    if reference_graph_has_cycle(root, &edges) {
        return Err(WptInterpretationError::ReferenceCycle);
    }
    Ok(WptReferenceGraph::new(root.clone(), edges, fuzzy))
}

fn reference_limitation(error: &WptInterpretationError) -> Option<WptInterpretationLimitation> {
    match error {
        WptInterpretationError::MissingReference => {
            Some(WptInterpretationLimitation::IncompleteReferenceClosure)
        }
        WptInterpretationError::InvalidReferencePath => {
            Some(WptInterpretationLimitation::UnsupportedReferencePath)
        }
        WptInterpretationError::ReferenceCycle => Some(WptInterpretationLimitation::ReferenceCycle),
        WptInterpretationError::ReferenceDepthExceeded => {
            Some(WptInterpretationLimitation::ReferenceDepthBound)
        }
        WptInterpretationError::ReferenceNodeLimitExceeded => {
            Some(WptInterpretationLimitation::ReferenceNodeBound)
        }
        WptInterpretationError::ReferenceEdgeLimitExceeded => {
            Some(WptInterpretationLimitation::ReferenceEdgeBound)
        }
        WptInterpretationError::Html(_) => {
            Some(WptInterpretationLimitation::ReferenceDocumentUnimportable)
        }
        _ => None,
    }
}

fn reference_graph_has_cycle(root: &UpstreamPath, edges: &[WptReferenceEdge]) -> bool {
    fn visit(
        node: &UpstreamPath,
        edges: &[WptReferenceEdge],
        active: &mut BTreeSet<UpstreamPath>,
        complete: &mut BTreeSet<UpstreamPath>,
    ) -> bool {
        if active.contains(node) {
            return true;
        }
        if complete.contains(node) {
            return false;
        }
        active.insert(node.clone());
        for edge in edges.iter().filter(|edge| edge.source() == node) {
            if visit(edge.target(), edges, active, complete) {
                return true;
            }
        }
        active.remove(node);
        complete.insert(node.clone());
        false
    }
    visit(root, edges, &mut BTreeSet::new(), &mut BTreeSet::new())
}

fn validate_reference_bounds(
    depth: usize,
    node_count: usize,
    edge_count: usize,
) -> Result<(), WptInterpretationError> {
    if depth > WPT_MAX_REFERENCE_DEPTH {
        return Err(WptInterpretationError::ReferenceDepthExceeded);
    }
    if node_count > WPT_MAX_REFERENCE_NODES {
        return Err(WptInterpretationError::ReferenceNodeLimitExceeded);
    }
    if edge_count > WPT_MAX_REFERENCE_EDGES {
        return Err(WptInterpretationError::ReferenceEdgeLimitExceeded);
    }
    Ok(())
}

fn html_facts(metadata: &ParsedHtmlMetadata) -> HtmlFacts {
    let mut facts = HtmlFacts::default();
    for element in &metadata.elements {
        let attrs = attributes(element);
        match element.name.as_str() {
            "script" => {
                facts.scripts.push(ScriptFact {
                    src: attrs.get("src").cloned(),
                    kind: classify_script_type(attrs.get("type").map(String::as_str)),
                });
            }
            "link" => {
                let rel = attrs
                    .get("rel")
                    .map(|value| value.split_ascii_whitespace().collect::<Vec<_>>())
                    .unwrap_or_default();
                if let Some(href) = attrs.get("href") {
                    if rel.contains(&"match") {
                        facts
                            .references
                            .push((WptReferenceRelation::Match, href.clone()))
                    }
                    if rel.contains(&"mismatch") {
                        facts
                            .references
                            .push((WptReferenceRelation::Mismatch, href.clone()))
                    }
                    if rel.contains(&"help") {
                        facts.help_links.push(href.clone())
                    }
                }
            }
            "meta" => {
                if attrs
                    .get("name")
                    .is_some_and(|v| v.eq_ignore_ascii_case("fuzzy"))
                    && let Some(value) = attrs.get("content")
                {
                    facts.fuzzy.push(value.clone())
                }
            }
            "html" => {
                facts.reftest_wait = attrs.get("class").is_some_and(|value| {
                    value
                        .split_ascii_whitespace()
                        .any(|token| token == "reftest-wait")
                });
            }
            _ => {}
        }
    }
    facts.scripts.sort();
    facts.references.sort();
    facts.help_links.sort();
    facts
}
fn attributes(element: &HtmlElementMetadata) -> BTreeMap<String, String> {
    element.attributes.iter().cloned().collect()
}
impl ScriptFact {
    fn is_executable(&self) -> bool {
        matches!(
            self.kind,
            ScriptKind::ClassicExecutable | ScriptKind::ModuleExecutable
        )
    }
    fn is_executable_resource(&self, name: &str) -> bool {
        self.is_executable()
            && self
                .src
                .as_deref()
                .is_some_and(|value| script_resource_name(value) == Some(name))
    }
}
fn script_resource_name(value: &str) -> Option<&str> {
    value
        .split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('/').next())
}
fn classify_script_type(value: Option<&str>) -> ScriptKind {
    let Some(value) = value else {
        return ScriptKind::ClassicExecutable;
    };
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() || is_javascript_mime(&value) {
        ScriptKind::ClassicExecutable
    } else if value == "module" {
        ScriptKind::ModuleExecutable
    } else if value
        .bytes()
        .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        ScriptKind::InertDataBlock
    } else {
        ScriptKind::NotYetClassifiable
    }
}
fn is_javascript_mime(value: &str) -> bool {
    matches!(
        value,
        "application/ecmascript"
            | "application/javascript"
            | "application/x-ecmascript"
            | "application/x-javascript"
            | "text/ecmascript"
            | "text/javascript"
            | "text/javascript1.0"
            | "text/javascript1.1"
            | "text/javascript1.2"
            | "text/javascript1.3"
            | "text/javascript1.4"
            | "text/javascript1.5"
            | "text/jscript"
            | "text/livescript"
            | "text/x-ecmascript"
            | "text/x-javascript"
    )
}
fn is_wdspec_path(path: &UpstreamPath) -> bool {
    let parts = path.as_str().split('/').collect::<Vec<_>>();
    parts.len() >= 4
        && parts[0] == "webdriver"
        && parts[1] == "tests"
        && path.as_str().ends_with(".py")
}
fn deferred_html_form(path: &UpstreamPath) -> WptSourceForm {
    let value = path.as_str();
    if value.ends_with("-manual.html") {
        WptSourceForm::Manual
    } else if value.ends_with("-visual.html") {
        WptSourceForm::Visual
    } else if value.ends_with("-crash.html") {
        WptSourceForm::CrashTest
    } else if value.ends_with("-print.html") {
        WptSourceForm::PrintReftest
    } else {
        WptSourceForm::NotYetClassifiable
    }
}
fn capability_feature(value: &str) -> Result<CapabilityFeatureId, WptInterpretationError> {
    CapabilityFeatureId::parse(value).map_err(|_| WptInterpretationError::InvalidGenericProjection)
}
fn harness_feature(value: &str) -> Result<HarnessFeatureId, WptInterpretationError> {
    HarnessFeatureId::parse(value).map_err(|_| WptInterpretationError::InvalidGenericProjection)
}
fn environment_profile(value: &str) -> Result<EnvironmentProfileId, WptInterpretationError> {
    EnvironmentProfileId::parse(value).map_err(|_| WptInterpretationError::InvalidGenericProjection)
}
fn resource_profile(value: &str) -> Result<ResourceProfileId, WptInterpretationError> {
    ResourceProfileId::parse(value).map_err(|_| WptInterpretationError::InvalidGenericProjection)
}
fn resolve_reference(
    from: &UpstreamPath,
    href: &str,
) -> Result<UpstreamPath, WptInterpretationError> {
    let clean = href
        .split(['?', '#'])
        .next()
        .ok_or(WptInterpretationError::InvalidReferencePath)?;
    let mut parts = if clean.starts_with('/') {
        Vec::new()
    } else {
        from.as_str()
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .tap_mut(|v| {
                v.pop();
            })
    };
    for part in clean.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts
                    .pop()
                    .ok_or(WptInterpretationError::InvalidReferencePath)?;
            }
            value => parts.push(value.to_owned()),
        }
    }
    UpstreamPath::parse(&parts.join("/")).map_err(|_| WptInterpretationError::InvalidReferencePath)
}
trait TapMut: Sized {
    fn tap_mut(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}
impl<T> TapMut for T {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_text_does_not_fake_script_dependency() {
        let metadata=parse_html_metadata(br#"<script>const x='<script src=\"/resources/testdriver.js\">'</script><link rel=match href=ref.html>"#).unwrap();
        let facts = html_facts(&metadata);
        assert_eq!(
            facts.scripts,
            vec![ScriptFact {
                src: None,
                kind: ScriptKind::ClassicExecutable,
            }]
        );
        assert!(
            !facts
                .scripts
                .iter()
                .any(|value| value.is_executable_resource("testdriver.js"))
        );
    }
    #[test]
    fn reference_resolution_is_confined() {
        let root = UpstreamPath::parse("css/a/test.html").unwrap();
        assert_eq!(
            resolve_reference(&root, "../ref.html").unwrap().as_str(),
            "css/ref.html"
        );
        assert!(resolve_reference(&root, "../../../escape.html").is_err());
    }
    #[test]
    fn graph_cycles_are_distinct_from_shared_reference_nodes() {
        let a = UpstreamPath::parse("a.html").unwrap();
        let b = UpstreamPath::parse("b.html").unwrap();
        let c = UpstreamPath::parse("c.html").unwrap();
        let shared = vec![
            WptReferenceEdge::new(a.clone(), WptReferenceRelation::Match, b.clone()),
            WptReferenceEdge::new(a.clone(), WptReferenceRelation::Mismatch, c.clone()),
            WptReferenceEdge::new(c.clone(), WptReferenceRelation::Match, b.clone()),
        ];
        assert!(!reference_graph_has_cycle(&a, &shared));
        let cycle = vec![
            WptReferenceEdge::new(a.clone(), WptReferenceRelation::Match, b.clone()),
            WptReferenceEdge::new(b, WptReferenceRelation::Match, a.clone()),
        ];
        assert!(reference_graph_has_cycle(&a, &cycle));
    }
    #[test]
    fn script_types_distinguish_executable_module_inert_and_unresolved() {
        assert_eq!(classify_script_type(None), ScriptKind::ClassicExecutable);
        assert_eq!(
            classify_script_type(Some("module")),
            ScriptKind::ModuleExecutable
        );
        assert_eq!(
            classify_script_type(Some("application/json")),
            ScriptKind::InertDataBlock
        );
        assert_eq!(
            classify_script_type(Some("text/javascript")),
            ScriptKind::ClassicExecutable
        );
        assert_eq!(
            classify_script_type(Some("bad\nvalue")),
            ScriptKind::NotYetClassifiable
        );
        let facts = html_facts(&parse_html_metadata(br#"<script type=application/json src=/resources/testharness.js></script><link rel=match href=ref.html>"#).unwrap());
        assert!(
            !facts
                .scripts
                .iter()
                .any(|script| script.is_executable_resource("testharness.js"))
        );
    }
    #[test]
    fn valid_deferred_forms_are_not_malformed() {
        let manual = UpstreamPath::parse("css/example-manual.html").unwrap();
        assert_eq!(deferred_html_form(&manual), WptSourceForm::Manual);
        let unknown = UpstreamPath::parse("css/example.html").unwrap();
        assert_eq!(
            deferred_html_form(&unknown),
            WptSourceForm::NotYetClassifiable
        );
    }
    #[test]
    fn reference_graph_bounds_are_explicit_and_independent() {
        assert!(
            validate_reference_bounds(
                WPT_MAX_REFERENCE_DEPTH,
                WPT_MAX_REFERENCE_NODES,
                WPT_MAX_REFERENCE_EDGES
            )
            .is_ok()
        );
        assert_eq!(
            validate_reference_bounds(WPT_MAX_REFERENCE_DEPTH + 1, 1, 0),
            Err(WptInterpretationError::ReferenceDepthExceeded)
        );
        assert_eq!(
            validate_reference_bounds(1, WPT_MAX_REFERENCE_NODES + 1, 0),
            Err(WptInterpretationError::ReferenceNodeLimitExceeded)
        );
        assert_eq!(
            validate_reference_bounds(1, 1, WPT_MAX_REFERENCE_EDGES + 1),
            Err(WptInterpretationError::ReferenceEdgeLimitExceeded)
        );
        for (error, expected) in [
            (
                WptInterpretationError::ReferenceDepthExceeded,
                WptInterpretationLimitation::ReferenceDepthBound,
            ),
            (
                WptInterpretationError::ReferenceNodeLimitExceeded,
                WptInterpretationLimitation::ReferenceNodeBound,
            ),
            (
                WptInterpretationError::ReferenceEdgeLimitExceeded,
                WptInterpretationLimitation::ReferenceEdgeBound,
            ),
        ] {
            assert_eq!(reference_limitation(&error), Some(expected));
        }
    }
}
