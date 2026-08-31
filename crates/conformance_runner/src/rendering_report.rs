use std::io::Write;

use rendering_test_support::RenderingObservedExecutionOutcome;

use crate::model::*;
use crate::rendering_runner::{RenderingCaseResult, RenderingExecutionAttempt};
use crate::report::{
    BoundedWriter, DEFAULT_REPORT_LIMITS, ReportBuildError, ReportLimits, ReportPublicationError,
};

pub const RENDERING_REPORT_FORMAT_V1: &str = "borrowser-conformance-rendering-report-v1";

pub fn build_rendering_report(cases: &[RenderingCaseResult]) -> Result<Vec<u8>, ReportBuildError> {
    build_rendering_report_with_limits(cases, DEFAULT_REPORT_LIMITS)
}

fn build_rendering_report_with_limits(
    cases: &[RenderingCaseResult],
    limits: ReportLimits,
) -> Result<Vec<u8>, ReportBuildError> {
    validate_rendering_report_limits(cases, limits)?;
    let execution_variant_count = cases.iter().try_fold(0usize, |count, case| {
        count
            .checked_add(case.variants.len())
            .ok_or(ReportBuildError::ReportTooLarge {
                maximum: limits.total_bytes,
            })
    })?;
    let mut writer = BoundedWriter::new(limits.total_bytes)?;
    writer.line("format", RENDERING_REPORT_FORMAT_V1)?;
    writer.line("granularity", "execution-variant")?;
    writer.number("logical-case-count", cases.len())?;
    writer.number("execution-variant-count", execution_variant_count)?;
    for case in cases {
        writer.raw("\nBEGIN logical-case\n")?;
        writer.line("test-id", case.ag.test_id.as_str())?;
        writer.line("observation", case.ag.observation.as_str())?;
        write_logical_ag(&mut writer, &case.ag)?;
        writer.number("variant-count", case.variants.len())?;
        for variant in &case.variants {
            let id = variant.variant.value();
            writer.raw("BEGIN variant\n")?;
            writer.line("environment", id.stable_environment_label())?;
            writer.number(
                "available-width-css-px",
                id.available_width_css_px.get() as usize,
            )?;
            writer.list(
                "profiles",
                variant
                    .profiles
                    .iter()
                    .map(|profile| profile.stable_label()),
            )?;
            match &variant.execution {
                RenderingExecutionAttempt::NotAttempted { reason, .. } => {
                    writer.line("attempt", "not-attempted")?;
                    writer.line("not-attempted-reason", reason.stable_label())?;
                }
                RenderingExecutionAttempt::Attempted { outcome } => {
                    writer.line("attempt", "attempted")?;
                    write_outcome(&mut writer, outcome)?;
                }
            }
            writer.line("policy", policy_name(variant.policy))?;
            writer.raw("END variant\n")?;
        }
        writer.raw("END logical-case\n")?;
    }
    Ok(writer.finish())
}

