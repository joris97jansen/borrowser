use conformance_test_support::{
    EngineCapabilityKind, EnvironmentRequirementKind, ExpectedFailureClassification,
    HarnessLimitationKind, LanePolicyScope, ObservationSurface, RequirementTag, TestId,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SingletonExecutionVariant {
    #[default]
    Singleton,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExecutionVariantId<V>(V);

impl<V> ExecutionVariantId<V> {
    pub const fn new(value: V) -> Self {
        Self(value)
    }

    pub const fn value(&self) -> &V {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParserObservationProfile {
    HtmlTokenizer,
    HtmlTreeConstruction,
    DomTree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParserObservationSurface {
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
pub enum ClassificationCompleteness {
    Classified,
    NotYetClassified { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityAvailability {
    Available,
    Unavailable { missing: Vec<ReasonedCapability> },
    NotYetEstablished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReasonedCapability {
    pub kind: EngineCapabilityKind,
    pub feature: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HarnessReadiness {
    Ready,
    NotReady {
        limitations: Vec<ReasonedHarnessLimitation>,
    },
    NotYetEstablished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReasonedHarnessLimitation {
    pub kind: HarnessLimitationKind,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReasonedEnvironmentRequirement {
    pub kind: EnvironmentRequirementKind,
    pub profile: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stability {
    Stable,
    Flaky { reason: String },
    NotYetEstablished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReasonedLaneExclusion {
    pub policy: LanePolicyScope,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgExpectation {
    ExpectedPass,
    ExpectedFail {
        failure: ExpectedFailureClassification,
        reason: String,
    },
    NotEstablished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Eligibility {
    Runnable,
    NotRunnable {
        blockers: Vec<EligibilityFact>,
        unresolved: Vec<EligibilityFact>,
    },
    NotYetEstablished {
        unresolved: Vec<EligibilityFact>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EligibilityFact {
    EngineCapability {
        kind: EngineCapabilityKind,
        feature: Option<String>,
        reason: String,
    },
    Harness {
        kind: HarnessLimitationKind,
        reason: String,
    },
    Environment {
        kind: EnvironmentRequirementKind,
        profile: String,
        requirement_reason: String,
        assessment_reason: String,
    },
    Classification {
        reason: String,
    },
    EngineCapabilityAvailability,
    HarnessReadiness,
    EnvironmentRequirement {
        kind: EnvironmentRequirementKind,
        profile: String,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotAttemptedReason {
    Eligibility,
    AePreExecutionEvaluation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalizedExecutionFailureCategory {
    SnapshotRead,
    SnapshotFormat,
    ParserObservation,
    FixtureExecutionResourceExhaustion,
    ValidatedFixtureInvariant,
    LegacyTokenizerDriver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalizedIncompleteObservationReason {
    LegacyNonAuthoritativeObservation,
    StorageLimitExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreAttemptEvaluationOutcome {
    NotExecutedByAe {
        classification: String,
    },
    UnsupportedFixtureSemantics {
        capability: String,
    },
    UnsupportedExpectation {
        surface: ParserObservationSurface,
    },
    EvaluationFailure {
        category: NormalizedExecutionFailureCategory,
        identity: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservedExecutionOutcome {
    SemanticPass,
    ExpectationMismatch {
        strategy: Option<String>,
        surface: ParserObservationSurface,
        difference: String,
    },
    ParityMismatch {
        strategy: String,
        surface: ParserObservationSurface,
        difference: String,
    },
    ExecutionFailure {
        category: NormalizedExecutionFailureCategory,
        identity: String,
    },
    IncompleteObservation {
        strategy: Option<String>,
        surface: Option<ParserObservationSurface>,
        reason: NormalizedIncompleteObservationReason,
        retained: Option<usize>,
        dropped: Option<u64>,
    },
    FinalInvariantFailure {
        strategy: Option<String>,
        first: Option<String>,
        count: u8,
    },
}

/// AG1's attempt/outcome invariant is structural: a terminal observed parser
/// execution outcome can only be stored in the `Attempted` branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionAttempt {
    NotAttempted {
        reason: NotAttemptedReason,
        pre_attempt: Option<PreAttemptEvaluationOutcome>,
    },
    Attempted {
        outcome: ObservedExecutionOutcome,
    },
}

/// Reusable AG1 structural attempt boundary for subsystem adapters whose
/// pre-attempt and terminal outcome types are not parser semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubsystemExecutionAttempt<Reason, PreAttempt, Outcome> {
    NotAttempted {
        reason: Reason,
        pre_attempt: Option<PreAttempt>,
    },
    Attempted {
        outcome: Outcome,
    },
}

impl<Reason, PreAttempt, Outcome> SubsystemExecutionAttempt<Reason, PreAttempt, Outcome> {
    pub fn observed_outcome(&self) -> Option<&Outcome> {
        match self {
            Self::NotAttempted { .. } => None,
            Self::Attempted { outcome } => Some(outcome),
        }
    }
}

impl ExecutionAttempt {
    #[cfg(any(feature = "html-parser", test))]
    pub(crate) const fn eligibility_blocked() -> Self {
        Self::NotAttempted {
            reason: NotAttemptedReason::Eligibility,
            pre_attempt: None,
        }
    }

    pub fn observed_outcome(&self) -> Option<&ObservedExecutionOutcome> {
        match self {
            Self::NotAttempted { .. } => None,
            Self::Attempted { outcome } => Some(outcome),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationArtifact {
    pub surface: ParserObservationSurface,
    pub format: String,
    pub bytes: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalizedAeDispositionContext {
    MatchedPass,
    MatchedSkip,
    UnexpectedOutcome,
    IncompleteObservation,
    Xpass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedPolicyResult {
    ExpectedPass,
    UnexpectedFail,
    ExpectedFail,
    UnexpectedPass,
    NotRun,
    NotYetEstablished,
    UnexpectedOutcome,
}

/// The subsystem-neutral AG state carried alongside every normalized result.
///
/// This deliberately keeps metadata establishment, eligibility, expectation,
/// and derived policy as orthogonal dimensions. Subsystem observations and
/// terminal outcomes are not stored here and remain lossless typed values in
/// their owning adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgCaseState {
    pub test_id: TestId,
    pub observation: ObservationSurface,
    pub classification: ClassificationCompleteness,
    pub requirements: Vec<RequirementTag>,
    pub capability: Option<CapabilityAvailability>,
    pub harness: Option<HarnessReadiness>,
    pub environment_requirements: Vec<ReasonedEnvironmentRequirement>,
    pub stability: Option<Stability>,
    pub lane_exclusions: Vec<ReasonedLaneExclusion>,
    pub eligibility: Eligibility,
    pub expectation: AgExpectation,
}

/// Policy-facing projection of a lossless subsystem terminal outcome.
///
/// Adapters derive this value from their closed outcome enums; it never
/// replaces or rewrites the observed subsystem result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedPolicyClass {
    SemanticPass,
    SemanticMismatch,
    OtherTerminalOutcome,
}

impl DerivedPolicyResult {
    pub const fn is_unexpected(self) -> bool {
        matches!(
            self,
            Self::UnexpectedFail | Self::UnexpectedPass | Self::UnexpectedOutcome
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedCaseResult {
    pub ag: AgCaseState,
    pub variant: ExecutionVariantId<SingletonExecutionVariant>,
    pub profile: ParserObservationProfile,
    pub execution: ExecutionAttempt,
    pub observations: Vec<ObservationArtifact>,
    pub ae_disposition: Option<NormalizedAeDispositionContext>,
    pub policy: DerivedPolicyResult,
}

#[cfg(any(feature = "html-parser", test))]
pub(crate) fn derive_policy(
    expectation: &AgExpectation,
    eligibility: &Eligibility,
    execution: &ExecutionAttempt,
) -> DerivedPolicyResult {
    if !matches!(eligibility, Eligibility::Runnable) {
        return if matches!(eligibility, Eligibility::NotYetEstablished { .. }) {
            DerivedPolicyResult::NotYetEstablished
        } else {
            DerivedPolicyResult::NotRun
        };
    }
    let ExecutionAttempt::Attempted { outcome } = execution else {
        return DerivedPolicyResult::UnexpectedOutcome;
    };
    derive_policy_from_class(
        expectation,
        eligibility,
        match outcome {
            ObservedExecutionOutcome::SemanticPass => ObservedPolicyClass::SemanticPass,
            ObservedExecutionOutcome::ExpectationMismatch { .. }
            | ObservedExecutionOutcome::ParityMismatch { .. } => {
                ObservedPolicyClass::SemanticMismatch
            }
            ObservedExecutionOutcome::ExecutionFailure { .. }
            | ObservedExecutionOutcome::IncompleteObservation { .. }
            | ObservedExecutionOutcome::FinalInvariantFailure { .. } => {
                ObservedPolicyClass::OtherTerminalOutcome
            }
        },
    )
}

#[cfg(any(feature = "html-parser", feature = "css", feature = "rendering", test))]
pub(crate) fn derive_policy_from_class(
    expectation: &AgExpectation,
    eligibility: &Eligibility,
    observed: ObservedPolicyClass,
) -> DerivedPolicyResult {
    if !matches!(eligibility, Eligibility::Runnable) {
        return if matches!(eligibility, Eligibility::NotYetEstablished { .. }) {
            DerivedPolicyResult::NotYetEstablished
        } else {
            DerivedPolicyResult::NotRun
        };
    }
    let semantic_pass = observed == ObservedPolicyClass::SemanticPass;
    let semantic_mismatch = observed == ObservedPolicyClass::SemanticMismatch;
    match expectation {
        AgExpectation::ExpectedPass => {
            if semantic_pass {
                DerivedPolicyResult::ExpectedPass
            } else if semantic_mismatch {
                DerivedPolicyResult::UnexpectedFail
            } else {
                DerivedPolicyResult::UnexpectedOutcome
            }
        }
        AgExpectation::ExpectedFail { failure, .. } => match failure {
            ExpectedFailureClassification::SemanticMismatch => {
                if semantic_mismatch {
                    DerivedPolicyResult::ExpectedFail
                } else if semantic_pass {
                    DerivedPolicyResult::UnexpectedPass
                } else {
                    DerivedPolicyResult::UnexpectedOutcome
                }
            }
        },
        AgExpectation::NotEstablished => DerivedPolicyResult::NotYetEstablished,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xfail_and_xpass_are_derived_from_typed_semantic_mismatch() {
        let eligible = Eligibility::Runnable;
        let xfail = AgExpectation::ExpectedFail {
            failure: ExpectedFailureClassification::SemanticMismatch,
            reason: "typed synthetic policy proof".to_owned(),
        };
        let mismatch = ExecutionAttempt::Attempted {
            outcome: ObservedExecutionOutcome::ExpectationMismatch {
                strategy: Some("whole".to_owned()),
                surface: ParserObservationSurface::Tree,
                difference: "typed mismatch".to_owned(),
            },
        };
        assert_eq!(
            derive_policy(&xfail, &eligible, &mismatch),
            DerivedPolicyResult::ExpectedFail
        );
        assert_eq!(
            derive_policy(
                &xfail,
                &eligible,
                &ExecutionAttempt::Attempted {
                    outcome: ObservedExecutionOutcome::SemanticPass,
                },
            ),
            DerivedPolicyResult::UnexpectedPass
        );
    }

    #[test]
    fn non_runnable_has_no_observed_execution_outcome() {
        let eligibility = Eligibility::NotRunnable {
            blockers: vec![],
            unresolved: vec![],
        };
        let execution = ExecutionAttempt::eligibility_blocked();
        assert_eq!(execution.observed_outcome(), None);
        assert_eq!(
            derive_policy(&AgExpectation::ExpectedPass, &eligibility, &execution),
            DerivedPolicyResult::NotRun
        );
    }
}
