use conformance_test_support::{
    LanePolicyScope, ObservationSurface, ReferenceKind, ReferenceRelation, SubsystemOwner, TestId,
    ValidatedFixture,
};
use rendering_test_support::RenderingExecutionVariantId;

use crate::{
    AgCaseState, CssCaseResult, DerivedPolicyResult, ExecutionVariantId, NormalizedCaseResult,
    RenderingVariantResult, SingletonExecutionVariant,
};

use super::AggregateAccounting;

/// One named AG lane execution request. Stage 1 deliberately uses only the
/// existing empty execution-environment assessment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateExecutionRequest {
    pub lane: LanePolicyScope,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AggregateExecutionVariantId {
    Singleton(ExecutionVariantId<SingletonExecutionVariant>),
    Rendering(ExecutionVariantId<RenderingExecutionVariantId>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AggregateVariantKey {
    pub test_id: TestId,
    pub observation: ObservationSurface,
    pub variant: AggregateExecutionVariantId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaneSelection {
    NotApplicable,
    Selected {
        lane: LanePolicyScope,
    },
    Excluded {
        lane: LanePolicyScope,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AggregateTerminalOutcome {
    SemanticPass,
    SemanticFail,
    ExecutionFailure,
    ResourceFailure,
    IncompleteObservation,
    InvariantFailure,
    /// Reserved by the AG9 contract. No Stage 1 adapter produces this value.
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateNotAttemptedReason {
    Eligibility,
    LaneExcluded,
    ParserPreAttemptEvaluation,
    CssFragmentCapabilityUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateExecutionAttempt {
    NotAttempted { reason: AggregateNotAttemptedReason },
    Attempted { outcome: AggregateTerminalOutcome },
}

impl AggregateExecutionAttempt {
    pub const fn terminal_outcome(&self) -> Option<AggregateTerminalOutcome> {
        match self {
            Self::NotAttempted { .. } => None,
            Self::Attempted { outcome } => Some(*outcome),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AggregateComparisonKind {
    AuthoredExpectedObservation,
    StaticDocumentReference {
        reference_kind: ReferenceKind,
        relation: ReferenceRelation,
    },
}

/// Lossless subsystem result retained alongside the aggregate projection.
/// Aggregate outcomes never replace these authoritative typed values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateSubsystemResult {
    Parser(NormalizedCaseResult),
    Css(CssCaseResult),
    Rendering(RenderingVariantResult),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateVariantResult {
    pub key: AggregateVariantKey,
    pub selection: LaneSelection,
    pub execution: AggregateExecutionAttempt,
    pub policy: DerivedPolicyResult,
    pub comparison: AggregateComparisonKind,
    pub subsystem: AggregateSubsystemResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateCaseResult {
    pub fixture: ValidatedFixture,
    pub owner: SubsystemOwner,
    pub ag: AgCaseState,
    pub variants: Vec<AggregateVariantResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateRun {
    request: AggregateExecutionRequest,
    cases: Vec<AggregateCaseResult>,
    accounting: AggregateAccounting,
}

impl AggregateRun {
    pub(crate) fn validated(
        request: AggregateExecutionRequest,
        cases: Vec<AggregateCaseResult>,
        accounting: AggregateAccounting,
    ) -> Self {
        Self {
            request,
            cases,
            accounting,
        }
    }

    pub const fn request(&self) -> AggregateExecutionRequest {
        self.request
    }

    pub fn cases(&self) -> &[AggregateCaseResult] {
        &self.cases
    }

    pub const fn accounting(&self) -> &AggregateAccounting {
        &self.accounting
    }
}
