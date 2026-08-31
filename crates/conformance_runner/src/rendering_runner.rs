use std::convert::Infallible;
use std::path::Path;

use conformance_test_support::{
    ExecutionEnvironmentAssessment, InventoryRepository, ObservationSurface, ValidatedFixture,
    discover_inventory, evaluate_execution_eligibility, load_expected_results,
};
use rendering_test_support::{
    RenderingExecutionVariantId, RenderingObservationOwner, RenderingObservationProfile,
    RenderingObservedExecutionOutcome, evaluate_variant, load_variant_execution,
};

use crate::metadata::{ag_expectation, eligibility_facts, metadata_facts};
use crate::model::*;
use crate::rendering_package::{RenderingPackageReconciliationError, reconcile_rendering_package};
use crate::rendering_report::rendering_mismatch_evidence_bytes;
use crate::report::{DEFAULT_REPORT_LIMITS, RetainedEvidenceBudget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingNotAttemptedReason {
    Eligibility,
}
impl RenderingNotAttemptedReason {
    pub const fn stable_label(self) -> &'static str {
        "eligibility"
    }
}

pub type RenderingExecutionAttempt = SubsystemExecutionAttempt<
    RenderingNotAttemptedReason,
    Infallible,
    RenderingObservedExecutionOutcome,