fn write_logical_ag(writer: &mut BoundedWriter, ag: &AgCaseState) -> Result<(), ReportBuildError> {
    match &ag.classification {
        ClassificationCompleteness::Classified => writer.line("classification", "classified")?,
        ClassificationCompleteness::NotYetClassified { reason } => {
            writer.line("classification", "not-yet-classified")?;
            writer.line("classification-reason", reason)?;
        }
    }
    writer.list(
        "requirements",
        ag.requirements.iter().map(|item| item.as_str()),
    )?;
    match &ag.capability {
        None => writer.line("engine", "not-classified")?,
        Some(CapabilityAvailability::Available) => writer.line("engine", "available")?,
        Some(CapabilityAvailability::NotYetEstablished) => {
            writer.line("engine", "not-yet-established")?
        }
        Some(CapabilityAvailability::Unavailable { missing }) => {
            writer.line("engine", "unavailable")?;
            for item in missing {
                writer.raw("BEGIN engine-missing\n")?;
                writer.line("kind", item.kind.as_str())?;
                writer.optional_line("feature", item.feature.as_deref())?;
                writer.line("reason", &item.reason)?;
                writer.raw("END engine-missing\n")?;
            }
        }
    }
    match &ag.harness {
        None => writer.line("harness", "not-classified")?,
        Some(HarnessReadiness::Ready) => writer.line("harness", "ready")?,
        Some(HarnessReadiness::NotYetEstablished) => {
            writer.line("harness", "not-yet-established")?
        }
        Some(HarnessReadiness::NotReady { limitations }) => {
            writer.line("harness", "not-ready")?;
            for item in limitations {
                writer.raw("BEGIN harness-limitation\n")?;
                writer.line("kind", item.kind.as_str())?;
                writer.line("reason", &item.reason)?;
                writer.raw("END harness-limitation\n")?;
            }
        }
    }
    for item in &ag.environment_requirements {
        writer.raw("BEGIN environment-requirement\n")?;
        writer.line("kind", item.kind.as_str())?;
        writer.line("profile", &item.profile)?;
        writer.line("reason", &item.reason)?;
        writer.raw("END environment-requirement\n")?;
    }
    match &ag.expectation {
        AgExpectation::ExpectedPass => writer.line("expectation", "expected-pass")?,
        AgExpectation::ExpectedFail { failure, reason } => {
            writer.line("expectation", "expected-fail")?;
            writer.line("expected-failure", failure.as_str())?;
            writer.line("expected-failure-reason", reason)?;
        }
        AgExpectation::NotEstablished => writer.line("expectation", "not-established")?,
    }
    writer.line(
        "stability",
        match &ag.stability {
            Some(Stability::Stable) => "stable",
            Some(Stability::Flaky { .. }) => "flaky",
            Some(Stability::NotYetEstablished) => "not-yet-established",
            None => "not-classified",
        },
    )?;
    if let Some(Stability::Flaky { reason }) = &ag.stability {
        writer.line("stability-reason", reason)?;
    }
    for item in &ag.lane_exclusions {
        writer.raw("BEGIN lane-exclusion\n")?;
        writer.line("policy", item.policy.as_str())?;
        writer.line("reason", &item.reason)?;
        writer.raw("END lane-exclusion\n")?;
    }
    match &ag.eligibility {
        Eligibility::Runnable => writer.line("eligibility", "runnable")?,
        Eligibility::NotRunnable {
            blockers,
            unresolved,
        } => {
            writer.line("eligibility", "not-runnable")?;
            for fact in blockers {
                write_eligibility_fact(writer, "blocker", fact)?;
            }
            for fact in unresolved {
                write_eligibility_fact(writer, "unresolved", fact)?;
            }
        }
        Eligibility::NotYetEstablished { unresolved } => {
            writer.line("eligibility", "not-yet-established")?;
            for fact in unresolved {
                write_eligibility_fact(writer, "unresolved", fact)?;
            }
        }
    }
    Ok(())
}

fn write_eligibility_fact(
    writer: &mut BoundedWriter,
    role: &str,
    fact: &EligibilityFact,
) -> Result<(), ReportBuildError> {
    writer.raw("BEGIN eligibility-fact\n")?;
    writer.line("role", role)?;
    match fact {
        EligibilityFact::EngineCapability {
            kind,
            feature,
            reason,
        } => {
            writer.line("kind", kind.as_str())?;
            writer.optional_line("feature", feature.as_deref())?;
            writer.line("reason", reason)?;
        }
        EligibilityFact::Harness { kind, reason } => {
            writer.line("kind", kind.as_str())?;
            writer.line("reason", reason)?;
        }
        EligibilityFact::Environment {
            kind,
            profile,
            requirement_reason,
            assessment_reason,
        } => {
            writer.line("kind", kind.as_str())?;
            writer.line("profile", profile)?;
            writer.line("requirement-reason", requirement_reason)?;
            writer.line("assessment-reason", assessment_reason)?;
        }
        EligibilityFact::Classification { reason } => {
            writer.line("kind", "classification")?;
            writer.line("reason", reason)?;
        }
        EligibilityFact::EngineCapabilityAvailability => {
            writer.line("kind", "engine-capability-availability")?
        }
        EligibilityFact::HarnessReadiness => writer.line("kind", "harness-readiness")?,
        EligibilityFact::EnvironmentRequirement {
            kind,
            profile,
            reason,
        } => {
            writer.line("kind", kind.as_str())?;
            writer.line("profile", profile)?;
            writer.line("reason", reason)?;
        }
    }
    writer.raw("END eligibility-fact\n")
}

pub fn build_and_write_rendering_report(
    cases: &[RenderingCaseResult],
    output: &mut impl Write,
) -> Result<(), ReportPublicationError> {
    let report = build_rendering_report(cases).map_err(ReportPublicationError::Build)?;
    output
        .write_all(&report)
        .map_err(ReportPublicationError::OutputWrite)
}

