use std::io::Write;

use rendering_test_support::{
    RenderingDifferenceLine, RenderingFirstDifference, RenderingObservedExecutionOutcome,
};

use crate::model::*;
use crate::rendering_runner::{
    RenderingCaptureSummary, RenderingCaseResult, RenderingExecutionAttempt, RenderingOracleKind,
    RenderingReferenceObservedOutcome, RenderingVariantObservedOutcome,
};
use crate::report::{
    BoundedWriter, DEFAULT_REPORT_LIMITS, ReportBuildError, ReportLimits, ReportPublicationError,
};

pub const RENDERING_REPORT_FORMAT_V1: &str = "borrowser-conformance-rendering-report-v1";
pub const RENDERING_REPORT_FORMAT_V2: &str = "borrowser-conformance-rendering-report-v2";
pub const REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1: usize = 16 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderingReportVersion {
    V1,
    V2,
}

pub fn build_rendering_report(cases: &[RenderingCaseResult]) -> Result<Vec<u8>, ReportBuildError> {
    build_rendering_report_with_limits(cases, DEFAULT_REPORT_LIMITS, RenderingReportVersion::V2)
}

pub fn build_rendering_report_v1(
    cases: &[RenderingCaseResult],
) -> Result<Vec<u8>, ReportBuildError> {
    build_rendering_report_with_limits(cases, DEFAULT_REPORT_LIMITS, RenderingReportVersion::V1)
}

fn build_rendering_report_with_limits(
    cases: &[RenderingCaseResult],
    limits: ReportLimits,
    version: RenderingReportVersion,
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
    writer.line(
        "format",
        match version {
            RenderingReportVersion::V1 => RENDERING_REPORT_FORMAT_V1,
            RenderingReportVersion::V2 => RENDERING_REPORT_FORMAT_V2,
        },
    )?;
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
            if version == RenderingReportVersion::V2 {
                write_oracle(&mut writer, variant.oracle)?;
            }
            match &variant.execution {
                RenderingExecutionAttempt::NotAttempted { reason, .. } => {
                    writer.line("attempt", "not-attempted")?;
                    let reason = match reason {
                        crate::RenderingNotAttemptedReason::Eligibility => "eligibility",
                        crate::RenderingNotAttemptedReason::LaneExcluded => {
                            return Err(ReportBuildError::UnsupportedReportCase {
                                format: match version {
                                    RenderingReportVersion::V1 => RENDERING_REPORT_FORMAT_V1,
                                    RenderingReportVersion::V2 => RENDERING_REPORT_FORMAT_V2,
                                },
                            });
                        }
                    };
                    writer.line("not-attempted-reason", reason)?;
                }
                RenderingExecutionAttempt::Attempted { outcome } => {
                    writer.line("attempt", "attempted")?;
                    match (version, outcome) {
                        (
                            RenderingReportVersion::V1,
                            RenderingVariantObservedOutcome::AuthoredSnapshot(outcome),
                        ) => write_snapshot_outcome(&mut writer, outcome)?,
                        (
                            RenderingReportVersion::V2,
                            RenderingVariantObservedOutcome::AuthoredSnapshot(outcome),
                        ) => write_snapshot_outcome(&mut writer, outcome)?,
                        (
                            RenderingReportVersion::V2,
                            RenderingVariantObservedOutcome::DocumentReference(outcome),
                        ) => write_reference_outcome(&mut writer, variant.oracle, outcome)?,
                        (RenderingReportVersion::V1, _) => {
                            return Err(ReportBuildError::UnsupportedReportCase {
                                format: RENDERING_REPORT_FORMAT_V1,
                            });
                        }
                    }
                }
            }
            writer.line("policy", policy_name(variant.policy))?;
            writer.raw("END variant\n")?;
        }
        writer.raw("END logical-case\n")?;
    }
    Ok(writer.finish())
}

