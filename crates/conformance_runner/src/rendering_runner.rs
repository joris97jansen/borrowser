use std::convert::Infallible;
use std::path::Path;

use conformance_test_support::{
    ExecutionEnvironmentAssessment, FixtureFormat, InventoryRepository, ObservationSurface,
    ReferenceKind, ReferenceRelation, ValidatedFixture, discover_inventory,
    evaluate_execution_eligibility, load_expected_results,
};
use rendering_test_support::{
    CanonicalRenderingCapture, PairedRenderingCaptureOutcome, RenderingCaptureOutcome,
    RenderingComparisonFailure, RenderingDifferenceEvidenceFailure, RenderingDifferenceLocator,
    RenderingExecutionFailure, RenderingExecutionPhase, RenderingExecutionVariantId,
    RenderingFinalInvariantFailure, RenderingFirstDifference, RenderingIncompleteObservationReason,
    RenderingObservationOwner, RenderingObservationProfile, RenderingObservedExecutionOutcome,
    RenderingOracleComparison, RenderingOracleVerdict, RenderingProfileObservation,
    capture_paired_variant, compare_canonical_rendering_captures, evaluate_variant,
    load_variant_execution, materialize_rendering_first_difference,
};

use crate::metadata::{ag_expectation, eligibility_facts, metadata_facts};
use crate::model::*;
use crate::rendering_package::{
    RenderingPackageReconciliationError, reconcile_paired_rendering_package,
    reconcile_rendering_package,
};
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
    RenderingVariantObservedOutcome,
