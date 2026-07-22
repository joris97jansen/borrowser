use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureFileV1 {
    pub format: String,
    pub id: String,
    pub source: FixtureSourceDeclaration,
    pub input: InputDeclaration,
    pub execution: ExecutionDeclaration,
    pub expectations: FixtureExpectationDeclarations,
    pub disposition: FixtureDispositionDeclaration,
    #[serde(default)]
    pub metadata: FixtureMetadataDeclaration,
    #[serde(default)]
    pub extensions: BTreeMap<String, ExtensionDeclaration>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureMetadataDeclaration {
    pub description: Option<String>,
    #[serde(default)]
    pub comments: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FixtureSourceKindDeclaration {
    Native,
    External,
    Quarantine,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSourceDeclaration {
    pub kind: FixtureSourceKindDeclaration,
    pub provenance: Option<String>,
    pub tracking_issue: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InputKindDeclaration {
    Utf8Text,
    RawBytes,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputDeclaration {
    pub path: String,
    pub kind: InputKindDeclaration,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionDeclaration {
    pub target: ParserTargetDeclaration,
    pub reference_delivery: String,
    pub deliveries: Vec<DeliveryDeclaration>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ParserTargetKindDeclaration {
    StandaloneTokenizer,
    Document,
    Fragment,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ScriptingDeclaration {
    Disabled,
    Enabled,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FragmentContextDeclaration {
    pub namespace: String,
    pub local_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParserTargetDeclaration {
    pub kind: ParserTargetKindDeclaration,
    pub scripting: Option<ScriptingDeclaration>,
    pub fragment: Option<FragmentContextDeclaration>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryUnitDeclaration {
    Bytes,
    UnicodeScalars,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryStrategyDeclaration {
    Whole,
    Boundaries,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryDeclaration {
    pub name: String,
    pub unit: DeliveryUnitDeclaration,
    pub strategy: DeliveryStrategyDeclaration,
    pub boundaries: Option<Vec<usize>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureExpectationDeclarations {
    pub tokens: Option<String>,
    pub parse_errors: Option<String>,
    pub implementation_diagnostics: Option<String>,
    pub document_mode: Option<String>,
    pub tree: Option<String>,
    pub patches: Option<String>,
    #[serde(default)]
    pub transitions: Vec<TransitionExpectationDeclaration>,
    pub unsupported_features: Option<String>,
    pub final_invariants: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionExpectationDeclaration {
    pub delivery: String,
    pub path: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FixtureDispositionStatusDeclaration {
    Active,
    ExpectedUnsupported,
    ExpectedFailure,
    Skipped,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureDispositionDeclaration {
    pub status: FixtureDispositionStatusDeclaration,
    pub reason: Option<String>,
    pub capability: Option<FixtureCapabilityDeclaration>,
    pub failure: Option<ExpectedFailureDeclaration>,
    pub classification: Option<SkipClassificationDeclaration>,
    pub reference: Option<DispositionReferenceDeclaration>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DispositionReferenceKindDeclaration {
    TrackingIssue,
    Provenance,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispositionReferenceDeclaration {
    pub kind: DispositionReferenceKindDeclaration,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FixtureCapabilityKindDeclaration {
    RawByteInput,
    ByteDelivery,
    UnicodeScalarChunking,
    DocumentExecution,
    FragmentParsing,
    ScriptingEnabled,
    UnknownRequiredExtension,
    TokensExpectation,
    ParseErrorsExpectation,
    ImplementationDiagnosticsExpectation,
    DocumentModeExpectation,
    TreeExpectation,
    PatchesExpectation,
    TransitionsExpectation,
    UnsupportedFeaturesExpectation,
    FinalInvariantsExpectation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FixtureCapabilityDeclaration {
    pub kind: FixtureCapabilityKindDeclaration,
    pub id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectedFailureDeclaration {
    TokenSnapshotRead,
    TokenSnapshotFormat,
    TokenizerDriver,
    ValidatedFixtureInvariant,
    TokensMismatch,
    ParseErrorsMismatch,
    ImplementationDiagnosticsMismatch,
    DocumentModeMismatch,
    TreeMismatch,
    PatchesMismatch,
    TransitionsMismatch,
    UnsupportedFeaturesMismatch,
    FinalInvariantsMismatch,
    DecoderCarryNotEmptyInvariant,
    PreprocessingNotFlushedInvariant,
    EofEmissionInvalidInvariant,
    PendingTokenizerConstructInvariant,
    TokenizerOutputUnaccountedInvariant,
    PendingTableTextInvariant,
    InvalidInsertionModeInvariant,
    OpenElementsInconsistentInvariant,
    ActiveFormattingInconsistentInvariant,
    TemplateModesInconsistentInvariant,
    FormPointerInvalidInvariant,
    ParentChildRelationshipInvalidInvariant,
    NamespaceRelationshipInvalidInvariant,
    TemplateAssociationInvalidInvariant,
    PatchMaterializationIncompleteInvariant,
    LiveTreeMismatchInvariant,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkipClassificationKindDeclaration {
    UnsupportedCapability,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkipClassificationDeclaration {
    pub kind: SkipClassificationKindDeclaration,
    pub capability: FixtureCapabilityDeclaration,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionDeclaration {
    pub required: bool,
    pub value: toml::Value,
}