fn write_outcome(
    writer: &mut BoundedWriter,
    outcome: &RenderingObservedExecutionOutcome,
) -> Result<(), ReportBuildError> {
    let (label, observations) = match outcome {
        RenderingObservedExecutionOutcome::SemanticPass { observations } => {
            ("semantic-pass", observations.as_slice())
        }
        RenderingObservedExecutionOutcome::SemanticMismatch {
            observations,
            mismatches,
        } => {
            writer.line("observed", "semantic-mismatch")?;
            writer.number("mismatch-count", mismatches.len())?;
            for mismatch in mismatches {
                writer.raw("BEGIN mismatch\n")?;
                writer.line("profile", mismatch.profile.stable_label())?;
                writer.number(
                    "first-mismatching-line",
                    mismatch.difference.first_mismatching_line,
                )?;
                writer.number("expected-bytes", mismatch.difference.expected_bytes)?;
                writer.number("actual-bytes", mismatch.difference.actual_bytes)?;
                writer.raw("END mismatch\n")?;
            }
            ("", observations.as_slice())
        }
        RenderingObservedExecutionOutcome::ExecutionFailure { phase, failure } => {
            writer.line("observed", "execution-failure")?;
            writer.line("phase", phase.stable_label())?;
            writer.line("failure", failure.stable_label())?;
            write_execution_failure_evidence(writer, failure)?;
            return Ok(());
        }
        RenderingObservedExecutionOutcome::IncompleteObservation {
            phase,
            profile,
            reason,
            observations,
        } => {
            writer.line("observed", "incomplete-observation")?;
            writer.line("phase", phase.stable_label())?;
            writer.line("profile", profile.stable_label())?;
            writer.line("reason", reason.stable_label())?;
            if let rendering_test_support::RenderingIncompleteObservationReason::ByteLimitExceeded {
                maximum,
                observed_at_least,
            } = reason
            {
                writer.number("maximum-bytes", *maximum)?;
                writer.number("observed-at-least-bytes", *observed_at_least)?;
            }
            ("", observations.as_slice())
        }
        RenderingObservedExecutionOutcome::FinalInvariantFailure {
            phase,
            failure,
            observations,
        } => {
            writer.line("observed", "final-invariant-failure")?;
            writer.line("phase", phase.stable_label())?;
            writer.line("failure", failure.stable_label())?;
            ("", observations.as_slice())
        }
    };
    if !label.is_empty() {
        writer.line("observed", label)?;
    }
    writer.number("observation-count", observations.len())?;
    for observation in observations {
        writer.raw("BEGIN observation\n")?;
        writer.line("profile", observation.profile.stable_label())?;
        writer.multiline("bytes", &observation.bytes)?;
        writer.raw("END observation\n")?;
    }
    Ok(())
}

fn write_execution_failure_evidence(
    writer: &mut BoundedWriter,
    failure: &rendering_test_support::RenderingExecutionFailure,
) -> Result<(), ReportBuildError> {
    use rendering_test_support::RenderingExecutionFailure;
    match failure {
        RenderingExecutionFailure::HtmlParser(error) => {
            writer.line("failure-identity", error.stable_label())?;
            writer.line("detail", &error.to_string())
        }
        RenderingExecutionFailure::CssRuleCollection(error) => {
            writer.line("failure-identity", error.stable_label())?;
            writer.line("detail", &error.to_string())
        }
        RenderingExecutionFailure::CssStyleResolution(error) => {
            writer.line("failure-identity", error.stable_label())?;
            writer.line("detail", &error.to_string())
        }
        RenderingExecutionFailure::CssComputedStyle(error)
        | RenderingExecutionFailure::CssStyleTree(error) => {
            writer.line("failure-identity", error.stable_label())?;
            writer.line("detail", &error.to_string())
        }
        RenderingExecutionFailure::StylesheetSemanticInputResourceLimited { index } => {
            writer.number("stylesheet-index", *index)
        }
        RenderingExecutionFailure::StorageAllocation { storage } => {
            writer.line("storage", storage.stable_label())
        }
        RenderingExecutionFailure::HtmlSemanticInputResourceLimited { degradations } => {
            writer.number("degradation-count", degradations.len())?;
            for reason in degradations.reasons() {
                writer.line("degradation", reason.stable_label())?;
            }
            Ok(())
        }
    }
}