>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderingVariantResult {
    pub variant: ExecutionVariantId<RenderingExecutionVariantId>,
    pub profiles: Vec<RenderingObservationProfile>,
    pub execution: RenderingExecutionAttempt,
    pub policy: DerivedPolicyResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderingCaseResult {
    pub ag: AgCaseState,
    pub variants: Vec<RenderingVariantResult>,
}

#[derive(Debug)]
pub enum RenderingRunError {
    Inventory(conformance_test_support::InventoryErrors),
    ExpectedResults(conformance_test_support::ExpectedResultsErrors),
    MissingExpectedResult { test_id: String },
    Fixture(RenderingPackageReconciliationError),
    Reporting(crate::ReportBuildError),
}
impl RenderingRunError {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::Inventory(_) => "inventory",
            Self::ExpectedResults(_) => "expected-results",
            Self::MissingExpectedResult { .. } => "missing-expected-result",
            Self::Fixture(_) => "fixture",
            Self::Reporting(_) => "reporting",
        }
    }
}
impl std::fmt::Display for RenderingRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inventory(error) => write!(f, "{error}"),
            Self::ExpectedResults(error) => write!(f, "{error}"),
            Self::MissingExpectedResult { test_id } => {
                write!(f, "missing validated AG3 record for {test_id}")
            }
            Self::Fixture(error) => write!(f, "invalid AG rendering package: {error}"),
            Self::Reporting(error) => write!(f, "AG rendering report evidence failed: {error}"),
        }
    }
}
impl std::error::Error for RenderingRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inventory(error) => Some(error),
            Self::ExpectedResults(error) => Some(error),
            Self::Fixture(error) => Some(error),
            Self::Reporting(error) => Some(error),
            Self::MissingExpectedResult { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct RenderingRunSummary {
    cases: Vec<RenderingCaseResult>,
}
impl RenderingRunSummary {
    pub fn cases(&self) -> &[RenderingCaseResult] {
        &self.cases
    }
    pub fn has_unexpected_results(&self) -> bool {
        self.cases
            .iter()
            .flat_map(|case| &case.variants)
            .any(|variant| variant.policy.is_unexpected())
    }
}

pub fn run_repository_rendering_cases(
    repository_root: &Path,
) -> Result<RenderingRunSummary, RenderingRunError> {
    let fixture_root = repository_root.join("tests/conformance/fixtures");
    let inventory = discover_inventory(&InventoryRepository::new(repository_root, &fixture_root))
        .map_err(RenderingRunError::Inventory)?;
    let expected = load_expected_results(repository_root, &inventory)
        .map_err(RenderingRunError::ExpectedResults)?;
    let environment = ExecutionEnvironmentAssessment::empty();
    let mut cases = Vec::new();
    let mut evidence_budget = RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
    for outer in inventory
        .fixtures()
        .iter()
        .filter(|fixture| is_rendering(fixture.observation()))
    {
        let expected_view =
            expected
                .get(outer.id())
                .ok_or_else(|| RenderingRunError::MissingExpectedResult {
                    test_id: outer.id().as_str().to_owned(),
                })?;
        let metadata = metadata_facts(expected_view);
        let eligibility =
            eligibility_facts(evaluate_execution_eligibility(expected_view, &environment));
        let expectation = ag_expectation(expected_view);
        let package = if matches!(metadata.harness, Some(HarnessReadiness::Ready)) {
            Some(
                reconcile_rendering_package(
                    repository_root,
                    outer,
                    owner(outer.observation()).expect("filtered rendering owner"),
                )
                .map_err(RenderingRunError::Fixture)?,
            )
        } else {
            None
        };
        let mut variants = Vec::new();
        if let Some(package) = &package {
            for variant in package.variants() {
                let variant_id = variant.id();
                let execution = if matches!(eligibility, Eligibility::Runnable) {
                    let execution = load_variant_execution(variant).map_err(|error| {
                        RenderingRunError::Fixture(RenderingPackageReconciliationError::Nested(
                            error,
                        ))
                    })?;
                    let outcome = evaluate_variant(&execution);
                    retain_observations(&mut evidence_budget, outer, &outcome)?;
                    RenderingExecutionAttempt::Attempted { outcome }
                } else {
                    RenderingExecutionAttempt::NotAttempted {
                        reason: RenderingNotAttemptedReason::Eligibility,
                        pre_attempt: None,
                    }
                };
                let policy = derive_rendering_policy(&expectation, &eligibility, &execution);
                variants.push(RenderingVariantResult {
                    variant: ExecutionVariantId::new(variant_id),
                    profiles: package.profiles().to_vec(),
                    execution,
                    policy,
                });
            }
        }
        cases.push(RenderingCaseResult {
            ag: AgCaseState {
                test_id: outer.id().clone(),
                observation: outer.observation(),
                classification: metadata.classification,
                requirements: metadata.requirements,
                capability: metadata.capability,
                harness: metadata.harness,
                environment_requirements: metadata.environment_requirements,
                stability: metadata.stability,
                lane_exclusions: metadata.lane_exclusions,
                eligibility,
                expectation,
            },
            variants,
        });
    }
    cases.sort_by(|left, right| left.ag.test_id.cmp(&right.ag.test_id));
    Ok(RenderingRunSummary { cases })
}

fn retain_observations(
    budget: &mut RetainedEvidenceBudget,
    outer: &ValidatedFixture,
    outcome: &RenderingObservedExecutionOutcome,
) -> Result<(), RenderingRunError> {
    let observations = match outcome {
        RenderingObservedExecutionOutcome::SemanticPass { observations }
        | RenderingObservedExecutionOutcome::SemanticMismatch { observations, .. }
        | RenderingObservedExecutionOutcome::IncompleteObservation { observations, .. }
        | RenderingObservedExecutionOutcome::FinalInvariantFailure { observations, .. } => {
            observations.as_slice()
        }
        RenderingObservedExecutionOutcome::ExecutionFailure { .. } => &[],
    };
    for observation in observations {
        budget
            .retain_named_observation(
                outer.id().as_str(),
                observation.profile.stable_label(),
                observation.bytes.len(),
            )
            .map_err(RenderingRunError::Reporting)?;
    }
    if let RenderingObservedExecutionOutcome::SemanticMismatch { mismatches, .. } = outcome {
        let bytes = rendering_mismatch_evidence_bytes(mismatches).ok_or_else(|| {
            RenderingRunError::Reporting(crate::ReportBuildError::MismatchDiagnosticTooLarge {
                test_id: outer.id().as_str().to_owned(),
                actual: usize::MAX,
                maximum: DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes,
            })
        })?;
        budget
            .retain_mismatch(outer.id().as_str(), bytes)
            .map_err(RenderingRunError::Reporting)?;
    }
    Ok(())
}

fn derive_rendering_policy(
    expectation: &AgExpectation,
    eligibility: &Eligibility,
    execution: &RenderingExecutionAttempt,
) -> DerivedPolicyResult {
    if !matches!(eligibility, Eligibility::Runnable) {
        return if matches!(eligibility, Eligibility::NotYetEstablished { .. }) {
            DerivedPolicyResult::NotYetEstablished
        } else {
            DerivedPolicyResult::NotRun
        };
    }
    let RenderingExecutionAttempt::Attempted { outcome } = execution else {
        return DerivedPolicyResult::UnexpectedOutcome;
    };
    let observed = match outcome {
        RenderingObservedExecutionOutcome::SemanticPass { .. } => ObservedPolicyClass::SemanticPass,
        RenderingObservedExecutionOutcome::SemanticMismatch { .. } => {
            ObservedPolicyClass::SemanticMismatch
        }
        _ => ObservedPolicyClass::OtherTerminalOutcome,
    };
    derive_policy_from_class(expectation, eligibility, observed)
}

fn is_rendering(surface: ObservationSurface) -> bool {
    owner(surface).is_some()
}
fn owner(surface: ObservationSurface) -> Option<RenderingObservationOwner> {
    match surface {
        ObservationSurface::LayoutGeometry => Some(RenderingObservationOwner::Layout),
        ObservationSurface::PaintOperations => Some(RenderingObservationOwner::Paint),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conformance_test_support::ExpectedFailureClassification;

    fn expected_fail() -> AgExpectation {
        AgExpectation::ExpectedFail {
            failure: ExpectedFailureClassification::SemanticMismatch,
            reason: "known structural mismatch".to_owned(),
        }
    }

    #[test]
    fn expected_fail_policy_applies_only_to_aggregate_semantic_assertion() {
        let mismatch = RenderingExecutionAttempt::Attempted {
            outcome: RenderingObservedExecutionOutcome::SemanticMismatch {
                observations: vec![],
                mismatches: vec![],
            },
        };
        assert_eq!(
            derive_rendering_policy(&expected_fail(), &Eligibility::Runnable, &mismatch),
            DerivedPolicyResult::ExpectedFail
        );
        let pass = RenderingExecutionAttempt::Attempted {
            outcome: RenderingObservedExecutionOutcome::SemanticPass {
                observations: vec![],
            },
        };
        assert_eq!(
            derive_rendering_policy(&expected_fail(), &Eligibility::Runnable, &pass),
            DerivedPolicyResult::UnexpectedPass
        );
        let failure = RenderingExecutionAttempt::Attempted {
            outcome: RenderingObservedExecutionOutcome::ExecutionFailure {
                phase: rendering_test_support::RenderingExecutionPhase::HtmlDocumentParsing,
                failure: rendering_test_support::RenderingExecutionFailure::StylesheetSemanticInputResourceLimited {
                    index: 0,
                },
            },
        };
        assert_eq!(
            derive_rendering_policy(&expected_fail(), &Eligibility::Runnable, &failure),
            DerivedPolicyResult::UnexpectedOutcome
        );
    }
}