>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingOracleKind {
    AuthoredSnapshot,
    DocumentReference {
        reference_kind: ReferenceKind,
        relation: ReferenceRelation,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingRelationResult {
    SemanticPass,
    SemanticMismatch,
}

impl RenderingRelationResult {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::SemanticPass => "semantic-pass",
            Self::SemanticMismatch => "semantic-mismatch",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderingObservationSummary {
    pub profile: RenderingObservationProfile,
    pub bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingCaptureSummary {
    Complete {
        observations: Vec<RenderingObservationSummary>,
    },
    ExecutionFailure {
        phase: RenderingExecutionPhase,
        failure: RenderingExecutionFailure,
    },
    IncompleteObservation {
        phase: RenderingExecutionPhase,
        profile: RenderingObservationProfile,
        reason: RenderingIncompleteObservationReason,
        observations: Vec<RenderingObservationSummary>,
    },
    FinalInvariantFailure {
        phase: RenderingExecutionPhase,
        failure: RenderingFinalInvariantFailure,
        observations: Vec<RenderingObservationSummary>,
    },
}

impl RenderingCaptureSummary {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::Complete { .. } => "complete",
            Self::ExecutionFailure { .. } => "execution-failure",
            Self::IncompleteObservation { .. } => "incomplete-observation",
            Self::FinalInvariantFailure { .. } => "final-invariant-failure",
        }
    }

    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingReferenceObservedOutcome {
    Relation {
        test: RenderingCaptureSummary,
        reference: RenderingCaptureSummary,
        oracle: RenderingOracleVerdict,
        semantic: RenderingRelationResult,
        first_difference: Option<RenderingFirstDifference>,
    },
    CaptureTerminal {
        test: RenderingCaptureSummary,
        reference: RenderingCaptureSummary,
    },
    ComparisonInvariant {
        test: RenderingCaptureSummary,
        reference: RenderingCaptureSummary,
        failure: RenderingComparisonFailure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingVariantObservedOutcome {
    AuthoredSnapshot(RenderingObservedExecutionOutcome),
    DocumentReference(RenderingReferenceObservedOutcome),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderingVariantResult {
    pub variant: ExecutionVariantId<RenderingExecutionVariantId>,
    pub profiles: Vec<RenderingObservationProfile>,
    pub oracle: RenderingOracleKind,
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
    MissingExpectedResult {
        test_id: String,
    },
    Fixture(RenderingPackageReconciliationError),
    ReferenceEvidence {
        test_id: String,
        failure: RenderingDifferenceEvidenceFailure,
    },
    Reporting(crate::ReportBuildError),
}
impl RenderingRunError {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::Inventory(_) => "inventory",
            Self::ExpectedResults(_) => "expected-results",
            Self::MissingExpectedResult { .. } => "missing-expected-result",
            Self::Fixture(_) => "fixture",
            Self::ReferenceEvidence { .. } => "reference-evidence",
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
            Self::ReferenceEvidence { test_id, failure } => write!(
                f,
                "AG reference difference evidence failed for {test_id}: {failure}"
            ),
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
            Self::ReferenceEvidence { failure, .. } => Some(failure),
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
        let mut variants = Vec::new();
        if matches!(metadata.harness, Some(HarnessReadiness::Ready)) {
            match outer.format() {
                FixtureFormat::V2 => {
                    let package = reconcile_rendering_package(
                        repository_root,
                        outer,
                        owner(outer.observation()).expect("filtered rendering owner"),
                    )
                    .map_err(RenderingRunError::Fixture)?;
                    for variant in package.variants() {
                        let variant_id = variant.id();
                        let execution = if matches!(eligibility, Eligibility::Runnable) {
                            let execution = load_variant_execution(variant).map_err(|error| {
                                RenderingRunError::Fixture(
                                    RenderingPackageReconciliationError::Nested(error),
                                )
                            })?;
                            let outcome = evaluate_variant(&execution);
                            retain_snapshot_observations(&mut evidence_budget, outer, &outcome)?;
                            RenderingExecutionAttempt::Attempted {
                                outcome: RenderingVariantObservedOutcome::AuthoredSnapshot(outcome),
                            }
                        } else {
                            not_attempted()
                        };
                        let policy =
                            derive_rendering_policy(&expectation, &eligibility, &execution);
                        variants.push(RenderingVariantResult {
                            variant: ExecutionVariantId::new(variant_id),
                            profiles: package.profiles().to_vec(),
                            oracle: RenderingOracleKind::AuthoredSnapshot,
                            execution,
                            policy,
                        });
                    }
                }
                FixtureFormat::V3 => {
                    let reconciled = reconcile_paired_rendering_package(
                        repository_root,
                        outer,
                        owner(outer.observation()).expect("filtered rendering owner"),
                    )
                    .map_err(RenderingRunError::Fixture)?;
                    for variant in reconciled.package.variants() {
                        let variant_id = variant.id();
                        let execution = if matches!(eligibility, Eligibility::Runnable) {
                            let captures = capture_paired_variant(variant);
                            let outcome =
                                evaluate_reference_captures(captures, reconciled.relation)
                                    .map_err(|failure| RenderingRunError::ReferenceEvidence {
                                        test_id: outer.id().as_str().to_owned(),
                                        failure,
                                    })?;
                            retain_reference_evidence(&mut evidence_budget, outer, &outcome)?;
                            RenderingExecutionAttempt::Attempted {
                                outcome: RenderingVariantObservedOutcome::DocumentReference(
                                    outcome,
                                ),
                            }
                        } else {
                            not_attempted()
                        };
                        let policy =
                            derive_rendering_policy(&expectation, &eligibility, &execution);
                        variants.push(RenderingVariantResult {
                            variant: ExecutionVariantId::new(variant_id),
                            profiles: reconciled.package.profiles().to_vec(),
                            oracle: RenderingOracleKind::DocumentReference {
                                reference_kind: reconciled.reference_kind,
                                relation: reconciled.relation,
                            },
                            execution,
                            policy,
                        });
                    }
                }
                FixtureFormat::V1 => {
                    return Err(RenderingRunError::Fixture(
                        RenderingPackageReconciliationError::FixtureV2Required {
                            test_id: outer.id().as_str().to_owned(),
                        },
                    ));
                }
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

fn not_attempted() -> RenderingExecutionAttempt {
    RenderingExecutionAttempt::NotAttempted {
        reason: RenderingNotAttemptedReason::Eligibility,
        pre_attempt: None,
    }
}

fn retain_snapshot_observations(
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

fn evaluate_reference_captures(
    captures: PairedRenderingCaptureOutcome,
    relation: ReferenceRelation,
) -> Result<RenderingReferenceObservedOutcome, RenderingDifferenceEvidenceFailure> {
    evaluate_reference_captures_with(captures, relation, materialize_rendering_first_difference)
}

fn evaluate_reference_captures_with<MaterializeEvidence>(
    captures: PairedRenderingCaptureOutcome,
    relation: ReferenceRelation,
    materialize_evidence: MaterializeEvidence,
) -> Result<RenderingReferenceObservedOutcome, RenderingDifferenceEvidenceFailure>
where
    MaterializeEvidence:
        FnOnce(
            &CanonicalRenderingCapture,
            &CanonicalRenderingCapture,
            RenderingDifferenceLocator,
        ) -> Result<RenderingFirstDifference, RenderingDifferenceEvidenceFailure>,
{
    let comparison = match (&captures.test, &captures.reference) {
        (RenderingCaptureOutcome::Complete(test), RenderingCaptureOutcome::Complete(reference)) => {
            Some((
                test,
                reference,
                compare_canonical_rendering_captures(test, reference),
            ))
        }
        _ => None,
    };
    let Some((test_capture, reference_capture, comparison)) = comparison else {
        return Ok(RenderingReferenceObservedOutcome::CaptureTerminal {
            test: summarize_capture(captures.test),
            reference: summarize_capture(captures.reference),
        });
    };
    let (oracle, first_difference) = match comparison {
        Ok(RenderingOracleComparison::Equivalent) => (RenderingOracleVerdict::Equivalent, None),
        Ok(RenderingOracleComparison::Different { first_difference }) => (
            RenderingOracleVerdict::Different,
            Some(materialize_evidence(
                test_capture,
                reference_capture,
                first_difference,
            )?),
        ),
        Err(failure) => {
            return Ok(RenderingReferenceObservedOutcome::ComparisonInvariant {
                test: summarize_capture(captures.test),
                reference: summarize_capture(captures.reference),
                failure,
            });
        }
    };
    let semantic = match (oracle, relation) {
        (RenderingOracleVerdict::Equivalent, ReferenceRelation::Match)
        | (RenderingOracleVerdict::Different, ReferenceRelation::Mismatch) => {
            RenderingRelationResult::SemanticPass
        }
        (RenderingOracleVerdict::Different, ReferenceRelation::Match)
        | (RenderingOracleVerdict::Equivalent, ReferenceRelation::Mismatch) => {
            RenderingRelationResult::SemanticMismatch
        }
    };
    Ok(RenderingReferenceObservedOutcome::Relation {
        test: summarize_capture(captures.test),
        reference: summarize_capture(captures.reference),
        oracle,
        semantic,
        first_difference,
    })
}

fn summarize_capture(capture: RenderingCaptureOutcome) -> RenderingCaptureSummary {
    match capture {
        RenderingCaptureOutcome::Complete(capture) => RenderingCaptureSummary::Complete {
            observations: summarize_observations(capture),
        },
        RenderingCaptureOutcome::ExecutionFailure { phase, failure } => {
            RenderingCaptureSummary::ExecutionFailure { phase, failure }
        }
        RenderingCaptureOutcome::IncompleteObservation {
            phase,
            profile,
            reason,
            observations,
        } => RenderingCaptureSummary::IncompleteObservation {
            phase,
            profile,
            reason,
            observations: summarize_profile_observations(observations),
        },
        RenderingCaptureOutcome::FinalInvariantFailure {
            phase,
            failure,
            observations,
        } => RenderingCaptureSummary::FinalInvariantFailure {
            phase,
            failure,
            observations: summarize_profile_observations(observations),
        },
    }
}

fn summarize_observations(capture: CanonicalRenderingCapture) -> Vec<RenderingObservationSummary> {
    summarize_profile_observations(capture.observations)
}

fn summarize_profile_observations(
    observations: Vec<RenderingProfileObservation>,
) -> Vec<RenderingObservationSummary> {
    observations
        .into_iter()
        .map(|observation| RenderingObservationSummary {
            profile: observation.profile,
            bytes: observation.bytes.len(),
        })
        .collect()
}

fn retain_reference_evidence(
    budget: &mut RetainedEvidenceBudget,
    outer: &ValidatedFixture,
    outcome: &RenderingReferenceObservedOutcome,
) -> Result<(), RenderingRunError> {
    let RenderingReferenceObservedOutcome::Relation {
        first_difference: Some(difference),
        ..
    } = outcome
    else {
        return Ok(());
    };
    retain_reference_difference(budget, outer.id().as_str(), difference)
}

fn retain_reference_difference(
    budget: &mut RetainedEvidenceBudget,
    test_id: &str,
    difference: &RenderingFirstDifference,
) -> Result<(), RenderingRunError> {
    let bytes = crate::rendering_report::rendering_first_difference_evidence_bytes(difference)
        .map_err(RenderingRunError::Reporting)?;
    if bytes > crate::rendering_report::REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1 {
        return Err(RenderingRunError::Reporting(
            crate::ReportBuildError::MismatchDiagnosticTooLarge {
                test_id: test_id.to_owned(),
                actual: bytes,
                maximum: crate::rendering_report::REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1,
            },
        ));
    }
    budget
        .retain_mismatch(test_id, bytes)
        .map_err(RenderingRunError::Reporting)
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
        RenderingVariantObservedOutcome::AuthoredSnapshot(
            RenderingObservedExecutionOutcome::SemanticPass { .. },
        ) => ObservedPolicyClass::SemanticPass,
        RenderingVariantObservedOutcome::AuthoredSnapshot(
            RenderingObservedExecutionOutcome::SemanticMismatch { .. },
        ) => ObservedPolicyClass::SemanticMismatch,
        RenderingVariantObservedOutcome::DocumentReference(
            RenderingReferenceObservedOutcome::Relation {
                semantic: RenderingRelationResult::SemanticPass,
                ..
            },
        ) => ObservedPolicyClass::SemanticPass,
        RenderingVariantObservedOutcome::DocumentReference(
            RenderingReferenceObservedOutcome::Relation {
                semantic: RenderingRelationResult::SemanticMismatch,
                ..
            },
        ) => ObservedPolicyClass::SemanticMismatch,
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

    fn capture(bytes: &str) -> RenderingCaptureOutcome {
        RenderingCaptureOutcome::Complete(CanonicalRenderingCapture {
            variant: RenderingExecutionVariantId {
                environment: rendering_test_support::SyntheticTextMetricsV1::SyntheticTextMetricsV1,
                available_width_css_px: rendering_test_support::AvailableWidthCssPx::try_new(320)
                    .unwrap(),
            },
            observations: vec![RenderingProfileObservation {
                profile: RenderingObservationProfile::Paint(
                    rendering_test_support::PaintObservationProfile::PaintOperations,
                ),
                bytes: bytes.to_owned(),
            }],
        })
    }

    #[test]
    fn paired_relation_truth_table_is_separate_from_policy() {
        for (test, reference, relation, expected) in [
            (
                "same",
                "same",
                ReferenceRelation::Match,
                RenderingRelationResult::SemanticPass,
            ),
            (
                "test",
                "reference",
                ReferenceRelation::Match,
                RenderingRelationResult::SemanticMismatch,
            ),
            (
                "same",
                "same",
                ReferenceRelation::Mismatch,
                RenderingRelationResult::SemanticMismatch,
            ),
            (
                "test",
                "reference",
                ReferenceRelation::Mismatch,
                RenderingRelationResult::SemanticPass,
            ),
        ] {
            let outcome = evaluate_reference_captures(
                PairedRenderingCaptureOutcome {
                    test: capture(test),
                    reference: capture(reference),
                },
                relation,
            )
            .unwrap();
            assert!(matches!(
                outcome,
                RenderingReferenceObservedOutcome::Relation { semantic, .. }
                    if semantic == expected
            ));
        }
    }

    #[test]
    fn a_terminal_side_never_satisfies_mismatch() {
        let outcome = evaluate_reference_captures(
            PairedRenderingCaptureOutcome {
                test: RenderingCaptureOutcome::ExecutionFailure {
                    phase: RenderingExecutionPhase::HtmlDocumentParsing,
                    failure: RenderingExecutionFailure::StylesheetSemanticInputResourceLimited {
                        index: 0,
                    },
                },
                reference: capture("reference"),
            },
            ReferenceRelation::Mismatch,
        )
        .unwrap();
        assert!(matches!(
            outcome,
            RenderingReferenceObservedOutcome::CaptureTerminal { .. }
        ));
    }

    #[test]
    fn evidence_failure_is_tooling_failure_not_comparison_or_semantic_outcome() {
        let result = evaluate_reference_captures_with(
            PairedRenderingCaptureOutcome {
                test: capture("test"),
                reference: capture("reference"),
            },
            ReferenceRelation::Mismatch,
            |_, _, _| Err(RenderingDifferenceEvidenceFailure::ExcerptAllocation),
        );
        assert_eq!(
            result,
            Err(RenderingDifferenceEvidenceFailure::ExcerptAllocation)
        );
        let run_error = RenderingRunError::ReferenceEvidence {
            test_id: "synthetic-reference".to_owned(),
            failure: result.unwrap_err(),
        };
        assert_eq!(run_error.stable_label(), "reference-evidence");
        assert!(!run_error.to_string().contains("comparison-invariant"));
    }

    #[test]
    fn retained_reference_measurement_preserves_allocation_failure_as_reporting() {
        let outcome = evaluate_reference_captures(
            PairedRenderingCaptureOutcome {
                test: capture("test"),
                reference: capture("reference"),
            },
            ReferenceRelation::Mismatch,
        )
        .unwrap();
        let RenderingReferenceObservedOutcome::Relation {
            first_difference: Some(difference),
            ..
        } = outcome
        else {
            panic!("unequal complete captures must retain first-difference evidence");
        };
        let mut budget = RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
        let result = crate::report::with_forced_allocation_failure(|| {
            retain_reference_difference(&mut budget, "synthetic-reference", &difference)
        });
        assert!(matches!(
            result,
            Err(RenderingRunError::Reporting(
                crate::ReportBuildError::AllocationFailure
            ))
        ));
    }

    #[test]
    fn expected_fail_policy_applies_only_to_aggregate_semantic_assertion() {
        let mismatch = RenderingExecutionAttempt::Attempted {
            outcome: RenderingVariantObservedOutcome::AuthoredSnapshot(
                RenderingObservedExecutionOutcome::SemanticMismatch {
                    observations: vec![],
                    mismatches: vec![],
                },
            ),
        };
        assert_eq!(
            derive_rendering_policy(&expected_fail(), &Eligibility::Runnable, &mismatch),
            DerivedPolicyResult::ExpectedFail
        );
        let pass = RenderingExecutionAttempt::Attempted {
            outcome: RenderingVariantObservedOutcome::AuthoredSnapshot(
                RenderingObservedExecutionOutcome::SemanticPass {
                    observations: vec![],
                },
            ),
        };
        assert_eq!(
            derive_rendering_policy(&expected_fail(), &Eligibility::Runnable, &pass),
            DerivedPolicyResult::UnexpectedPass
        );
        let failure = RenderingExecutionAttempt::Attempted {
            outcome: RenderingVariantObservedOutcome::AuthoredSnapshot(
                RenderingObservedExecutionOutcome::ExecutionFailure {
                phase: rendering_test_support::RenderingExecutionPhase::HtmlDocumentParsing,
                failure: rendering_test_support::RenderingExecutionFailure::StylesheetSemanticInputResourceLimited {
                    index: 0,
                },
                },
            ),
        };
        assert_eq!(
            derive_rendering_policy(&expected_fail(), &Eligibility::Runnable, &failure),
            DerivedPolicyResult::UnexpectedOutcome
        );
    }
}
