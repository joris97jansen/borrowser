use conformance_test_support::{
    AccountedDerivedAdaptation, AccountedExternalSource, ExternalAdapterVersion, ExternalLineageId,
    HarnessFeatureId, SourceRecordId, SourceRequirements, TestId,
};
use external_test_provenance::{ExternalFileIdentity, Sha256Digest, UpstreamPath};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptSourceForm {
    Reftest,
    TestHarness,
    WdSpec,
    Manual,
    Visual,
    CrashTest,
    PrintReftest,
    NotYetClassifiable,
    MalformedOrUnimportable,
}
impl WptSourceForm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reftest => "reftest",
            Self::TestHarness => "testharness",
            Self::WdSpec => "wdspec",
            Self::Manual => "manual-deferred",
            Self::Visual => "visual-deferred",
            Self::CrashTest => "crashtest-deferred",
            Self::PrintReftest => "print-reftest-deferred",
            Self::NotYetClassifiable => "not-yet-classifiable",
            Self::MalformedOrUnimportable => "malformed-or-unimportable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptReferenceRelation {
    Match,
    Mismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WptFuzzyMetadata {
    owner: UpstreamPath,
    value: String,
}
impl WptFuzzyMetadata {
    pub(crate) fn new(owner: UpstreamPath, value: String) -> Self {
        Self { owner, value }
    }
    pub fn owner(&self) -> &UpstreamPath {
        &self.owner
    }
    pub fn value(&self) -> &str {
        &self.value
    }
}
impl WptReferenceRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Mismatch => "mismatch",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WptReferenceEdge {
    source: UpstreamPath,
    relation: WptReferenceRelation,
    target: UpstreamPath,
}
impl WptReferenceEdge {
    pub(crate) fn new(
        source: UpstreamPath,
        relation: WptReferenceRelation,
        target: UpstreamPath,
    ) -> Self {
        Self {
            source,
            relation,
            target,
        }
    }
    pub fn source(&self) -> &UpstreamPath {
        &self.source
    }
    pub fn relation(&self) -> WptReferenceRelation {
        self.relation
    }
    pub fn target(&self) -> &UpstreamPath {
        &self.target
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WptReferenceGraph {
    root: UpstreamPath,
    edges: Vec<WptReferenceEdge>,
    fuzzy_metadata: Vec<WptFuzzyMetadata>,
}
impl WptReferenceGraph {
    pub(crate) fn new(
        root: UpstreamPath,
        mut edges: Vec<WptReferenceEdge>,
        mut fuzzy_metadata: Vec<WptFuzzyMetadata>,
    ) -> Self {
        edges.sort_by(|a, b| {
            (&a.source, a.relation, &a.target).cmp(&(&b.source, b.relation, &b.target))
        });
        fuzzy_metadata.sort();
        fuzzy_metadata.dedup();
        Self {
            root,
            edges,
            fuzzy_metadata,
        }
    }
    pub fn root(&self) -> &UpstreamPath {
        &self.root
    }
    pub fn edges(&self) -> &[WptReferenceEdge] {
        &self.edges
    }
    pub fn fuzzy_metadata(&self) -> &[WptFuzzyMetadata] {
        &self.fuzzy_metadata
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptInterpretationLimitation {
    IncompleteReferenceClosure,
    UnsupportedReferencePath,
    ReferenceCycle,
    ReferenceDepthBound,
    ReferenceNodeBound,
    ReferenceEdgeBound,
    ReferenceDocumentUnimportable,
}
impl WptInterpretationLimitation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IncompleteReferenceClosure => "incomplete-reference-closure",
            Self::UnsupportedReferencePath => "unsupported-reference-path",
            Self::ReferenceCycle => "reference-cycle",
            Self::ReferenceDepthBound => "reference-depth-bound",
            Self::ReferenceNodeBound => "reference-node-bound",
            Self::ReferenceEdgeBound => "reference-edge-bound",
            Self::ReferenceDocumentUnimportable => "reference-document-unimportable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptInterpretationStatus {
    Complete,
    MalformedOrUnimportable,
    NotYetClassifiable,
    BoundedImportLimitation(WptInterpretationLimitation),
}
impl WptInterpretationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::MalformedOrUnimportable => "malformed-or-unimportable",
            Self::NotYetClassifiable => "not-yet-classifiable",
            Self::BoundedImportLimitation(_) => "bounded-import-limitation",
        }
    }
    pub fn limitation(self) -> Option<WptInterpretationLimitation> {
        match self {
            Self::BoundedImportLimitation(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptAutomationRequirement {
    TestHarnessJavascript,
    TestDriver,
    WebDriverSession,
}
impl WptAutomationRequirement {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TestHarnessJavascript => "testharness-javascript",
            Self::TestDriver => "testdriver",
            Self::WebDriverSession => "webdriver-session",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptReadinessRequirement {
    ReftestWait,
    UnclassifiedScriptType,
}
impl WptReadinessRequirement {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReftestWait => "reftest-wait",
            Self::UnclassifiedScriptType => "unclassified-script-type",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptServerRequirement {
    Substitution,
    SpecialOrigins,
    PipesAndHeaders,
}
impl WptServerRequirement {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Substitution => "substitution",
            Self::SpecialOrigins => "special-origins",
            Self::PipesAndHeaders => "pipes-and-headers",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptResourceDetail {
    SelfContained,
    PinnedLocal(UpstreamPath),
    WptServer,
    LiveNetwork,
    PlatformService(String),
}
impl WptResourceDetail {
    pub fn as_str(&self) -> String {
        match self {
            Self::SelfContained => "self-contained".to_owned(),
            Self::PinnedLocal(path) => format!("pinned-local:{}", path.as_str()),
            Self::WptServer => "wpt-server".to_owned(),
            Self::LiveNetwork => "live-network".to_owned(),
            Self::PlatformService(value) => format!("platform-service:{value}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InterpretationEvidence {
    kind: String,
    value: String,
}
impl InterpretationEvidence {
    pub(crate) fn new(kind: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            value: value.into(),
        }
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterpretedWptRecord {
    source_record_id: SourceRecordId,
    source_form: WptSourceForm,
    interpretation_status: WptInterpretationStatus,
    feature_areas: Vec<conformance_test_support::CapabilityFeatureId>,
    generic_requirements: SourceRequirements,
    reference_graph: Option<WptReferenceGraph>,
    automation_requirements: Vec<WptAutomationRequirement>,
    readiness_requirements: Vec<WptReadinessRequirement>,
    server_requirements: Vec<WptServerRequirement>,
    resource_details: Vec<WptResourceDetail>,
    interpretation_evidence: Vec<InterpretationEvidence>,
}
impl InterpretedWptRecord {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        source_record_id: SourceRecordId,
        source_form: WptSourceForm,
        mut feature_areas: Vec<conformance_test_support::CapabilityFeatureId>,
        generic_requirements: SourceRequirements,
        reference_graph: Option<WptReferenceGraph>,
        mut automation_requirements: Vec<WptAutomationRequirement>,
        mut readiness_requirements: Vec<WptReadinessRequirement>,
        mut server_requirements: Vec<WptServerRequirement>,
        mut resource_details: Vec<WptResourceDetail>,
        mut interpretation_evidence: Vec<InterpretationEvidence>,
    ) -> Self {
        feature_areas.sort();
        feature_areas.dedup();
        automation_requirements.sort();
        automation_requirements.dedup();
        readiness_requirements.sort();
        readiness_requirements.dedup();
        server_requirements.sort();
        server_requirements.dedup();
        resource_details.sort();
        resource_details.dedup();
        interpretation_evidence.sort();
        let interpretation_status = match source_form {
            WptSourceForm::MalformedOrUnimportable => {
                WptInterpretationStatus::MalformedOrUnimportable
            }
            WptSourceForm::NotYetClassifiable => WptInterpretationStatus::NotYetClassifiable,
            _ => WptInterpretationStatus::Complete,
        };
        Self {
            source_record_id,
            source_form,
            interpretation_status,
            feature_areas,
            generic_requirements,
            reference_graph,
            automation_requirements,
            readiness_requirements,
            server_requirements,
            resource_details,
            interpretation_evidence,
        }
    }
    pub fn source_record_id(&self) -> &SourceRecordId {
        &self.source_record_id
    }
    pub fn source_form(&self) -> WptSourceForm {
        self.source_form
    }
    pub fn interpretation_status(&self) -> WptInterpretationStatus {
        self.interpretation_status
    }
    pub fn feature_areas(&self) -> &[conformance_test_support::CapabilityFeatureId] {
        &self.feature_areas
    }
    pub fn generic_requirements(&self) -> &SourceRequirements {
        &self.generic_requirements
    }
    pub fn reference_graph(&self) -> Option<&WptReferenceGraph> {
        self.reference_graph.as_ref()
    }
    pub fn automation_requirements(&self) -> &[WptAutomationRequirement] {
        &self.automation_requirements
    }
    pub fn readiness_requirements(&self) -> &[WptReadinessRequirement] {
        &self.readiness_requirements
    }
    pub fn server_requirements(&self) -> &[WptServerRequirement] {
        &self.server_requirements
    }
    pub fn resource_details(&self) -> &[WptResourceDetail] {
        &self.resource_details
    }
    pub fn interpretation_evidence(&self) -> &[InterpretationEvidence] {
        &self.interpretation_evidence
    }

    pub(crate) fn malformed(
        source_record_id: SourceRecordId,
        evidence: InterpretationEvidence,
    ) -> Self {
        Self::new(
            source_record_id,
            WptSourceForm::MalformedOrUnimportable,
            Vec::new(),
            conformance_test_support::SourceRequirementsBuilder::new()
                .build()
                .expect("empty requirements are valid"),
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![evidence],
        )
    }

    pub(crate) fn not_yet_classifiable(
        source_record_id: SourceRecordId,
        evidence: InterpretationEvidence,
    ) -> Self {
        Self::new(
            source_record_id,
            WptSourceForm::NotYetClassifiable,
            Vec::new(),
            conformance_test_support::SourceRequirementsBuilder::new()
                .build()
                .expect("empty requirements are valid"),
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![evidence],
        )
    }

    pub(crate) fn with_bounded_import_limitation(
        mut self,
        limitation: WptInterpretationLimitation,
        evidence: InterpretationEvidence,
    ) -> Self {
        self.interpretation_status = WptInterpretationStatus::BoundedImportLimitation(limitation);
        self.interpretation_evidence.push(evidence);
        self.interpretation_evidence.sort();
        self.reference_graph = None;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountedWptRecord {
    interpreted: InterpretedWptRecord,
    generic_accounting: AccountedExternalSource,
    filter_assessment: WptFilterAssessment,
    derived_adaptations: Vec<AccountedDerivedAdaptation>,
}
impl AccountedWptRecord {
    pub(crate) fn new(
        interpreted: InterpretedWptRecord,
        generic_accounting: AccountedExternalSource,
        filter_assessment: WptFilterAssessment,
        mut derived_adaptations: Vec<AccountedDerivedAdaptation>,
    ) -> Self {
        debug_assert_eq!(
            interpreted.source_record_id(),
            generic_accounting.source_record_id()
        );
        derived_adaptations.sort_by(|a, b| a.lineage_id().cmp(b.lineage_id()));
        Self {
            interpreted,
            generic_accounting,
            filter_assessment,
            derived_adaptations,
        }
    }
    pub fn interpreted(&self) -> &InterpretedWptRecord {
        &self.interpreted
    }
    pub fn generic_accounting(&self) -> &AccountedExternalSource {
        &self.generic_accounting
    }
    pub fn filter_assessment(&self) -> &WptFilterAssessment {
        &self.filter_assessment
    }
    pub fn derived_adaptations(&self) -> &[AccountedDerivedAdaptation] {
        &self.derived_adaptations
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptFilterDimension {
    TestType,
    PathCategory,
    FeatureArea,
    NoJsCompatibility,
    ResourceAndNetwork,
    RenderingAndPixel,
    PlatformDependency,
}
impl WptFilterDimension {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TestType => "test-type",
            Self::PathCategory => "path-category",
            Self::FeatureArea => "feature-area",
            Self::NoJsCompatibility => "no-js-compatibility",
            Self::ResourceAndNetwork => "resource-and-network",
            Self::RenderingAndPixel => "rendering-and-pixel",
            Self::PlatformDependency => "platform-dependency",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WptFilterOutcome {
    Included,
    Excluded,
    NotYetEstablished,
}
impl WptFilterOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Included => "included",
            Self::Excluded => "excluded",
            Self::NotYetEstablished => "not-yet-established",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WptFilterFact {
    dimension: WptFilterDimension,
    outcome: WptFilterOutcome,
    evidence: String,
}
impl WptFilterFact {
    pub(crate) fn new(
        dimension: WptFilterDimension,
        outcome: WptFilterOutcome,
        evidence: String,
    ) -> Self {
        Self {
            dimension,
            outcome,
            evidence,
        }
    }
    pub fn dimension(&self) -> WptFilterDimension {
        self.dimension
    }
    pub fn outcome(&self) -> WptFilterOutcome {
        self.outcome
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WptFilterAssessment {
    facts: Vec<WptFilterFact>,
}
impl WptFilterAssessment {
    pub(crate) fn new(mut facts: Vec<WptFilterFact>) -> Self {
        facts.sort();
        Self { facts }
    }
    pub fn facts(&self) -> &[WptFilterFact] {
        &self.facts
    }
}

#[derive(Clone, Debug)]
pub struct WptSourceFile {
    id: String,
    identity: ExternalFileIdentity,
    local_path: String,
    role: WptFileRole,
    parents: Vec<SourceRecordId>,
}
impl WptSourceFile {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn identity(&self) -> &ExternalFileIdentity {
        &self.identity
    }
    pub fn local_path(&self) -> &str {
        &self.local_path
    }
    pub fn role(&self) -> WptFileRole {
        self.role
    }
    pub fn parents(&self) -> &[SourceRecordId] {
        &self.parents
    }
    pub(crate) fn new(
        id: String,
        identity: ExternalFileIdentity,
        local_path: String,
        role: WptFileRole,
        parents: Vec<SourceRecordId>,
    ) -> Self {
        Self {
            id,
            identity,
            local_path,
            role,
            parents,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WptFileRole {
    AccountedSource,
    ReferenceNode,
    StaticResource,
}
impl WptFileRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AccountedSource => "accounted-source",
            Self::ReferenceNode => "reference-node",
            Self::StaticResource => "static-resource",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DerivedFixtureLineage {
    id: ExternalLineageId,
    source_record: SourceRecordId,
    adapter: DerivedFixtureAdapter,
    adaptation: DerivedFixtureAdaptation,
}
impl DerivedFixtureLineage {
    pub fn id(&self) -> &ExternalLineageId {
        &self.id
    }
    pub fn source_record(&self) -> &SourceRecordId {
        &self.source_record
    }
    pub fn adapter(&self) -> &HarnessFeatureId {
        &self.adapter.id
    }
    pub fn adapter_version(&self) -> &ExternalAdapterVersion {
        &self.adapter.version
    }
    pub fn derived_test_id(&self) -> &TestId {
        &self.adapter.derived_test_id
    }
    pub fn description(&self) -> &str {
        &self.adaptation.description
    }
    pub fn transformation(&self) -> WptAdaptationTransformation {
        self.adaptation.transformation
    }
    pub fn reference_file_id(&self) -> &str {
        &self.adaptation.reference_file_id
    }
    pub fn test_artifact_sha256(&self) -> Sha256Digest {
        self.adaptation.test_artifact_sha256
    }
    pub fn reference_artifact_sha256(&self) -> Sha256Digest {
        self.adaptation.reference_artifact_sha256
    }
    pub(crate) fn new(
        id: ExternalLineageId,
        source_record: SourceRecordId,
        adapter: DerivedFixtureAdapter,
        adaptation: DerivedFixtureAdaptation,
    ) -> Self {
        Self {
            id,
            source_record,
            adapter,
            adaptation,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DerivedFixtureAdapter {
    id: HarnessFeatureId,
    version: ExternalAdapterVersion,
    derived_test_id: TestId,
}
impl DerivedFixtureAdapter {
    pub(crate) fn new(
        id: HarnessFeatureId,
        version: ExternalAdapterVersion,
        derived_test_id: TestId,
    ) -> Self {
        Self {
            id,
            version,
            derived_test_id,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DerivedFixtureAdaptation {
    description: String,
    transformation: WptAdaptationTransformation,
    reference_file_id: String,
    test_artifact_sha256: Sha256Digest,
    reference_artifact_sha256: Sha256Digest,
}
impl DerivedFixtureAdaptation {
    pub(crate) fn new(
        description: String,
        transformation: WptAdaptationTransformation,
        reference_file_id: String,
        test_artifact_sha256: Sha256Digest,
        reference_artifact_sha256: Sha256Digest,
    ) -> Self {
        Self {
            description,
            transformation,
            reference_file_id,
            test_artifact_sha256,
            reference_artifact_sha256,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WptAdaptationTransformation {
    ExactCopyV1,
}
impl WptAdaptationTransformation {
    pub fn as_str(self) -> &'static str {
        "exact-copy-v1"
    }
}

pub const WPT_MAX_SOURCE_FILES: usize = 256;
pub const WPT_MAX_SOURCE_RECORDS: usize = 128;
pub const WPT_MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const WPT_MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
pub const WPT_MAX_REFERENCE_DEPTH: usize = 8;
pub const WPT_MAX_REFERENCE_NODES: usize = 64;
pub const WPT_MAX_REFERENCE_EDGES: usize = 128;
pub const WPT_MAX_CLOSURE_FILES_PER_RECORD: usize = 32;