fn validate_rendering_report_limits(
    cases: &[RenderingCaseResult],
    limits: ReportLimits,
) -> Result<(), ReportBuildError> {
    for case in cases {
        for variant in &case.variants {
            let outcome = match &variant.execution {
                RenderingExecutionAttempt::NotAttempted { .. } => continue,
                RenderingExecutionAttempt::Attempted { outcome } => outcome,
            };
            let observations = match outcome {
                RenderingObservedExecutionOutcome::SemanticPass { observations }
                | RenderingObservedExecutionOutcome::SemanticMismatch { observations, .. }
                | RenderingObservedExecutionOutcome::IncompleteObservation {
                    observations, ..
                }
                | RenderingObservedExecutionOutcome::FinalInvariantFailure {
                    observations, ..
                } => observations.as_slice(),
                RenderingObservedExecutionOutcome::ExecutionFailure { .. } => &[],
            };
            for observation in observations {
                if observation.bytes.len() > limits.observation_bytes {
                    return Err(ReportBuildError::ObservationTooLarge {
                        test_id: case.ag.test_id.as_str().to_owned(),
                        surface: observation.profile.stable_label().to_owned(),
                        actual: observation.bytes.len(),
                        maximum: limits.observation_bytes,
                    });
                }
            }
            if let RenderingObservedExecutionOutcome::SemanticMismatch { mismatches, .. } = outcome
            {
                let evidence_bytes = rendering_mismatch_evidence_bytes(mismatches).ok_or(
                    ReportBuildError::MismatchDiagnosticTooLarge {
                        test_id: case.ag.test_id.as_str().to_owned(),
                        actual: usize::MAX,
                        maximum: limits.mismatch_diagnostic_bytes,
                    },
                )?;
                if evidence_bytes > limits.mismatch_diagnostic_bytes {
                    return Err(ReportBuildError::MismatchDiagnosticTooLarge {
                        test_id: case.ag.test_id.as_str().to_owned(),
                        actual: evidence_bytes,
                        maximum: limits.mismatch_diagnostic_bytes,
                    });
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn rendering_mismatch_evidence_bytes(
    mismatches: &[rendering_test_support::RenderingMismatchEvidence],
) -> Option<usize> {
    mismatches.iter().try_fold(0usize, |total, mismatch| {
        let item = "BEGIN mismatch\n"
            .len()
            .checked_add("profile".len() + 6 + mismatch.profile.stable_label().len())?
            .checked_add(
                "first-mismatching-line".len()
                    + 4
                    + decimal_digits(mismatch.difference.first_mismatching_line),
            )?
            .checked_add(
                "expected-bytes".len() + 4 + decimal_digits(mismatch.difference.expected_bytes),
            )?
            .checked_add(
                "actual-bytes".len() + 4 + decimal_digits(mismatch.difference.actual_bytes),
            )?
            .checked_add("END mismatch\n".len())?;
        total.checked_add(item)
    })
}

fn decimal_digits(value: usize) -> usize {
    value
        .checked_ilog10()
        .map_or(1, |digits| digits as usize + 1)
}

fn policy_name(policy: DerivedPolicyResult) -> &'static str {
    match policy {
        DerivedPolicyResult::ExpectedPass => "expected-pass",
        DerivedPolicyResult::UnexpectedFail => "unexpected-fail",
        DerivedPolicyResult::ExpectedFail => "xfail",
        DerivedPolicyResult::UnexpectedPass => "xpass",
        DerivedPolicyResult::NotRun => "not-run",
        DerivedPolicyResult::NotYetEstablished => "not-yet-established",
        DerivedPolicyResult::UnexpectedOutcome => "unexpected-outcome",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conformance_test_support::{ObservationSurface, TestId};
    use rendering_test_support::{
        AvailableWidthCssPx, PaintObservationProfile, RenderingExecutionVariantId,
        RenderingMismatchEvidence, RenderingObservationProfile, RenderingProfileObservation,
        RenderingSnapshotDifference, SyntheticTextMetricsV1,
    };

    use crate::rendering_runner::RenderingVariantResult;

    fn case_with_outcome(outcome: RenderingObservedExecutionOutcome) -> RenderingCaseResult {
        let profile = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations);
        RenderingCaseResult {
            ag: AgCaseState {
                test_id: TestId::parse("rendering-report-limit").unwrap(),
                observation: ObservationSurface::PaintOperations,
                classification: ClassificationCompleteness::Classified,
                requirements: vec![],
                capability: Some(CapabilityAvailability::Available),
                harness: Some(HarnessReadiness::Ready),
                environment_requirements: vec![],
                stability: Some(Stability::Stable),
                lane_exclusions: vec![],
                eligibility: Eligibility::Runnable,
                expectation: AgExpectation::ExpectedPass,
            },
            variants: vec![RenderingVariantResult {
                variant: ExecutionVariantId::new(RenderingExecutionVariantId {
                    environment: SyntheticTextMetricsV1::SyntheticTextMetricsV1,
                    available_width_css_px: AvailableWidthCssPx::try_new(320).unwrap(),
                }),
                profiles: vec![profile],
                execution: RenderingExecutionAttempt::Attempted { outcome },
                policy: DerivedPolicyResult::ExpectedPass,
            }],
        }
    }

    fn pass_with_observation(bytes: String) -> RenderingCaseResult {
        case_with_outcome(RenderingObservedExecutionOutcome::SemanticPass {
            observations: vec![RenderingProfileObservation {
                profile: RenderingObservationProfile::Paint(
                    PaintObservationProfile::PaintOperations,
                ),
                bytes,
            }],
        })
    }

    #[test]
    fn rendering_report_uses_ag_xfail_and_xpass_policy_labels() {
        assert_eq!(policy_name(DerivedPolicyResult::ExpectedFail), "xfail");
        assert_eq!(policy_name(DerivedPolicyResult::UnexpectedPass), "xpass");
    }

    #[test]
    fn report_accepts_an_exactly_maximum_sized_observation() {
        let case = pass_with_observation("a".repeat(DEFAULT_REPORT_LIMITS.observation_bytes));
        assert!(build_rendering_report(&[case]).is_ok());
    }

    #[test]
    fn report_rejects_maximum_plus_one_observation_before_serialization() {
        let case = pass_with_observation("a".repeat(DEFAULT_REPORT_LIMITS.observation_bytes + 1));
        assert!(matches!(
            build_rendering_report(&[case]),
            Err(ReportBuildError::ObservationTooLarge {
                actual,
                maximum,
                ..
            }) if actual == DEFAULT_REPORT_LIMITS.observation_bytes + 1
                && maximum == DEFAULT_REPORT_LIMITS.observation_bytes
        ));
    }

    #[test]
    fn report_total_overflow_remains_separately_classified() {
        let case = pass_with_observation("small".to_owned());
        let limits = ReportLimits {
            total_bytes: 64,
            observation_bytes: DEFAULT_REPORT_LIMITS.observation_bytes,
            mismatch_diagnostic_bytes: DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes,
        };
        assert!(matches!(
            build_rendering_report_with_limits(&[case], limits),
            Err(ReportBuildError::ReportTooLarge { maximum: 64 })
        ));
    }

    #[test]
    fn report_emits_structured_mismatch_evidence_and_enforces_its_limit() {
        let mismatch = RenderingMismatchEvidence {
            profile: RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations),
            difference: RenderingSnapshotDifference {
                first_mismatching_line: 3,
                expected_bytes: 20,
                actual_bytes: 21,
            },
        };
        let case = case_with_outcome(RenderingObservedExecutionOutcome::SemanticMismatch {
            observations: vec![],
            mismatches: vec![mismatch],
        });
        let report =
            String::from_utf8(build_rendering_report(std::slice::from_ref(&case)).unwrap())
                .unwrap();
        assert!(report.contains("first-mismatching-line = 3\n"));
        assert!(report.contains("expected-bytes = 20\n"));
        assert!(report.contains("actual-bytes = 21\n"));
        assert!(report.contains("execution-variant-count = 1\n"));

        let limits = ReportLimits {
            total_bytes: DEFAULT_REPORT_LIMITS.total_bytes,
            observation_bytes: DEFAULT_REPORT_LIMITS.observation_bytes,
            mismatch_diagnostic_bytes: 1,
        };
        assert!(matches!(
            build_rendering_report_with_limits(&[case], limits),
            Err(ReportBuildError::MismatchDiagnosticTooLarge { maximum: 1, .. })
        ));
    }

    #[test]
    fn incomplete_observation_reports_its_typed_serialization_phase() {
        let case = case_with_outcome(RenderingObservedExecutionOutcome::IncompleteObservation {
            phase: rendering_test_support::RenderingExecutionPhase::ObservationSerialization,
            profile: RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations),
            reason: rendering_test_support::RenderingIncompleteObservationReason::AllocationFailure,
            observations: vec![],
        });
        let report = String::from_utf8(build_rendering_report(&[case]).unwrap()).unwrap();
        assert!(report.contains("observed = \"incomplete-observation\"\n"));
        assert!(report.contains("phase = \"observation-serialization\"\n"));
    }
}
