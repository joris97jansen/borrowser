//! Human-reviewed AG8 selection filters over already interpreted WPT records.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use conformance_test_support::{
    AssessmentEvidence, CapabilityFeatureId, EngineCapabilityKind, ExternalLineageId,
    GenericResourceRequirement, RequirementTag, SelectionPolicyAssessment, SelectionPolicyState,
    SourceRecordId,
};
use external_test_provenance::{ConfinedFileError, read_confined_regular_file};
use serde::Deserialize;

use crate::{
    InterpretedWptRecord, ValidatedWptSourceSet, WptFilterAssessment, WptFilterDimension,
    WptFilterFact, WptFilterOutcome, WptSourceForm,
};

pub const WPT_SELECTION_POLICY_FORMAT_V1: &str = "borrowser-wpt-selection-policy-v1";
pub const WPT_SELECTION_POLICY_PATH: &str = "tests/conformance/external/wpt/selection-policy.toml";
const MAX_POLICY_BYTES: u64 = 256 * 1024;
const MAX_POLICY_VALUES: usize = 128;

#[derive(Clone, Debug)]
pub struct ValidatedWptSelectionPolicy {
    id: String,
    direct: DirectPolicy,
    records: BTreeMap<SourceRecordId, RecordPolicy>,
    derived: BTreeMap<ExternalLineageId, DerivedPolicy>,
}
impl ValidatedWptSelectionPolicy {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn record(&self, id: &SourceRecordId) -> Option<&RecordPolicy> {
        self.records.get(id)
    }
    pub(crate) fn derived(&self) -> impl Iterator<Item = &DerivedPolicy> {
        self.derived.values()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RecordPolicy {
    category: String,
    path_prefix: String,
}

#[derive(Clone, Debug)]
pub(crate) struct DerivedPolicy {
    lineage_id: ExternalLineageId,
    source_record: SourceRecordId,
    source_form: WptSourceForm,
    category: String,
    feature_area: CapabilityFeatureId,
    requires_no_js: bool,
    resource_classes: Vec<ResourceClass>,
    capability_kind: EngineCapabilityKind,
    capability_feature: CapabilityFeatureId,
    harness_adapter: conformance_test_support::HarnessFeatureId,
    representation: conformance_test_support::RepresentationFeatureId,
}
impl DerivedPolicy {
    pub(crate) fn lineage_id(&self) -> &ExternalLineageId {
        &self.lineage_id
    }
    pub(crate) fn source_record(&self) -> &SourceRecordId {
        &self.source_record
    }
    pub(crate) fn capability_kind(&self) -> EngineCapabilityKind {
        self.capability_kind
    }
    pub(crate) fn capability_feature(&self) -> &CapabilityFeatureId {
        &self.capability_feature
    }
    pub(crate) fn harness_adapter(&self) -> &conformance_test_support::HarnessFeatureId {
        &self.harness_adapter
    }
    pub(crate) fn representation(&self) -> &conformance_test_support::RepresentationFeatureId {
        &self.representation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ResourceClass {
    SelfContained,
    PinnedLocalStatic,
    ControlledHttp,
    ServerBehavior,
    LiveNetwork,
    PlatformService,
}

#[derive(Clone, Debug)]
struct DirectPolicy {
    source_forms: Vec<WptSourceForm>,
    path_categories: Vec<String>,
    feature_areas: Vec<CapabilityFeatureId>,
    require_no_js: bool,
    resource_classes: Vec<ResourceClass>,
    allow_pixel_assertions: bool,
    allow_platform_dependencies: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WptSelectionPolicyError {
    Io,
    TooLarge,
    InvalidSchema,
    UnsupportedVersion,
    InvalidIdentity,
    DuplicateId,
    PopulationMismatch,
    DanglingLineage,
    UnsafePath,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: String,
    policy: String,
    direct: DirectWire,
    records: Vec<RecordWire>,
    derived: Vec<DerivedWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectWire {
    source_forms: Vec<String>,
    path_categories: Vec<String>,
    feature_areas: Vec<String>,
    no_js: String,
    resource_classes: Vec<String>,
    pixel_assertions: String,
    platform_dependencies: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordWire {
    id: String,
    category: String,
    path_prefix: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedWire {
    lineage_id: String,
    source_record: String,
    source_form: String,
    category: String,
    feature_area: String,
    requires_no_js: bool,
    resource_classes: Vec<String>,
    capability_kind: String,
    capability_feature: String,
    harness_adapter: String,
    representation: String,
}

pub fn load_wpt_selection_policy(
    repository_root: &Path,
    set: &ValidatedWptSourceSet,
) -> Result<ValidatedWptSelectionPolicy, WptSelectionPolicyError> {
    let bytes = read_confined_regular_file(
        repository_root,
        Path::new(WPT_SELECTION_POLICY_PATH),
        MAX_POLICY_BYTES,
    )
    .map_err(map_confined_error)?;
    let wire: Wire =
        toml::from_slice(&bytes).map_err(|_| WptSelectionPolicyError::InvalidSchema)?;
    if wire.format != WPT_SELECTION_POLICY_FORMAT_V1 {
        return Err(WptSelectionPolicyError::UnsupportedVersion);
    }
    if wire.records.len() > MAX_POLICY_VALUES || wire.derived.len() > MAX_POLICY_VALUES {
        return Err(WptSelectionPolicyError::TooLarge);
    }
    let direct = DirectPolicy {
        source_forms: parse_many(&wire.direct.source_forms, parse_source_form)?,
        path_categories: validated_strings(wire.direct.path_categories)?,
        feature_areas: parse_many(&wire.direct.feature_areas, |v| {
            CapabilityFeatureId::parse(v).map_err(|_| WptSelectionPolicyError::InvalidIdentity)
        })?,
        require_no_js: match wire.direct.no_js.as_str() {
            "required" => true,
            "allowed" => false,
            _ => return Err(WptSelectionPolicyError::InvalidSchema),
        },
        resource_classes: parse_many(&wire.direct.resource_classes, parse_resource_class)?,
        allow_pixel_assertions: parse_allowance(&wire.direct.pixel_assertions)?,
        allow_platform_dependencies: parse_allowance(&wire.direct.platform_dependencies)?,
    };
    let mut records = BTreeMap::new();
    for value in wire.records {
        let id = SourceRecordId::parse(&value.id)
            .map_err(|_| WptSelectionPolicyError::InvalidIdentity)?;
        let source = set
            .record(&id)
            .ok_or(WptSelectionPolicyError::PopulationMismatch)?;
        let source_file = set
            .file_by_id(source.source_file_id())
            .ok_or(WptSelectionPolicyError::PopulationMismatch)?;
        if !value.path_prefix.ends_with('/')
            || !source_file
                .identity()
                .path()
                .as_str()
                .starts_with(&value.path_prefix)
        {
            return Err(WptSelectionPolicyError::InvalidIdentity);
        }
        if records
            .insert(
                id,
                RecordPolicy {
                    category: value.category,
                    path_prefix: value.path_prefix,
                },
            )
            .is_some()
        {
            return Err(WptSelectionPolicyError::DuplicateId);
        }
    }
    if records.len() != set.records().len()
        || set
            .records()
            .iter()
            .any(|record| !records.contains_key(record.id()))
    {
        return Err(WptSelectionPolicyError::PopulationMismatch);
    }
    let mut derived = BTreeMap::new();
    for value in wire.derived {
        let lineage_id = ExternalLineageId::parse(&value.lineage_id)
            .map_err(|_| WptSelectionPolicyError::InvalidIdentity)?;
        let source_record = SourceRecordId::parse(&value.source_record)
            .map_err(|_| WptSelectionPolicyError::InvalidIdentity)?;
        if !set
            .lineages()
            .iter()
            .any(|lineage| lineage.id() == &lineage_id && lineage.source_record() == &source_record)
        {
            return Err(WptSelectionPolicyError::DanglingLineage);
        }
        let policy = DerivedPolicy {
            lineage_id: lineage_id.clone(),
            source_record,
            source_form: parse_source_form(&value.source_form)?,
            category: value.category,
            feature_area: CapabilityFeatureId::parse(&value.feature_area)
                .map_err(|_| WptSelectionPolicyError::InvalidIdentity)?,
            requires_no_js: value.requires_no_js,
            resource_classes: parse_many(&value.resource_classes, parse_resource_class)?,
            capability_kind: EngineCapabilityKind::parse(&value.capability_kind)
                .ok_or(WptSelectionPolicyError::InvalidSchema)?,
            capability_feature: CapabilityFeatureId::parse(&value.capability_feature)
                .map_err(|_| WptSelectionPolicyError::InvalidIdentity)?,
            harness_adapter: conformance_test_support::HarnessFeatureId::parse(
                &value.harness_adapter,
            )
            .map_err(|_| WptSelectionPolicyError::InvalidIdentity)?,
            representation: conformance_test_support::RepresentationFeatureId::parse(
                &value.representation,
            )
            .map_err(|_| WptSelectionPolicyError::InvalidIdentity)?,
        };
        if derived.insert(lineage_id, policy).is_some() {
            return Err(WptSelectionPolicyError::DuplicateId);
        }
    }
    Ok(ValidatedWptSelectionPolicy {
        id: wire.policy,
        direct,
        records,
        derived,
    })
}

pub fn evaluate_wpt_filter(
    policy: &ValidatedWptSelectionPolicy,
    record: &InterpretedWptRecord,
) -> Result<WptFilterAssessment, WptSelectionPolicyError> {
    let metadata = policy
        .record(record.source_record_id())
        .ok_or(WptSelectionPolicyError::PopulationMismatch)?;
    let mut facts = Vec::new();
    fact(
        &mut facts,
        WptFilterDimension::TestType,
        policy.direct.source_forms.contains(&record.source_form()),
        format!("source form {}", record.source_form().as_str()),
    );
    fact(
        &mut facts,
        WptFilterDimension::PathCategory,
        policy.direct.path_categories.contains(&metadata.category),
        format!(
            "category {} under {}",
            metadata.category, metadata.path_prefix
        ),
    );
    fact(
        &mut facts,
        WptFilterDimension::FeatureArea,
        record
            .feature_areas()
            .iter()
            .any(|area| policy.direct.feature_areas.contains(area)),
        format!(
            "declared feature areas {}",
            record
                .feature_areas()
                .iter()
                .map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
    );
    let no_js = record
        .generic_requirements()
        .requirement_tags()
        .contains(&RequirementTag::NoJs);
    let requires_js = record
        .generic_requirements()
        .requirement_tags()
        .contains(&RequirementTag::RequiresJs);
    let (no_js_outcome, no_js_evidence) = if !policy.direct.require_no_js {
        (
            WptFilterOutcome::Included,
            "policy permits sources regardless of no-JS classification",
        )
    } else if no_js {
        (
            WptFilterOutcome::Included,
            "positive source metadata establishes no-JS compatibility",
        )
    } else if requires_js {
        (
            WptFilterOutcome::Excluded,
            "positive source interpretation establishes a JavaScript requirement",
        )
    } else {
        (
            WptFilterOutcome::NotYetEstablished,
            "no-JS compatibility is not yet positively established",
        )
    };
    facts.push(WptFilterFact::new(
        WptFilterDimension::NoJsCompatibility,
        no_js_outcome,
        no_js_evidence.to_owned(),
    ));
    let resources = resource_classes(record);
    fact(
        &mut facts,
        WptFilterDimension::ResourceAndNetwork,
        resources
            .iter()
            .all(|kind| policy.direct.resource_classes.contains(kind)),
        format!(
            "resource classes {}",
            resources
                .iter()
                .map(|v| resource_class_str(*v))
                .collect::<Vec<_>>()
                .join(",")
        ),
    );
    let pixel = record
        .generic_requirements()
        .requirement_tags()
        .contains(&RequirementTag::RequiresPixelComparison);
    fact(
        &mut facts,
        WptFilterDimension::RenderingAndPixel,
        policy.direct.allow_pixel_assertions || !pixel,
        if pixel {
            "source requires pixel comparison".to_owned()
        } else {
            "source has no pixel requirement".to_owned()
        },
    );
    let platform = record
        .generic_requirements()
        .resources()
        .iter()
        .any(|v| matches!(v, GenericResourceRequirement::PlatformService { .. }))
        || record
            .generic_requirements()
            .requirement_tags()
            .contains(&RequirementTag::RequiresUserInteraction)
        || record.source_form() == WptSourceForm::WdSpec;
    fact(
        &mut facts,
        WptFilterDimension::PlatformDependency,
        policy.direct.allow_platform_dependencies || !platform,
        if platform {
            "source requires unsupported platform or automation dependency".to_owned()
        } else {
            "no unsupported platform dependency declared".to_owned()
        },
    );
    Ok(WptFilterAssessment::new(facts))
}

pub(crate) fn evaluate_derived_filter(
    policy: &ValidatedWptSelectionPolicy,
    derived: &DerivedPolicy,
    record: &InterpretedWptRecord,
) -> Result<SelectionPolicyAssessment, WptSelectionPolicyError> {
    let metadata = policy
        .record(record.source_record_id())
        .ok_or(WptSelectionPolicyError::PopulationMismatch)?;
    let resources = resource_classes(record);
    let included = derived.source_record == *record.source_record_id()
        && derived.source_form == record.source_form()
        && derived.category == metadata.category
        && record
            .feature_areas()
            .iter()
            .any(|area| area == &derived.feature_area)
        && (!derived.requires_no_js
            || record
                .generic_requirements()
                .requirement_tags()
                .contains(&RequirementTag::NoJs))
        && resources
            .iter()
            .all(|kind| derived.resource_classes.contains(kind));
    Ok(SelectionPolicyAssessment::new(if included { SelectionPolicyState::Included } else { SelectionPolicyState::Excluded }, vec![AssessmentEvidence::parse(if included { "The derived adaptation satisfies its explicit source-form, category, feature-area, no-JS, and resource filters." } else { "The derived adaptation does not satisfy every declared source filter." }).expect("static evidence")]))
}

fn fact(
    output: &mut Vec<WptFilterFact>,
    dimension: WptFilterDimension,
    included: bool,
    evidence: String,
) {
    output.push(WptFilterFact::new(
        dimension,
        if included {
            WptFilterOutcome::Included
        } else {
            WptFilterOutcome::Excluded
        },
        evidence,
    ));
}
fn resource_classes(record: &InterpretedWptRecord) -> BTreeSet<ResourceClass> {
    record
        .generic_requirements()
        .resources()
        .iter()
        .map(|v| match v {
            GenericResourceRequirement::SelfContained => ResourceClass::SelfContained,
            GenericResourceRequirement::PinnedLocalStatic { .. } => {
                ResourceClass::PinnedLocalStatic
            }
            GenericResourceRequirement::ControlledHttp { .. } => ResourceClass::ControlledHttp,
            GenericResourceRequirement::ServerBehavior { .. } => ResourceClass::ServerBehavior,
            GenericResourceRequirement::LiveNetwork { .. } => ResourceClass::LiveNetwork,
            GenericResourceRequirement::PlatformService { .. } => ResourceClass::PlatformService,
        })
        .collect()
}
fn parse_source_form(value: &str) -> Result<WptSourceForm, WptSelectionPolicyError> {
    match value {
        "reftest" => Ok(WptSourceForm::Reftest),
        "testharness" => Ok(WptSourceForm::TestHarness),
        "wdspec" => Ok(WptSourceForm::WdSpec),
        _ => Err(WptSelectionPolicyError::InvalidSchema),
    }
}
fn parse_resource_class(value: &str) -> Result<ResourceClass, WptSelectionPolicyError> {
    match value {
        "self-contained" => Ok(ResourceClass::SelfContained),
        "pinned-local-static" => Ok(ResourceClass::PinnedLocalStatic),
        "controlled-http" => Ok(ResourceClass::ControlledHttp),
        "server-behavior" => Ok(ResourceClass::ServerBehavior),
        "live-network" => Ok(ResourceClass::LiveNetwork),
        "platform-service" => Ok(ResourceClass::PlatformService),
        _ => Err(WptSelectionPolicyError::InvalidSchema),
    }
}
fn resource_class_str(value: ResourceClass) -> &'static str {
    match value {
        ResourceClass::SelfContained => "self-contained",
        ResourceClass::PinnedLocalStatic => "pinned-local-static",
        ResourceClass::ControlledHttp => "controlled-http",
        ResourceClass::ServerBehavior => "server-behavior",
        ResourceClass::LiveNetwork => "live-network",
        ResourceClass::PlatformService => "platform-service",
    }
}
fn map_confined_error(error: ConfinedFileError) -> WptSelectionPolicyError {
    match error {
        ConfinedFileError::TooLarge => WptSelectionPolicyError::TooLarge,
        ConfinedFileError::Io | ConfinedFileError::Missing => WptSelectionPolicyError::Io,
        _ => WptSelectionPolicyError::UnsafePath,
    }
}
fn parse_allowance(value: &str) -> Result<bool, WptSelectionPolicyError> {
    match value {
        "allow" => Ok(true),
        "exclude" => Ok(false),
        _ => Err(WptSelectionPolicyError::InvalidSchema),
    }
}
fn validated_strings(mut values: Vec<String>) -> Result<Vec<String>, WptSelectionPolicyError> {
    if values.len() > MAX_POLICY_VALUES || values.iter().any(|v| v.is_empty() || v.trim() != v) {
        return Err(WptSelectionPolicyError::InvalidIdentity);
    }
    values.sort();
    values.dedup();
    Ok(values)
}
fn parse_many<T: Ord>(
    values: &[String],
    parser: impl Fn(&str) -> Result<T, WptSelectionPolicyError>,
) -> Result<Vec<T>, WptSelectionPolicyError> {
    if values.len() > MAX_POLICY_VALUES {
        return Err(WptSelectionPolicyError::TooLarge);
    }
    let mut parsed = values
        .iter()
        .map(|v| parser(v))
        .collect::<Result<Vec<_>, _>>()?;
    parsed.sort();
    parsed.dedup();
    Ok(parsed)
}
