use external_test_provenance::Sha256Digest;

pub const TREND_FORMAT_V1: &str = "borrowser-conformance-trend-v1";
pub const TREND_MAX_BYTES_V1: usize = 74_281_149;
pub(crate) const ADVISORY_CHANGE_CAPTURE: u8 = 1;
pub(crate) const ADVISORY_CHANGE_EVALUATION_PRESENCE: u8 = 2;
pub(crate) const ADVISORY_CHANGE_RESULT: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrendPopulation {
    LogicalCases,
    ExecutionVariants,
    AdvisoryComparisonPoints,
    BaselineNotes,
}

impl TrendPopulation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LogicalCases => "logical-cases",
            Self::ExecutionVariants => "execution-variants",
            Self::AdvisoryComparisonPoints => "advisory-comparison-points",
            Self::BaselineNotes => "baseline-notes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrendChangeKind {
    Added,
    Removed,
    Unchanged,
    Changed,
}

impl TrendChangeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Unchanged => "unchanged",
            Self::Changed => "changed",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrendPopulationCounts {
    pub old: u64,
    pub new: u64,
    pub added: u64,
    pub removed: u64,
    pub unchanged: u64,
    pub changed: u64,
}

impl TrendPopulationCounts {
    pub fn reconcile(self) -> Result<(), TrendError> {
        let old = self
            .removed
            .checked_add(self.unchanged)
            .and_then(|value| value.checked_add(self.changed))
            .ok_or(TrendError::Arithmetic)?;
        let new = self
            .added
            .checked_add(self.unchanged)
            .and_then(|value| value.checked_add(self.changed))
            .ok_or(TrendError::Arithmetic)?;
        if old != self.old || new != self.new {
            return Err(TrendError::Reconciliation);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrendRecord {
    pub(crate) kind: TrendChangeKind,
    pub(crate) key: Vec<u8>,
    pub(crate) old_fingerprint: Option<Sha256Digest>,
    pub(crate) new_fingerprint: Option<Sha256Digest>,
    pub(crate) advisory: Option<AdvisoryTrendExtension>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvisoryTrendExtension {
    pub(crate) old_evaluated: Option<bool>,
    pub(crate) new_evaluated: Option<bool>,
    pub(crate) change_mask: u8,
}

impl AdvisoryTrendExtension {
    pub const fn old_evaluated(self) -> Option<bool> {
        self.old_evaluated
    }

    pub const fn new_evaluated(self) -> Option<bool> {
        self.new_evaluated
    }

    pub const fn change_mask(self) -> u8 {
        self.change_mask
    }
}

impl TrendRecord {
    pub const fn kind(&self) -> TrendChangeKind {
        self.kind
    }
    pub fn key(&self) -> &[u8] {
        &self.key
    }
    pub const fn old_fingerprint(&self) -> Option<Sha256Digest> {
        self.old_fingerprint
    }
    pub const fn new_fingerprint(&self) -> Option<Sha256Digest> {
        self.new_fingerprint
    }
    pub const fn advisory_extension(&self) -> Option<AdvisoryTrendExtension> {
        self.advisory
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopulationTrend {
    pub(crate) population: TrendPopulation,
    pub(crate) counts: TrendPopulationCounts,
    pub(crate) records: Vec<TrendRecord>,
}

impl PopulationTrend {
    pub const fn population(&self) -> TrendPopulation {
        self.population
    }
    pub const fn counts(&self) -> TrendPopulationCounts {
        self.counts
    }
    pub fn records(&self) -> &[TrendRecord] {
        &self.records
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConformanceTrendV1 {
    pub(crate) old_baseline_sha256: Sha256Digest,
    pub(crate) new_baseline_sha256: Sha256Digest,
    pub(crate) inventory_scope: String,
    pub(crate) aggregate_contract: String,
    pub(crate) named_lane: String,
    pub(crate) environment_assessment: String,
    pub(crate) old_source_set_sha256: Sha256Digest,
    pub(crate) new_source_set_sha256: Sha256Digest,
    pub(crate) old_evaluation_scope: Vec<u8>,
    pub(crate) new_evaluation_scope: Vec<u8>,
    pub(crate) old_advisory_total: u64,
    pub(crate) old_advisory_evaluated: u64,
    pub(crate) old_advisory_unevaluated: u64,
    pub(crate) new_advisory_total: u64,
    pub(crate) new_advisory_evaluated: u64,
    pub(crate) new_advisory_unevaluated: u64,
    pub(crate) populations: [PopulationTrend; 4],
}

impl ConformanceTrendV1 {
    pub const fn old_baseline_sha256(&self) -> Sha256Digest {
        self.old_baseline_sha256
    }
    pub const fn new_baseline_sha256(&self) -> Sha256Digest {
        self.new_baseline_sha256
    }
    pub fn population(&self, population: TrendPopulation) -> &PopulationTrend {
        &self.populations[match population {
            TrendPopulation::LogicalCases => 0,
            TrendPopulation::ExecutionVariants => 1,
            TrendPopulation::AdvisoryComparisonPoints => 2,
            TrendPopulation::BaselineNotes => 3,
        }]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrendCompatibilityField {
    InventoryScope,
    AggregateContract,
    NamedLane,
    EnvironmentAssessment,
    ComponentVersions,
    AdvisoryTrackInvariant,
}

#[derive(Debug)]
pub enum TrendError {
    InvalidBaseline {
        input: TrendInputSide,
        source: super::super::baseline::BaselineError,
    },
    Incompatible(TrendCompatibilityField),
    Framing,
    TooLarge,
    LengthOverflow,
    PrematureEof,
    InvalidOption,
    InvalidUtf8,
    TrailingBytes,
    Arithmetic,
    Allocation,
    Reconciliation,
    DigestMismatch {
        input: TrendInputSide,
    },
    Input {
        input: TrendInputSide,
        source: external_test_provenance::SameObjectConfinedReadError,
    },
    Output(std::io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrendInputSide {
    Old,
    New,
}

impl std::fmt::Display for TrendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidBaseline { .. } => "trend baseline is invalid",
            Self::Incompatible(_) => "trend baselines are incompatible",
            Self::Framing => "trend framing is invalid",
            Self::TooLarge => "trend exceeds its exact byte ceiling",
            Self::LengthOverflow => "trend length arithmetic overflowed",
            Self::PrematureEof => "trend ended before a framed value was complete",
            Self::InvalidOption => "trend contains an invalid option discriminant",
            Self::InvalidUtf8 => "trend contains invalid UTF-8 in a string field",
            Self::TrailingBytes => "trend contains unconsumed trailing bytes",
            Self::Arithmetic => "trend arithmetic overflowed",
            Self::Allocation => "trend allocation failed",
            Self::Reconciliation => "trend accounting does not reconcile",
            Self::DigestMismatch { .. } => "trend baseline digest does not match",
            Self::Input { .. } => "trend baseline input failed",
            Self::Output(_) => "trend output failed",
        })
    }
}

impl std::error::Error for TrendError {}

impl From<super::super::binary_wire::BinaryWireError> for TrendError {
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