fn write_oracle(
    writer: &mut BoundedWriter,
    oracle: RenderingOracleKind,
) -> Result<(), ReportBuildError> {
    match oracle {
        RenderingOracleKind::AuthoredSnapshot => writer.line("oracle", "authored-snapshot"),
        RenderingOracleKind::DocumentReference {
            reference_kind,
            relation,
        } => {
            writer.line("oracle", "document-reference")?;
            writer.line("reference-kind", reference_kind.as_str())?;
            writer.line("relation", relation.as_str())
        }
    }
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

fn write_snapshot_outcome(
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

fn write_reference_outcome(
    writer: &mut BoundedWriter,
    oracle_kind: RenderingOracleKind,
    outcome: &RenderingReferenceObservedOutcome,
) -> Result<(), ReportBuildError> {
    match outcome {
        RenderingReferenceObservedOutcome::Relation {
            test,
            reference,
            oracle,
            semantic,
            first_difference,
        } => {
            writer.line("observed", semantic.stable_label())?;
            writer.line(
                "oracle-verdict",
                match oracle {
                    rendering_test_support::RenderingOracleVerdict::Equivalent => "equivalent",
                    rendering_test_support::RenderingOracleVerdict::Different => "different",
                },
            )?;
            write_capture_summary(writer, "test", test)?;
            write_capture_summary(writer, "reference", reference)?;
            match first_difference {
                Some(difference) => write_first_difference(writer, difference),
                None => {
                    writer.line("difference-evidence", "none")?;
                    if matches!(
                        oracle_kind,
                        RenderingOracleKind::DocumentReference {
                            relation: conformance_test_support::ReferenceRelation::Mismatch,
                            ..
                        }
                    ) && matches!(
                        oracle,
                        rendering_test_support::RenderingOracleVerdict::Equivalent
                    ) {
                        writer.line("difference-reason", "no-differing-profile-found")?;
                    }
                    Ok(())
                }
            }
        }
        RenderingReferenceObservedOutcome::CaptureTerminal { test, reference } => {
            writer.line("observed", "capture-terminal")?;
            write_capture_summary(writer, "test", test)?;
            write_capture_summary(writer, "reference", reference)
        }
        RenderingReferenceObservedOutcome::ComparisonInvariant {
            test,
            reference,
            failure,
        } => {
            writer.line("observed", "comparison-invariant-failure")?;
            writer.line("failure", failure.stable_label())?;
            write_capture_summary(writer, "test", test)?;
            write_capture_summary(writer, "reference", reference)
        }
    }
}

fn write_capture_summary(
    writer: &mut BoundedWriter,
    side: &str,
    summary: &RenderingCaptureSummary,
) -> Result<(), ReportBuildError> {
    writer.raw("BEGIN capture\n")?;
    writer.line("side", side)?;
    writer.line("state", summary.stable_label())?;
    match summary {
        RenderingCaptureSummary::Complete { observations } => {
            write_observation_summaries(writer, observations)?;
        }
        RenderingCaptureSummary::ExecutionFailure { phase, failure } => {
            writer.line("phase", phase.stable_label())?;
            writer.line("failure", failure.stable_label())?;
            write_execution_failure_evidence(writer, failure)?;
        }
        RenderingCaptureSummary::IncompleteObservation {
            phase,
            profile,
            reason,
            observations,
        } => {
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
            write_observation_summaries(writer, observations)?;
        }
        RenderingCaptureSummary::FinalInvariantFailure {
            phase,
            failure,
            observations,
        } => {
            writer.line("phase", phase.stable_label())?;
            writer.line("failure", failure.stable_label())?;
            write_observation_summaries(writer, observations)?;
        }
    }
    writer.raw("END capture\n")
}

fn write_observation_summaries(
    writer: &mut BoundedWriter,
    observations: &[crate::rendering_runner::RenderingObservationSummary],
) -> Result<(), ReportBuildError> {
    writer.number("observation-count", observations.len())?;
    for observation in observations {
        writer.raw("BEGIN observation-summary\n")?;
        writer.line("profile", observation.profile.stable_label())?;
        writer.number("bytes", observation.bytes)?;
        writer.raw("END observation-summary\n")?;
    }
    Ok(())
}

fn write_first_difference(
    writer: &mut BoundedWriter,
    difference: &RenderingFirstDifference,
) -> Result<(), ReportBuildError> {
    writer.line("difference-evidence", "first-difference-v1")?;
    writer.raw("BEGIN first-difference\n")?;
    writer.line("profile", difference.profile.stable_label())?;
    writer.u64_number("one-based-line", difference.one_based_line)?;
    writer.u64_number("test-observation-bytes", difference.test_observation_bytes)?;
    writer.u64_number(
        "reference-observation-bytes",
        difference.reference_observation_bytes,
    )?;
    write_difference_line(writer, "test", &difference.test_line)?;
    write_difference_line(writer, "reference", &difference.reference_line)?;
    writer.raw("END first-difference\n")
}

fn write_difference_line(
    writer: &mut BoundedWriter,
    side: &str,
    line: &RenderingDifferenceLine,
) -> Result<(), ReportBuildError> {
    writer.raw("BEGIN difference-line\n")?;
    writer.line("side", side)?;
    match line {
        RenderingDifferenceLine::Missing => writer.line("state", "missing")?,
        RenderingDifferenceLine::Present {
            original_bytes,
            excerpt,
            truncated,
        } => {
            writer.line("state", "present")?;
            writer.u64_number("original-bytes", *original_bytes)?;
            writer.line("truncated", if *truncated { "true" } else { "false" })?;
            writer.line("excerpt", excerpt)?;
        }
    }
    writer.raw("END difference-line\n")
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
            match outcome {
                RenderingVariantObservedOutcome::AuthoredSnapshot(outcome) => {
                    let observations = match outcome {
                        RenderingObservedExecutionOutcome::SemanticPass { observations }
                        | RenderingObservedExecutionOutcome::SemanticMismatch {
                            observations,
                            ..
                        }
                        | RenderingObservedExecutionOutcome::IncompleteObservation {
                            observations,
                            ..
                        }
                        | RenderingObservedExecutionOutcome::FinalInvariantFailure {
                            observations,
                            ..
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
                    if let RenderingObservedExecutionOutcome::SemanticMismatch {
                        mismatches, ..
                    } = outcome
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
                RenderingVariantObservedOutcome::DocumentReference(
                    RenderingReferenceObservedOutcome::Relation {
                        first_difference: Some(difference),
                        ..
                    },
                ) => {
                    let evidence_bytes = rendering_first_difference_evidence_bytes(difference)?;
                    if evidence_bytes > REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1 {
                        return Err(ReportBuildError::MismatchDiagnosticTooLarge {
                            test_id: case.ag.test_id.as_str().to_owned(),
                            actual: evidence_bytes,
                            maximum: REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1,
                        });
                    }
                    if evidence_bytes > limits.mismatch_diagnostic_bytes {
                        return Err(ReportBuildError::MismatchDiagnosticTooLarge {
                            test_id: case.ag.test_id.as_str().to_owned(),
                            actual: evidence_bytes,
                            maximum: limits.mismatch_diagnostic_bytes,
                        });
                    }
                }
                RenderingVariantObservedOutcome::DocumentReference(_) => {}
            }
        }
    }
    Ok(())
}

pub(crate) fn rendering_first_difference_evidence_bytes(
    difference: &RenderingFirstDifference,
) -> Result<usize, ReportBuildError> {
    let mut writer = BoundedWriter::new(usize::MAX)?;
    write_first_difference(&mut writer, difference)?;
    Ok(writer.finish().len())
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
    use conformance_test_support::{ObservationSurface, ReferenceKind, ReferenceRelation, TestId};
    use rendering_test_support::{
        AvailableWidthCssPx, PaintObservationProfile, RenderingExecutionVariantId,
        RenderingMismatchEvidence, RenderingObservationProfile, RenderingProfileObservation,
        RenderingSnapshotDifference, SyntheticTextMetricsV1,
    };

    use crate::rendering_runner::{RenderingRelationResult, RenderingVariantResult};

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
                oracle: RenderingOracleKind::AuthoredSnapshot,
                execution: RenderingExecutionAttempt::Attempted {
                    outcome: RenderingVariantObservedOutcome::AuthoredSnapshot(outcome),
                },
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

    fn first_difference(
        test_excerpt: String,
        reference_excerpt: String,
    ) -> RenderingFirstDifference {
        RenderingFirstDifference {
            profile: RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations),
            one_based_line: u64::MAX,
            test_observation_bytes: u64::MAX,
            reference_observation_bytes: u64::MAX,
            test_line: RenderingDifferenceLine::Present {
                original_bytes: u64::MAX,
                excerpt: test_excerpt,
                truncated: true,
            },
            reference_line: RenderingDifferenceLine::Present {
                original_bytes: u64::MAX,
                excerpt: reference_excerpt,
                truncated: true,
            },
        }
    }

    fn case_with_reference_difference(difference: RenderingFirstDifference) -> RenderingCaseResult {
        let mut case = case_with_outcome(RenderingObservedExecutionOutcome::SemanticPass {
            observations: vec![],
        });
        let variant = &mut case.variants[0];
        variant.oracle = RenderingOracleKind::DocumentReference {
            reference_kind: ReferenceKind::Semantic,
            relation: ReferenceRelation::Mismatch,
        };
        variant.execution = RenderingExecutionAttempt::Attempted {
            outcome: RenderingVariantObservedOutcome::DocumentReference(
                RenderingReferenceObservedOutcome::Relation {
                    test: RenderingCaptureSummary::Complete {
                        observations: vec![],
                    },
                    reference: RenderingCaptureSummary::Complete {
                        observations: vec![],
                    },
                    oracle: rendering_test_support::RenderingOracleVerdict::Different,
                    semantic: RenderingRelationResult::SemanticPass,
                    first_difference: Some(difference),
                },
            ),
        };
        case
    }

    #[test]
    fn rendering_report_uses_ag_xfail_and_xpass_policy_labels() {
        assert_eq!(policy_name(DerivedPolicyResult::ExpectedFail), "xfail");
        assert_eq!(policy_name(DerivedPolicyResult::UnexpectedPass), "xpass");
    }

    #[test]
    fn report_v1_remains_snapshot_only_while_default_report_is_v2() {
        let case = pass_with_observation("owner bytes".to_owned());
        assert_eq!(
            build_rendering_report_v1(std::slice::from_ref(&case)).unwrap(),
            include_bytes!("../tests/data/rendering-report-v1-compat.txt")
        );
        assert_eq!(
            build_rendering_report(&[case]).unwrap(),
            include_bytes!("../tests/data/rendering-report-v2.txt")
        );
    }

    #[test]
    fn historical_rendering_reports_reject_named_lane_only_not_attempted_state() {
        let mut case = pass_with_observation("owner bytes".to_owned());
        case.variants[0].execution = RenderingExecutionAttempt::NotAttempted {
            reason: crate::RenderingNotAttemptedReason::LaneExcluded,
            pre_attempt: None,
        };
        assert!(matches!(
            build_rendering_report_v1(std::slice::from_ref(&case)),
            Err(ReportBuildError::UnsupportedReportCase {
                format: RENDERING_REPORT_FORMAT_V1,
            })
        ));
        assert!(matches!(
            build_rendering_report(&[case]),
            Err(ReportBuildError::UnsupportedReportCase {
                format: RENDERING_REPORT_FORMAT_V2,
            })
        ));
    }

    #[test]
    fn successful_evidence_measurement_matches_encoding_and_proves_the_v1_ceiling() {
        let excerpt =
            "\u{1f}".repeat(rendering_test_support::REFERENCE_DIFFERENCE_EXCERPT_UTF8_BYTES_V1);
        assert_eq!(
            excerpt.len(),
            rendering_test_support::REFERENCE_DIFFERENCE_EXCERPT_UTF8_BYTES_V1
        );
        let difference = first_difference(excerpt.clone(), excerpt);
        let exact = rendering_first_difference_evidence_bytes(&difference).unwrap();
        assert!(exact <= REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1);
        let mut writer = BoundedWriter::new(exact).unwrap();
        write_first_difference(&mut writer, &difference).unwrap();
        assert_eq!(writer.finish().len(), exact);
        let mut too_small = BoundedWriter::new(exact - 1).unwrap();
        assert!(write_first_difference(&mut too_small, &difference).is_err());
    }

    #[test]
    fn evidence_measurement_and_its_report_caller_preserve_allocation_failure() {
        let difference = first_difference("test".to_owned(), "reference".to_owned());
        assert!(matches!(
            crate::report::with_forced_allocation_failure(|| {
                rendering_first_difference_evidence_bytes(&difference)
            }),
            Err(ReportBuildError::AllocationFailure)
        ));

        let case = case_with_reference_difference(difference);
        assert!(matches!(
            crate::report::with_forced_allocation_failure(|| {
                build_rendering_report(std::slice::from_ref(&case))
            }),
            Err(ReportBuildError::AllocationFailure)
        ));
    }

    #[test]
    fn ag7_and_generic_mismatch_evidence_limits_are_independently_enforced() {
        assert_eq!(REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1, 16 * 1024);
        assert_eq!(DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes, 1024 * 1024);
        assert!(
            REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1
                < DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes
        );

        let oversized = first_difference(
            "t".repeat(REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1),
            "r".repeat(REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1),
        );
        let oversized_bytes = rendering_first_difference_evidence_bytes(&oversized).unwrap();
        assert!(oversized_bytes > REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1);
        assert!(matches!(
            build_rendering_report(&[case_with_reference_difference(oversized)]),
            Err(ReportBuildError::MismatchDiagnosticTooLarge {
                actual,
                maximum: REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1,
                ..
            }) if actual == oversized_bytes
        ));

        let ordinary = first_difference("test".to_owned(), "reference".to_owned());
        let ordinary_bytes = rendering_first_difference_evidence_bytes(&ordinary).unwrap();
        assert!(ordinary_bytes < REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1);
        let generic_limits = ReportLimits {
            total_bytes: DEFAULT_REPORT_LIMITS.total_bytes,
            observation_bytes: DEFAULT_REPORT_LIMITS.observation_bytes,
            mismatch_diagnostic_bytes: ordinary_bytes - 1,
        };
        assert!(matches!(
            build_rendering_report_with_limits(
                &[case_with_reference_difference(ordinary)],
                generic_limits,
                RenderingReportVersion::V2,
            ),
            Err(ReportBuildError::MismatchDiagnosticTooLarge {
                actual,
                maximum,
                ..
            }) if actual == ordinary_bytes && maximum == ordinary_bytes - 1
        ));

        let mut default_budget = crate::report::RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
        default_budget
            .retain_mismatch(
                "one-mib-boundary",
                DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes,
            )
            .unwrap();
        let mut oversized_budget =
            crate::report::RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
        assert!(matches!(
            oversized_budget.retain_mismatch(
                "one-mib-boundary",
                DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes + 1,
            ),
            Err(ReportBuildError::MismatchDiagnosticTooLarge {
                actual,
                maximum,
                ..
            }) if actual == DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes + 1
                && maximum == DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes
        ));
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
            build_rendering_report_with_limits(&[case], limits, RenderingReportVersion::V2),
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
            build_rendering_report_with_limits(&[case], limits, RenderingReportVersion::V2),
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
