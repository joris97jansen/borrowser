use html::ElementNamespace;
use html::conformance::{CanonicalParserResult, InvariantFailureCode};
use std::path::PathBuf;

pub const FIXTURE_FORMAT_V1: &str = "borrowser-html-parser-fixture-v1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixtureId(String);

impl FixtureId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn validated(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeliveryName(String);

impl DeliveryName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn validated(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug)]
pub(super) struct FixtureBundle {
    repository_relative_path: String,
    absolute_path: PathBuf,
}

impl FixtureBundle {
    pub(super) fn validated(repository_relative_path: String, absolute_path: PathBuf) -> Self {
        Self {
            repository_relative_path,
            absolute_path,
        }
    }

    pub(super) fn repository_relative_path(&self) -> &str {
        &self.repository_relative_path
    }

    pub(super) fn absolute_path(&self) -> &std::path::Path {
        &self.absolute_path
    }
}

#[derive(Clone, Debug)]
pub(super) enum FixtureSource {
    Native,
    External { provenance: String },
    Quarantine { tracking_issue: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureSourceKind {
    Native,
    External,
    Quarantine,
}

impl FixtureSource {
    pub(super) fn kind(&self) -> FixtureSourceKind {
        match self {
            Self::Native => FixtureSourceKind::Native,
            Self::External { .. } => FixtureSourceKind::External,
            Self::Quarantine { .. } => FixtureSourceKind::Quarantine,
        }
    }

    pub(super) fn reference(&self) -> Option<&str> {
        match self {
            Self::Native => None,
            Self::External { provenance } => Some(provenance),
            Self::Quarantine { tracking_issue } => Some(tracking_issue),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum ExactInput {
    Utf8Text {
        path: String,
        bytes: Vec<u8>,
        text: String,
        sha256: String,
    },
    RawBytes {
        path: String,
        bytes: Vec<u8>,
        sha256: String,
    },
}

impl ExactInput {
    pub(super) fn bytes(&self) -> &[u8] {
        match self {
            Self::Utf8Text { bytes, .. } | Self::RawBytes { bytes, .. } => bytes,
        }
    }

    pub(super) fn path(&self) -> &str {
        match self {
            Self::Utf8Text { path, .. } | Self::RawBytes { path, .. } => path,
        }
    }

    pub(super) fn sha256(&self) -> &str {
        match self {
            Self::Utf8Text { sha256, .. } | Self::RawBytes { sha256, .. } => sha256,
        }
    }

    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Self::Utf8Text { text, .. } => Some(text),
            Self::RawBytes { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptingMode {
    Disabled,
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserTargetKind {
    StandaloneTokenizer,
    Document,
    Fragment,
}

#[derive(Clone, Debug)]
pub(super) struct FragmentContext {
    namespace: ElementNamespace,
    local_name: String,
}

impl FragmentContext {
    pub(super) fn validated(namespace: ElementNamespace, local_name: String) -> Self {
        Self {
            namespace,
            local_name,
        }
    }

    pub(super) fn namespace(&self) -> ElementNamespace {
        self.namespace
    }

    pub(super) fn local_name(&self) -> &str {
        &self.local_name
    }
}

#[derive(Clone, Debug)]
pub(super) enum ValidatedParserTarget {
    StandaloneTokenizer,
    Document {
        scripting: ScriptingMode,
    },
    Fragment {
        context: FragmentContext,
        scripting: ScriptingMode,
    },
}

#[derive(Clone, Debug)]
pub(super) enum ValidatedDelivery {
    WholeBytes {
        name: DeliveryName,
    },
    ByteBoundaries {
        name: DeliveryName,
        boundaries: Vec<usize>,
    },
    WholeUnicodeScalars {
        name: DeliveryName,
    },
    UnicodeScalarBoundaries {
        name: DeliveryName,
        boundaries: Vec<usize>,
    },
}

impl ValidatedDelivery {
    pub(super) fn name(&self) -> &DeliveryName {
        match self {
            Self::WholeBytes { name }
            | Self::ByteBoundaries { name, .. }
            | Self::WholeUnicodeScalars { name }
            | Self::UnicodeScalarBoundaries { name, .. } => name,
        }
    }

    pub(super) fn boundaries(&self) -> Option<&[usize]> {
        match self {
            Self::WholeBytes { .. } | Self::WholeUnicodeScalars { .. } => None,
            Self::ByteBoundaries { boundaries, .. }
            | Self::UnicodeScalarBoundaries { boundaries, .. } => Some(boundaries),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ValidatedExecution {
    target: ValidatedParserTarget,
    reference_delivery: DeliveryName,
    deliveries: Vec<ValidatedDelivery>,
}

impl ValidatedExecution {
    pub(super) fn validated(
        target: ValidatedParserTarget,
        reference_delivery: DeliveryName,
        deliveries: Vec<ValidatedDelivery>,
    ) -> Self {
        Self {
            target,
            reference_delivery,
            deliveries,
        }
    }

    pub(super) fn target(&self) -> &ValidatedParserTarget {
        &self.target
    }

    pub(super) fn reference_delivery(&self) -> &DeliveryName {
        &self.reference_delivery
    }

    pub(super) fn deliveries(&self) -> &[ValidatedDelivery] {
        &self.deliveries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExpectedSurface<T> {
    NotDeclared,
    Compare(T),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotPath(String);

impl SnapshotPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn validated(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TransitionSnapshotExpectation {
    delivery: DeliveryName,
    path: SnapshotPath,
}

impl TransitionSnapshotExpectation {
    pub(super) fn validated(delivery: DeliveryName, path: SnapshotPath) -> Self {
        Self { delivery, path }
    }

    pub(super) fn delivery(&self) -> &DeliveryName {
        &self.delivery
    }

    pub(super) fn path(&self) -> &SnapshotPath {
        &self.path
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EnabledExpectations {
    tokens: ExpectedSurface<SnapshotPath>,
    parse_errors: ExpectedSurface<SnapshotPath>,
    implementation_diagnostics: ExpectedSurface<SnapshotPath>,
    document_mode: ExpectedSurface<SnapshotPath>,
    tree: ExpectedSurface<SnapshotPath>,
    patches: ExpectedSurface<SnapshotPath>,
    transitions: ExpectedSurface<Vec<TransitionSnapshotExpectation>>,
    unsupported_features: ExpectedSurface<SnapshotPath>,
    final_invariants: ExpectedSurface<SnapshotPath>,
}

impl EnabledExpectations {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validated(
        tokens: ExpectedSurface<SnapshotPath>,
        parse_errors: ExpectedSurface<SnapshotPath>,
        implementation_diagnostics: ExpectedSurface<SnapshotPath>,
        document_mode: ExpectedSurface<SnapshotPath>,
        tree: ExpectedSurface<SnapshotPath>,
        patches: ExpectedSurface<SnapshotPath>,
        transitions: ExpectedSurface<Vec<TransitionSnapshotExpectation>>,
        unsupported_features: ExpectedSurface<SnapshotPath>,
        final_invariants: ExpectedSurface<SnapshotPath>,
    ) -> Self {
        Self {
            tokens,
            parse_errors,
            implementation_diagnostics,
            document_mode,
            tree,
            patches,
            transitions,
            unsupported_features,
            final_invariants,
        }
    }

    pub(super) fn tokens(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.tokens
    }

    pub(super) fn parse_errors(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.parse_errors
    }

    pub(super) fn implementation_diagnostics(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.implementation_diagnostics
    }

    pub(super) fn document_mode(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.document_mode
    }

    pub(super) fn tree(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.tree
    }

    pub(super) fn patches(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.patches
    }

    pub(super) fn transitions(&self) -> &ExpectedSurface<Vec<TransitionSnapshotExpectation>> {
        &self.transitions
    }

    pub(super) fn unsupported_features(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.unsupported_features
    }

    pub(super) fn final_invariants(&self) -> &ExpectedSurface<SnapshotPath> {
        &self.final_invariants
    }

    pub(super) fn is_declared(&self, surface: ExpectationSurface) -> bool {
        match surface {
            ExpectationSurface::Tokens => matches!(self.tokens, ExpectedSurface::Compare(_)),
            ExpectationSurface::ParseErrors => {
                matches!(self.parse_errors, ExpectedSurface::Compare(_))
            }
            ExpectationSurface::ImplementationDiagnostics => {
                matches!(self.implementation_diagnostics, ExpectedSurface::Compare(_))
            }
            ExpectationSurface::DocumentMode => {
                matches!(self.document_mode, ExpectedSurface::Compare(_))
            }
            ExpectationSurface::Tree => matches!(self.tree, ExpectedSurface::Compare(_)),
            ExpectationSurface::Patches => matches!(self.patches, ExpectedSurface::Compare(_)),
            ExpectationSurface::Transitions => {
                matches!(self.transitions, ExpectedSurface::Compare(_))
            }
            ExpectationSurface::UnsupportedFeatures => {
                matches!(self.unsupported_features, ExpectedSurface::Compare(_))
            }
            ExpectationSurface::FinalInvariants => {
                matches!(self.final_invariants, ExpectedSurface::Compare(_))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FixtureCapability {
    RawByteInput,
    ByteDelivery,
    UnicodeScalarChunking,
    DocumentExecution,
    FragmentParsing,
    ScriptingEnabled,
    UnknownRequiredExtension(String),
    Expectation(ExpectationSurface),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExpectationSurface {
    Tokens,
    ParseErrors,
    ImplementationDiagnostics,
    DocumentMode,
    Tree,
    Patches,
    Transitions,
    UnsupportedFeatures,
    FinalInvariants,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExpectedFailureClassification {
    Execution(ExecutionFailureClass),
    ExpectationMismatch(ExpectationSurface),
    InvariantFailure(InvariantFailureCode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExecutionFailureClass {
    SnapshotRead(ExpectationSurface),
    SnapshotFormat(ExpectationSurface),
    TokenizerDriver,
    ValidatedFixtureInvariant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SkipClassification {
    UnsupportedCapability(FixtureCapability),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DispositionReference {
    TrackingIssue(String),
    Provenance(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FixtureDisposition {
    Active,
    ExpectedUnsupported {
        reason: String,
        capability: FixtureCapability,
        reference: DispositionReference,
    },
    ExpectedFailure {
        reason: String,
        failure: ExpectedFailureClassification,
        reference: DispositionReference,
    },
    Skipped {
        reason: String,
        classification: SkipClassification,
        reference: DispositionReference,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FixtureExecutionOutcome {
    NotExecuted,
    Completed {
        result: Box<CanonicalParserResult>,
    },
    ExpectationMismatch {
        result: Box<CanonicalParserResult>,
        surface: ExpectationSurface,
        diff: String,
    },
    UnsupportedExpectation {
        surface: ExpectationSurface,
    },
    UnsupportedFixtureSemantics {
        capability: FixtureCapability,
    },
    ExecutionFailed {
        class: ExecutionFailureClass,
        message: String,
    },
    InvariantFailed {
        result: Box<CanonicalParserResult>,
        failures: Vec<InvariantFailureCode>,
    },
    IncompleteObservation {
        result: Box<CanonicalParserResult>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispositionEvaluation {
    Pass,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureRunReport {
    fixture_id: FixtureId,
    repository_relative_path: String,
    disposition: DispositionEvaluation,
    result: Option<CanonicalParserResult>,
}

impl FixtureRunReport {
    pub(super) fn new(
        fixture_id: FixtureId,
        repository_relative_path: String,
        disposition: DispositionEvaluation,
        result: Option<CanonicalParserResult>,
    ) -> Self {
        Self {
            fixture_id,
            repository_relative_path,
            disposition,
            result,
        }
    }

    pub fn fixture_id(&self) -> &FixtureId {
        &self.fixture_id
    }

    pub fn repository_relative_path(&self) -> &str {
        &self.repository_relative_path
    }

    pub fn disposition(&self) -> DispositionEvaluation {
        self.disposition
    }

    pub fn result(&self) -> Option<&CanonicalParserResult> {
        self.result.as_ref()
    }
}
