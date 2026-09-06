use external_test_provenance::Sha256Digest;

use crate::{AggregateRun, ReconciledExternalAdvisoryEvidence, SelectedDomAdvisoryOperation};

pub const BASELINE_FORMAT_V1: &str = "borrowser-conformance-baseline-v1";
pub const BASELINE_MAX_BYTES_V1: usize = 47_219_283;

pub struct SealedBaselineEvidence<'e, 'run> {
    pub(super) run: &'run AggregateRun,
    pub(super) evidence: BaselineEvidence<'e, 'run>,
}

pub(super) enum BaselineEvidence<'e, 'run> {
    NotRequested(&'e ReconciledExternalAdvisoryEvidence<'run>),
    Selected(&'e SelectedDomAdvisoryOperation<'run>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaselineSealError {
    OriginatingRunMismatch,
}

#[derive(Debug)]
pub enum BaselineError {
    Seal(BaselineSealError),
    AggregateDetail(crate::AggregateReportBuildError),
    Framing,
    TooLarge,
    LengthOverflow,
    Allocation,
    PrematureEof,
    InvalidOption,
    InvalidUtf8,
    TrailingBytes,
    HistoricalDetail,
    HistoricalCapture(external_test_provenance::CaptureV1Error),
    InvalidFormat,
    InvalidVersion,
    InvalidOrdering,
    DuplicateIdentity,
    InvalidReference,
    InvalidEvaluation,
    InvalidEvidence,
    Output(std::io::Error),
}

impl std::fmt::Display for BaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Seal(_) => "baseline evidence does not originate from the aggregate run",
            Self::AggregateDetail(_) => "aggregate detail construction failed",
            Self::Framing => "baseline binary framing is invalid",
            Self::TooLarge => "baseline exceeds its exact byte ceiling",
            Self::LengthOverflow => "baseline length arithmetic overflowed",
            Self::Allocation => "baseline allocation failed",
            Self::PrematureEof => "baseline ended before a framed value was complete",
            Self::InvalidOption => "baseline contains an invalid option discriminant",
            Self::InvalidUtf8 => "baseline contains invalid UTF-8 in a string field",
            Self::TrailingBytes => "baseline contains unconsumed trailing bytes",
            Self::HistoricalDetail => "embedded aggregate detail is invalid",
            Self::HistoricalCapture(_) => "historical capture identity is invalid",
            Self::InvalidFormat => "baseline format is invalid",
            Self::InvalidVersion => "baseline version is unsupported",
            Self::InvalidOrdering => "baseline records are not canonically ordered",
            Self::DuplicateIdentity => "baseline contains a duplicate identity",
            Self::InvalidReference => "baseline contains an invalid reference",
            Self::InvalidEvaluation => "baseline evaluation coverage is invalid",
            Self::InvalidEvidence => "baseline advisory evidence is invalid",
            Self::Output(_) => "baseline output failed",
        })
    }
}
impl std::error::Error for BaselineError {}
impl From<super::super::binary_wire::BinaryWireError> for BaselineError {
    fn from(value: super::super::binary_wire::BinaryWireError) -> Self {
        use super::super::binary_wire::BinaryWireError as Error;
        match value {
            Error::Excess => Self::TooLarge,
            Error::Overflow => Self::LengthOverflow,
            Error::Allocation => Self::Allocation,
            Error::PrematureEof => Self::PrematureEof,
            Error::InvalidOption => Self::InvalidOption,
            Error::InvalidUtf8 => Self::InvalidUtf8,
            Error::TrailingBytes => Self::TrailingBytes,
        }
    }
}
impl From<HistoricalDetailError> for BaselineError {
    fn from(_: HistoricalDetailError) -> Self {
        Self::HistoricalDetail
    }
}
impl From<external_test_provenance::CaptureV1Error> for BaselineError {
    fn from(v: external_test_provenance::CaptureV1Error) -> Self {
        Self::HistoricalCapture(v)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalTrack {
    pub id: String,
    pub invariant: Vec<String>,
    pub canonical_record: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HistoricalAdvisoryPointKey {
    pub variant: HistoricalVariantKey,
    pub comparable: String,
    pub track_id: String,
}
#[derive(Clone, Debug)]
pub(crate) struct HistoricalAdvisoryPoint {
    pub key: HistoricalAdvisoryPointKey,
    pub capture_id: Sha256Digest,
    pub canonical_record: Vec<u8>,
    pub result: Option<Vec<u8>>,
}
#[derive(Clone, Debug)]
pub(crate) struct HistoricalNote {
    pub id: String,
    pub canonical_record: Vec<u8>,
}
#[derive(Clone, Debug)]
pub(crate) enum HistoricalEvaluationScope {
    None,
    Selected {
        variant: HistoricalVariantKey,
        comparable: String,
    },
    AllDeclared,
}

pub(crate) struct ValidatedHistoricalBaseline {
    pub(crate) exact_bytes: Vec<u8>,
    pub(crate) detail: HistoricalDetail,
    pub(crate) captures: Vec<external_test_provenance::HistoricalCaptureIdentityV1>,
    pub(crate) tracks: Vec<HistoricalTrack>,
    pub(crate) points: Vec<HistoricalAdvisoryPoint>,
    pub(crate) evaluation: HistoricalEvaluationScope,
    pub(crate) notes: Vec<HistoricalNote>,
}

impl ValidatedHistoricalBaseline {
    pub(crate) fn exact_bytes(&self) -> &[u8] {
        &self.exact_bytes
    }
    #[cfg(test)]
    pub(crate) fn advisory_evaluation_scope(&self) -> &'static str {
        match self.evaluation {
            HistoricalEvaluationScope::None => "none/not-requested",
            HistoricalEvaluationScope::Selected { .. } => "selected-variant-only/completed",
            HistoricalEvaluationScope::AllDeclared => "all-declared/completed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HistoricalVariantIdentity {
    Singleton,
    Rendering {
        environment: String,
        available_width_css_px: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HistoricalVariantKey {
    pub test_id: String,
    pub observation: String,
    pub variant: HistoricalVariantIdentity,
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalVariant {
    pub key: HistoricalVariantKey,
    pub canonical_record: Vec<u8>,
    pub comparison: crate::AggregateComparisonKind,
    pub selection: crate::aggregate::model::AggregateSelectionProjection,
    pub attempt: crate::aggregate::model::AggregateAttemptProjection,
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalLogicalCase {
    pub test_id: String,
    pub member_digest: Sha256Digest,
    pub canonical_metadata_prefix: Vec<u8>,
    pub variants: Vec<HistoricalVariant>,
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalDetail {
    pub inventory_scope: String,
    pub aggregate_contract: String,
    pub named_lane: String,
    pub environment_assessment: String,
    pub population_identity_contract: String,
    pub source_set_digest: Sha256Digest,
    pub cases: Vec<HistoricalLogicalCase>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoricalDetailError {
    InvalidUtf8,
    InvalidFraming,
    InvalidValue,
    InvalidVocabulary,
    InvalidState,
    NonCanonicalOrder,
    DuplicateIdentity,
    AccountingMismatch,
    MemberDigestMismatch,
    SourceSetDigestMismatch,
    Overflow,
    Allocation,
}
