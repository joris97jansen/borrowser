use std::io::Write;

use crate::model::*;
use crate::report_writer::{CanonicalReportWriter, CanonicalReportWriterFailure};

pub(crate) type BoundedWriter = CanonicalReportWriter<ReportBuildError>;

pub const REPORT_FORMAT_V1: &str = "borrowser-conformance-parser-report-v1";

/// Fixed AG report policy shared by the versioned parser, CSS, and rendering
/// reports. These are runner infrastructure limits, not subsystem limits, and
/// are not configurable by fixtures, hosts, the CLI, or environment variables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportLimits {
    pub total_bytes: usize,
    pub observation_bytes: usize,
    pub mismatch_diagnostic_bytes: usize,
}

/// The 8 MiB artifact limit is over 2,700 times the largest reviewed AE
/// canonical sidecar measured when AG4 was introduced (2,971 bytes), while
/// allowing substantial headroom for the event-count guardrails in AE. The
/// 1 MiB mismatch limit is over 350 times that reviewed maximum. The 32 MiB
/// total permits several maximum-sized artifacts plus deterministic escaping
/// overhead while remaining suitable for CI; the complete seven-case AG4
/// report measured 30,269 bytes with framing and escaping. These AG limits
/// intentionally do not claim byte-bounded AE observation capture.
pub const DEFAULT_REPORT_LIMITS: ReportLimits = ReportLimits {
    total_bytes: 32 * 1024 * 1024,
    observation_bytes: 8 * 1024 * 1024,
    mismatch_diagnostic_bytes: 1024 * 1024,
};

#[derive(Debug)]
pub enum ReportBuildError {
    ObservationTooLarge {
        test_id: String,
        surface: String,
        actual: usize,
        maximum: usize,
    },
    MismatchDiagnosticTooLarge {
        test_id: String,
        actual: usize,
        maximum: usize,
    },
    ReportTooLarge {
        maximum: usize,
    },
    RetainedEvidenceTooLarge {
        actual: usize,
        maximum: usize,
    },
    AllocationFailure,
    UnsupportedReportCase {
        format: &'static str,
    },
}

impl std::fmt::Display for ReportBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObservationTooLarge {
                test_id,
                surface,
                actual,
                maximum,
            } => write!(
                f,
                "AG report observation exceeds limit: test={test_id} surface={surface} bytes={actual} maximum={maximum}"
            ),
            Self::MismatchDiagnosticTooLarge {
                test_id,
                actual,
                maximum,
            } => write!(
                f,
                "AG report mismatch diagnostic exceeds limit: test={test_id} bytes={actual} maximum={maximum}"
            ),
            Self::ReportTooLarge { maximum } => {
                write!(f, "AG report exceeds the fixed {maximum}-byte limit")
            }
            Self::RetainedEvidenceTooLarge { actual, maximum } => write!(
                f,
                "AG retained report evidence exceeds limit: bytes={actual} maximum={maximum}"
            ),
            Self::AllocationFailure => f.write_str("AG report allocation failed"),
            Self::UnsupportedReportCase { format } => {
                write!(f, "AG report format {format} cannot represent this case")
            }
        }
    }
}

impl std::error::Error for ReportBuildError {}

impl CanonicalReportWriterFailure for ReportBuildError {
    fn report_too_large(maximum: usize) -> Self {
        Self::ReportTooLarge { maximum }
    }

    fn allocation_failure() -> Self {
        Self::AllocationFailure
    }
}

/// Bounds evidence as soon as subsystem observations and mismatch diagnostics
/// cross into AG. Subsystems retain ownership of any earlier capture limits.
#[cfg(any(feature = "html-parser", feature = "css", feature = "rendering", test))]
pub(crate) struct RetainedEvidenceBudget {
    limits: ReportLimits,
    retained: usize,
}

#[cfg(any(feature = "html-parser", feature = "css", feature = "rendering", test))]
impl RetainedEvidenceBudget {
    pub(crate) const fn new(limits: ReportLimits) -> Self {
        Self {
            limits,
            retained: 0,
        }
    }

    #[cfg(any(feature = "html-parser", test))]
    pub(crate) fn retain_observation(
        &mut self,
        test_id: &str,
        surface: ParserObservationSurface,
        bytes: usize,
    ) -> Result<(), ReportBuildError> {
        self.retain_named_observation(test_id, surface_name(surface), bytes)
    }

    pub(crate) fn retain_named_observation(
        &mut self,
        test_id: &str,
        surface: &str,
        bytes: usize,
    ) -> Result<(), ReportBuildError> {
        if bytes > self.limits.observation_bytes {
            return Err(ReportBuildError::ObservationTooLarge {
                test_id: test_id.to_owned(),
                surface: surface.to_owned(),
                actual: bytes,
                maximum: self.limits.observation_bytes,
            });
        }
        self.retain(bytes)
    }

    #[cfg(any(feature = "html-parser", feature = "css", feature = "rendering", test))]
    pub(crate) fn retain_mismatch(
        &mut self,
        test_id: &str,
        bytes: usize,
    ) -> Result<(), ReportBuildError> {
        if bytes > self.limits.mismatch_diagnostic_bytes {
            return Err(ReportBuildError::MismatchDiagnosticTooLarge {
                test_id: test_id.to_owned(),
                actual: bytes,
                maximum: self.limits.mismatch_diagnostic_bytes,
            });
        }
        self.retain(bytes)
    }

    fn retain(&mut self, bytes: usize) -> Result<(), ReportBuildError> {
        let actual =
            self.retained
                .checked_add(bytes)
                .ok_or(ReportBuildError::RetainedEvidenceTooLarge {
                    actual: usize::MAX,
                    maximum: self.limits.total_bytes,
                })?;
        if actual > self.limits.total_bytes {
            return Err(ReportBuildError::RetainedEvidenceTooLarge {
                actual,
                maximum: self.limits.total_bytes,
            });
        }
        self.retained = actual;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ReportPublicationError {
    Build(ReportBuildError),
    OutputWrite(std::io::Error),
}

impl std::fmt::Display for ReportPublicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Build(error) => write!(f, "report construction failed: {error}"),
            Self::OutputWrite(error) => write!(f, "report output write failed: {error}"),
        }
    }
}

impl std::error::Error for ReportPublicationError {}

pub fn build_report(cases: &[NormalizedCaseResult]) -> Result<Vec<u8>, ReportBuildError> {
    build_report_with_limits(cases, DEFAULT_REPORT_LIMITS)
}

pub fn build_and_write_report(
    cases: &[NormalizedCaseResult],
    output: &mut impl Write,
) -> Result<(), ReportPublicationError> {
    let report = build_report(cases).map_err(ReportPublicationError::Build)?;
    output
        .write_all(&report)
        .map_err(ReportPublicationError::OutputWrite)
}

fn build_report_with_limits(
    cases: &[NormalizedCaseResult],
    limits: ReportLimits,
) -> Result<Vec<u8>, ReportBuildError> {
    validate_case_limits(cases, limits)?;
    let mut writer = BoundedWriter::new(limits.total_bytes)?;
    writer.line("format", REPORT_FORMAT_V1)?;
    writer.number("case-count", cases.len())?;
    for case in cases {
        writer.raw("\nBEGIN case\n")?;
        writer.line("test-id", case.ag.test_id.as_str())?;
        writer.line("profile", profile_name(case.profile))?;
        write_metadata(&mut writer, case)?;
        write_eligibility(&mut writer, &case.ag.eligibility)?;
        write_expectation(&mut writer, &case.ag.expectation)?;
        write_execution(&mut writer, &case.execution)?;
        writer.optional_line("ae-disposition", case.ae_disposition.map(disposition_name))?;
        writer.line("policy", policy_name(case.policy))?;
        writer.number("observation-count", case.observations.len())?;
        for artifact in &case.observations {
            writer.raw("BEGIN observation\n")?;
            writer.line("surface", surface_name(artifact.surface))?;
            writer.line("snapshot-format", &artifact.format)?;
            writer.multiline("bytes", &artifact.bytes)?;
            writer.raw("END observation\n")?;
        }
        writer.raw("END case\n")?;
    }
    Ok(writer.finish())
}

fn validate_case_limits(
    cases: &[NormalizedCaseResult],
    limits: ReportLimits,
) -> Result<(), ReportBuildError> {
    for case in cases {
        for artifact in &case.observations {
            if artifact.bytes.len() > limits.observation_bytes {
                return Err(ReportBuildError::ObservationTooLarge {
                    test_id: case.ag.test_id.as_str().to_owned(),
                    surface: surface_name(artifact.surface).to_owned(),
                    actual: artifact.bytes.len(),
                    maximum: limits.observation_bytes,
                });
            }
        }
        let diagnostic = match &case.execution {
            ExecutionAttempt::Attempted {
                outcome:
                    ObservedExecutionOutcome::ExpectationMismatch { difference, .. }
                    | ObservedExecutionOutcome::ParityMismatch { difference, .. },
            } => Some(difference),
            ExecutionAttempt::NotAttempted { .. } | ExecutionAttempt::Attempted { .. } => None,
        };
        if let Some(diagnostic) = diagnostic
            && diagnostic.len() > limits.mismatch_diagnostic_bytes
        {
            return Err(ReportBuildError::MismatchDiagnosticTooLarge {
                test_id: case.ag.test_id.as_str().to_owned(),
                actual: diagnostic.len(),
                maximum: limits.mismatch_diagnostic_bytes,
            });
        }
    }
    Ok(())
}

fn write_metadata(
    writer: &mut BoundedWriter,
    case: &NormalizedCaseResult,
) -> Result<(), ReportBuildError> {
    match &case.ag.classification {
        ClassificationCompleteness::NotYetClassified { reason } => {
            writer.line("classification", "not-yet-classified")?;
            writer.line("classification-reason", reason)?;
        }
        ClassificationCompleteness::Classified => {
            writer.line("classification", "classified")?;
        }
    }
    writer.list(
        "requirements",
        case.ag.requirements.iter().map(|value| value.as_str()),
    )?;
    match &case.ag.capability {
        None => writer.optional_line("engine", None)?,
        Some(engine) => match engine {
            CapabilityAvailability::Available => writer.line("engine", "available")?,
            CapabilityAvailability::NotYetEstablished => {
                writer.line("engine", "not-yet-established")?
            }
            CapabilityAvailability::Unavailable { missing } => {
                writer.line("engine", "unavailable")?;
                for item in missing {
                    writer.raw("BEGIN engine-missing\n")?;
                    writer.line("kind", item.kind.as_str())?;
                    writer.optional_line("feature", item.feature.as_deref())?;
                    writer.line("reason", &item.reason)?;
                    writer.raw("END engine-missing\n")?;
                }
            }
        },
    }
    match &case.ag.harness {
        None => writer.optional_line("harness", None)?,
        Some(harness) => match harness {
            HarnessReadiness::Ready => writer.line("harness", "ready")?,
            HarnessReadiness::NotYetEstablished => writer.line("harness", "not-yet-established")?,
            HarnessReadiness::NotReady { limitations } => {
                writer.line("harness", "not-ready")?;
                for item in limitations {
                    writer.raw("BEGIN harness-limitation\n")?;
                    writer.line("kind", item.kind.as_str())?;
                    writer.line("reason", &item.reason)?;
                    writer.raw("END harness-limitation\n")?;
                }
            }
        },
    }
    for item in &case.ag.environment_requirements {
        writer.raw("BEGIN environment-requirement\n")?;
        writer.line("kind", item.kind.as_str())?;
        writer.line("profile", &item.profile)?;
        writer.line("reason", &item.reason)?;
        writer.raw("END environment-requirement\n")?;
    }
    match &case.ag.stability {
        None => writer.optional_line("stability", None)?,
        Some(stability) => match stability {
            Stability::Stable => writer.line("stability", "stable")?,
            Stability::NotYetEstablished => writer.line("stability", "not-yet-established")?,
            Stability::Flaky { reason } => {
                writer.line("stability", "flaky")?;
                writer.line("stability-reason", reason)?;
            }
        },
    }
    for item in &case.ag.lane_exclusions {
        writer.raw("BEGIN lane-exclusion\n")?;
        writer.line("policy", item.policy.as_str())?;
        writer.line("reason", &item.reason)?;
        writer.raw("END lane-exclusion\n")?;
    }
    Ok(())
}

fn write_eligibility(
    writer: &mut BoundedWriter,
    eligibility: &Eligibility,
) -> Result<(), ReportBuildError> {
    match eligibility {
        Eligibility::Runnable => writer.line("eligibility", "runnable"),
        Eligibility::NotRunnable {
            blockers,
            unresolved,
        } => {
            writer.line("eligibility", "not-runnable")?;
            write_facts(writer, "eligibility-blocker", blockers)?;
            write_facts(writer, "eligibility-unresolved", unresolved)
        }
        Eligibility::NotYetEstablished { unresolved } => {
            writer.line("eligibility", "not-yet-established")?;
            write_facts(writer, "eligibility-unresolved", unresolved)
        }
    }
}

fn write_facts(
    writer: &mut BoundedWriter,
    prefix: &str,
    facts: &[EligibilityFact],
) -> Result<(), ReportBuildError> {
    for fact in facts {
        writer.raw("BEGIN ")?;
        writer.raw(prefix)?;
        writer.raw("\n")?;
        match fact {
            EligibilityFact::EngineCapability {
                kind,
                feature,
                reason,
            } => {
                writer.line("kind", "engine-capability")?;
                writer.line("capability-kind", kind.as_str())?;
                writer.optional_line("feature", feature.as_deref())?;
                writer.line("reason", reason)?;
            }
            EligibilityFact::Harness { kind, reason } => {
                writer.line("kind", "harness")?;
                writer.line("limitation-kind", kind.as_str())?;
                writer.line("reason", reason)?;
            }
            EligibilityFact::Environment {
                kind,
                profile,
                requirement_reason,
                assessment_reason,
            } => {
                writer.line("kind", "environment")?;
                writer.line("requirement-kind", kind.as_str())?;
                writer.line("profile", profile)?;
                writer.line("requirement-reason", requirement_reason)?;
                writer.line("assessment-reason", assessment_reason)?;
            }
            EligibilityFact::Classification { reason } => {
                writer.line("kind", "classification")?;
                writer.line("reason", reason)?;
            }
            EligibilityFact::EngineCapabilityAvailability => {
                writer.line("kind", "engine-capability-availability")?;
                writer.line("state", "not-yet-established")?;
            }
            EligibilityFact::HarnessReadiness => {
                writer.line("kind", "harness-readiness")?;
                writer.line("state", "not-yet-established")?;
            }
            EligibilityFact::EnvironmentRequirement {
                kind,
                profile,
                reason,
            } => {
                writer.line("kind", "environment-requirement")?;
                writer.line("requirement-kind", kind.as_str())?;
                writer.line("profile", profile)?;
                writer.line("reason", reason)?;
            }
        }
        writer.raw("END ")?;
        writer.raw(prefix)?;
        writer.raw("\n")?;
    }
    Ok(())
}

fn write_expectation(
    writer: &mut BoundedWriter,
    expectation: &AgExpectation,
) -> Result<(), ReportBuildError> {
    match expectation {
        AgExpectation::ExpectedPass => writer.line("expectation", "expected-pass"),
        AgExpectation::NotEstablished => writer.line("expectation", "not-established"),
        AgExpectation::ExpectedFail { failure, reason } => {
            writer.line("expectation", "expected-fail")?;
            writer.line("expected-failure-class", failure.as_str())?;
            writer.line("expected-failure-reason", reason)
        }
    }
}

fn write_execution(
    writer: &mut BoundedWriter,
    execution: &ExecutionAttempt,
) -> Result<(), ReportBuildError> {
    match execution {
        ExecutionAttempt::NotAttempted {
            reason,
            pre_attempt,
        } => {
            writer.line("attempt", "not-attempted")?;
            writer.line(
                "not-attempted-reason",
                historical_not_attempted_reason_name(*reason)?,
            )?;
            writer.optional_line(
                "pre-attempt-outcome",
                pre_attempt.as_ref().map(pre_attempt_name),
            )?;
            if let Some(pre_attempt) = pre_attempt {
                write_pre_attempt(writer, pre_attempt)?;
            }
            writer.optional_line("observed", None)
        }
        ExecutionAttempt::Attempted { outcome } => {
            writer.line("attempt", "attempted")?;
            writer.optional_line("not-attempted-reason", None)?;
            writer.optional_line("pre-attempt-outcome", None)?;
            write_observed(writer, outcome)
        }
    }
}

fn write_pre_attempt(
    writer: &mut BoundedWriter,
    outcome: &PreAttemptEvaluationOutcome,
) -> Result<(), ReportBuildError> {
    match outcome {
        PreAttemptEvaluationOutcome::NotExecutedByAe { classification } => {
            writer.line("pre-attempt-classification", classification)
        }
        PreAttemptEvaluationOutcome::UnsupportedFixtureSemantics { capability } => {
            writer.line("pre-attempt-capability", capability)
        }
        PreAttemptEvaluationOutcome::UnsupportedExpectation { surface } => {
            writer.line("pre-attempt-surface", surface_name(*surface))
        }
        PreAttemptEvaluationOutcome::EvaluationFailure { category, identity } => {
            writer.line(
                "pre-attempt-failure-category",
                execution_failure_category_name(*category),
            )?;
            writer.line("pre-attempt-identity", identity)
        }
    }
}

fn write_observed(
    writer: &mut BoundedWriter,
    observed: &ObservedExecutionOutcome,
) -> Result<(), ReportBuildError> {
    match observed {
        ObservedExecutionOutcome::SemanticPass => writer.line("observed", "semantic-pass"),
        ObservedExecutionOutcome::ExpectationMismatch {
            strategy,
            surface,
            difference,
        } => {
            writer.line("observed", "expectation-mismatch")?;
            writer.optional_line("observed-strategy", strategy.as_deref())?;
            writer.line("observed-surface", surface_name(*surface))?;
            writer.multiline("observed-difference", difference)
        }
        ObservedExecutionOutcome::ParityMismatch {
            strategy,
            surface,
            difference,
        } => {
            writer.line("observed", "parity-mismatch")?;
            writer.line("observed-strategy", strategy)?;
            writer.line("observed-surface", surface_name(*surface))?;
            writer.multiline("observed-difference", difference)
        }
        ObservedExecutionOutcome::ExecutionFailure { category, identity } => {
            writer.line("observed", "execution-failure")?;
            writer.line(
                "observed-failure-category",
                execution_failure_category_name(*category),
            )?;
            writer.line("observed-identity", identity)
        }
        ObservedExecutionOutcome::IncompleteObservation {
            strategy,
            surface,
            reason,
            retained,
            dropped,
        } => {
            writer.line("observed", "incomplete-observation")?;
            writer.optional_line("observed-strategy", strategy.as_deref())?;
            writer.optional_line("observed-surface", surface.map(surface_name))?;
            writer.line("observed-reason", incomplete_reason_name(*reason))?;
            writer.optional_usize("observed-retained", *retained)?;
            writer.optional_u64("observed-dropped", *dropped)
        }
        ObservedExecutionOutcome::FinalInvariantFailure {
            strategy,
            first,
            count,
        } => {
            writer.line("observed", "final-invariant-failure")?;
            writer.optional_line("observed-strategy", strategy.as_deref())?;
            writer.optional_line("observed-first-invariant", first.as_deref())?;
            writer.number("observed-invariant-count", usize::from(*count))
        }
    }
}

fn profile_name(value: ParserObservationProfile) -> &'static str {
    match value {
        ParserObservationProfile::HtmlTokenizer => "html-tokenizer",
        ParserObservationProfile::HtmlTreeConstruction => "html-tree-construction",
        ParserObservationProfile::DomTree => "dom-tree",
    }
}

pub(crate) fn surface_name(value: ParserObservationSurface) -> &'static str {
    match value {
        ParserObservationSurface::Tokens => "tokens",
        ParserObservationSurface::ParseErrors => "parse-errors",
        ParserObservationSurface::ImplementationDiagnostics => "implementation-diagnostics",
        ParserObservationSurface::DocumentMode => "document-mode",
        ParserObservationSurface::Tree => "tree",
        ParserObservationSurface::Patches => "patches",
        ParserObservationSurface::Transitions => "transitions",
        ParserObservationSurface::UnsupportedFeatures => "unsupported-features",
        ParserObservationSurface::FinalInvariants => "final-invariants",
    }
}

fn historical_not_attempted_reason_name(
    value: NotAttemptedReason,
) -> Result<&'static str, ReportBuildError> {
    match value {
        NotAttemptedReason::Eligibility => Ok("eligibility"),
        NotAttemptedReason::AePreExecutionEvaluation => Ok("ae-pre-execution-evaluation"),
        NotAttemptedReason::LaneExcluded => Err(ReportBuildError::UnsupportedReportCase {
            format: REPORT_FORMAT_V1,
        }),
    }
}

fn pre_attempt_name(value: &PreAttemptEvaluationOutcome) -> &'static str {
    match value {
        PreAttemptEvaluationOutcome::NotExecutedByAe { .. } => "not-executed-by-ae",
        PreAttemptEvaluationOutcome::UnsupportedFixtureSemantics { .. } => {
            "unsupported-fixture-semantics"
        }
        PreAttemptEvaluationOutcome::UnsupportedExpectation { .. } => "unsupported-expectation",
        PreAttemptEvaluationOutcome::EvaluationFailure { .. } => "evaluation-failure",
    }
}

fn execution_failure_category_name(value: NormalizedExecutionFailureCategory) -> &'static str {
    match value {
        NormalizedExecutionFailureCategory::SnapshotRead => "snapshot-read",
        NormalizedExecutionFailureCategory::SnapshotFormat => "snapshot-format",
        NormalizedExecutionFailureCategory::ParserObservation => "parser-observation",
        NormalizedExecutionFailureCategory::FixtureExecutionResourceExhaustion => {
            "fixture-execution-resource-exhaustion"
        }
        NormalizedExecutionFailureCategory::ValidatedFixtureInvariant => {
            "validated-fixture-invariant"
        }
        NormalizedExecutionFailureCategory::LegacyTokenizerDriver => "legacy-tokenizer-driver",
    }
}

fn incomplete_reason_name(value: NormalizedIncompleteObservationReason) -> &'static str {
    match value {
        NormalizedIncompleteObservationReason::LegacyNonAuthoritativeObservation => {
            "legacy-non-authoritative-observation"
        }
        NormalizedIncompleteObservationReason::StorageLimitExceeded => "storage-limit-exceeded",
    }
}

fn disposition_name(value: NormalizedAeDispositionContext) -> &'static str {
    match value {
        NormalizedAeDispositionContext::MatchedPass => "matched-pass",
        NormalizedAeDispositionContext::MatchedSkip => "matched-skip",
        NormalizedAeDispositionContext::UnexpectedOutcome => "unexpected-outcome",
        NormalizedAeDispositionContext::IncompleteObservation => "incomplete-observation",
        NormalizedAeDispositionContext::Xpass => "xpass",
    }
}

fn policy_name(value: DerivedPolicyResult) -> &'static str {
    match value {
        DerivedPolicyResult::ExpectedPass => "expected-pass",
        DerivedPolicyResult::UnexpectedFail => "unexpected-fail",
        DerivedPolicyResult::ExpectedFail => "xfail",
        DerivedPolicyResult::UnexpectedPass => "xpass",
        DerivedPolicyResult::NotRun => "not-run",
        DerivedPolicyResult::NotYetEstablished => "not-yet-established",
        DerivedPolicyResult::UnexpectedOutcome => "unexpected-outcome",
    }
}

#[cfg(all(test, feature = "rendering"))]
pub(crate) fn with_forced_allocation_failure<Output>(operation: impl FnOnce() -> Output) -> Output {
    crate::report_writer::with_forced_allocation_failure(operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conformance_test_support::{ObservationSurface, RequirementTag, TestId};

    fn case_with_artifact(bytes: &str) -> NormalizedCaseResult {
        NormalizedCaseResult {
            ag: AgCaseState {
                test_id: TestId::parse("parser-case").unwrap(),
                observation: ObservationSurface::HtmlTokenizer,
                classification: ClassificationCompleteness::NotYetClassified {
                    reason: "reason".to_owned(),
                },
                requirements: vec![],
                capability: None,
                harness: None,
                environment_requirements: vec![],
                stability: None,
                lane_exclusions: vec![],
                eligibility: Eligibility::NotYetEstablished { unresolved: vec![] },
                expectation: AgExpectation::NotEstablished,
            },
            variant: ExecutionVariantId::new(SingletonExecutionVariant::Singleton),
            profile: ParserObservationProfile::HtmlTokenizer,
            execution: ExecutionAttempt::eligibility_blocked(),
            observations: vec![ObservationArtifact {
                surface: ParserObservationSurface::Tokens,
                format: "html5-token-v2".to_owned(),
                bytes: bytes.to_owned(),
            }],
            ae_disposition: None,
            policy: DerivedPolicyResult::NotYetEstablished,
        }
    }

    #[test]
    fn observation_limit_accepts_exact_boundary_and_rejects_plus_one() {
        let limits = ReportLimits {
            total_bytes: 4096,
            observation_bytes: 4,
            mismatch_diagnostic_bytes: 16,
        };
        assert!(build_report_with_limits(&[case_with_artifact("1234")], limits).is_ok());
        assert!(matches!(
            build_report_with_limits(&[case_with_artifact("12345")], limits),
            Err(ReportBuildError::ObservationTooLarge {
                actual: 5,
                maximum: 4,
                ..
            })
        ));
    }

    #[test]
    fn mismatch_limit_accepts_exact_boundary_and_rejects_plus_one() {
        let mut exact = case_with_artifact("");
        exact.execution = ExecutionAttempt::Attempted {
            outcome: ObservedExecutionOutcome::ExpectationMismatch {
                strategy: Some("whole".to_owned()),
                surface: ParserObservationSurface::Tokens,
                difference: "1234".to_owned(),
            },
        };
        let limits = ReportLimits {
            total_bytes: 4096,
            observation_bytes: 4,
            mismatch_diagnostic_bytes: 4,
        };
        assert!(build_report_with_limits(&[exact.clone()], limits).is_ok());
        let mut oversized = exact;
        let ExecutionAttempt::Attempted {
            outcome: ObservedExecutionOutcome::ExpectationMismatch { difference, .. },
        } = &mut oversized.execution
        else {
            unreachable!()
        };
        difference.push('5');
        assert!(matches!(
            build_report_with_limits(&[oversized], limits),
            Err(ReportBuildError::MismatchDiagnosticTooLarge {
                actual: 5,
                maximum: 4,
                ..
            })
        ));
    }

    #[test]
    fn total_limit_is_checked_without_truncation() {
        let limits = ReportLimits {
            total_bytes: 32,
            observation_bytes: 16,
            mismatch_diagnostic_bytes: 16,
        };
        assert!(matches!(
            build_report_with_limits(&[case_with_artifact("x")], limits),
            Err(ReportBuildError::ReportTooLarge { maximum: 32 })
        ));
    }

    #[test]
    fn total_limit_accepts_the_exact_serialized_size_and_rejects_one_less() {
        let case = case_with_artifact("x");
        let measured = build_report_with_limits(std::slice::from_ref(&case), DEFAULT_REPORT_LIMITS)
            .unwrap()
            .len();
        let exact = ReportLimits {
            total_bytes: measured,
            observation_bytes: 16,
            mismatch_diagnostic_bytes: 16,
        };
        assert_eq!(
            build_report_with_limits(std::slice::from_ref(&case), exact)
                .unwrap()
                .len(),
            measured
        );
        let too_small = ReportLimits {
            total_bytes: measured - 1,
            ..exact
        };
        assert!(matches!(
            build_report_with_limits(&[case], too_small),
            Err(ReportBuildError::ReportTooLarge { maximum }) if maximum == measured - 1
        ));
    }

    #[test]
    fn report_is_repeatable_lf_utf8_and_has_no_debug_output() {
        let cases = [case_with_artifact("line one\nline two")];
        let first = build_report_with_limits(&cases, DEFAULT_REPORT_LIMITS).unwrap();
        let second = build_report_with_limits(&cases, DEFAULT_REPORT_LIMITS).unwrap();
        assert_eq!(first, second);
        assert!(!first.contains(&b'\r'));
        assert!(std::str::from_utf8(&first).is_ok());
        assert!(
            std::str::from_utf8(&first)
                .unwrap()
                .contains("line one\\nline two")
        );
    }

    struct CountingWriter {
        accepted: usize,
        fail_after: Option<usize>,
    }

    impl std::io::Write for CountingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let allowed = self.fail_after.map_or(bytes.len(), |limit| {
                limit.saturating_sub(self.accepted).min(bytes.len())
            });
            if allowed == 0 {
                return Err(std::io::Error::other("synthetic transport failure"));
            }
            self.accepted += allowed;
            Ok(allowed)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn build_failure_writes_no_bytes_and_transport_failure_may_accept_prefix() {
        let oversized = [case_with_artifact(
            &"x".repeat(DEFAULT_REPORT_LIMITS.observation_bytes + 1),
        )];
        let mut untouched = CountingWriter {
            accepted: 0,
            fail_after: None,
        };
        assert!(matches!(
            build_and_write_report(&oversized, &mut untouched),
            Err(ReportPublicationError::Build(
                ReportBuildError::ObservationTooLarge { .. }
            ))
        ));
        assert_eq!(untouched.accepted, 0);

        let mut prefix_writer = CountingWriter {
            accepted: 0,
            fail_after: Some(7),
        };
        assert!(matches!(
            build_and_write_report(&[case_with_artifact("small")], &mut prefix_writer),
            Err(ReportPublicationError::OutputWrite(_))
        ));
        assert_eq!(prefix_writer.accepted, 7);
    }

    #[test]
    fn allocation_failure_is_a_build_failure_and_publishes_nothing() {
        let mut output = CountingWriter {
            accepted: 0,
            fail_after: None,
        };
        crate::report_writer::with_forced_allocation_failure(|| {
            assert!(matches!(
                build_and_write_report(&[case_with_artifact("small")], &mut output),
                Err(ReportPublicationError::Build(
                    ReportBuildError::AllocationFailure
                ))
            ));
        });
        assert_eq!(output.accepted, 0);
    }

    #[test]
    fn optional_codec_distinguishes_absence_strings_and_zero_exactly() {
        let mut writer = BoundedWriter::new(1024).unwrap();
        writer.optional_line("missing", None).unwrap();
        writer.optional_line("none-string", Some("none")).unwrap();
        writer.optional_line("null-string", Some("null")).unwrap();
        writer.optional_usize("missing-number", None).unwrap();
        writer.optional_usize("zero", Some(0)).unwrap();
        writer.optional_u64("missing-u64", None).unwrap();
        writer.optional_u64("zero-u64", Some(0)).unwrap();
        assert_eq!(
            writer.finish(),
            b"missing = null\nnone-string = \"none\"\nnull-string = \"null\"\nmissing-number = null\nzero = 0\nmissing-u64 = null\nzero-u64 = 0\n"
        );
    }

    #[test]
    fn retained_evidence_is_bounded_before_accumulation() {
        let limits = ReportLimits {
            total_bytes: 5,
            observation_bytes: 4,
            mismatch_diagnostic_bytes: 2,
        };
        let mut budget = RetainedEvidenceBudget::new(limits);
        assert!(matches!(
            budget.retain_observation("case-a", ParserObservationSurface::Tree, 5),
            Err(ReportBuildError::ObservationTooLarge {
                actual: 5,
                maximum: 4,
                ..
            })
        ));
        assert_eq!(budget.retained, 0, "rejected evidence is never accumulated");
        budget
            .retain_observation("case-b", ParserObservationSurface::Tree, 4)
            .unwrap();
        assert!(matches!(
            budget.retain_mismatch("case-c", 2),
            Err(ReportBuildError::RetainedEvidenceTooLarge {
                actual: 6,
                maximum: 5,
            })
        ));
        assert_eq!(budget.retained, 4, "aggregate failure is non-mutating");
        budget.retain_mismatch("case-d", 1).unwrap();
        assert_eq!(budget.retained, 5, "exact aggregate boundary is accepted");
    }

    #[test]
    fn parser_report_v1_has_an_exact_byte_contract() {
        let case = NormalizedCaseResult {
            ag: AgCaseState {
                test_id: TestId::parse("typed-parser-case").unwrap(),
                observation: ObservationSurface::HtmlTokenizer,
                classification: ClassificationCompleteness::Classified,
                requirements: vec![RequirementTag::NoJs],
                capability: Some(CapabilityAvailability::Available),
                harness: Some(HarnessReadiness::Ready),
                environment_requirements: vec![],
                stability: Some(Stability::Stable),
                lane_exclusions: vec![],
                eligibility: Eligibility::Runnable,
                expectation: AgExpectation::ExpectedPass,
            },
            variant: ExecutionVariantId::new(SingletonExecutionVariant::Singleton),
            profile: ParserObservationProfile::HtmlTokenizer,
            execution: ExecutionAttempt::Attempted {
                outcome: ObservedExecutionOutcome::ExpectationMismatch {
                    strategy: Some("whole".to_owned()),
                    surface: ParserObservationSurface::Tokens,
                    difference: "line one\n\"quoted\" \\ slash".to_owned(),
                },
            },
            observations: vec![ObservationArtifact {
                surface: ParserObservationSurface::Tokens,
                format: "html5-token-v2".to_owned(),
                bytes: "TOKEN \\\"x\\\"\nEOF".to_owned(),
            }],
            ae_disposition: Some(NormalizedAeDispositionContext::UnexpectedOutcome),
            policy: DerivedPolicyResult::UnexpectedFail,
        };
        let expected = include_bytes!("../tests/data/parser-report-v1-compat.txt");
        assert_eq!(build_report(&[case]).unwrap(), expected);
    }

    #[test]
    fn parser_report_v1_rejects_named_lane_only_not_attempted_state() {
        let mut case = case_with_artifact("");
        case.execution = ExecutionAttempt::NotAttempted {
            reason: NotAttemptedReason::LaneExcluded,
            pre_attempt: None,
        };
        assert!(matches!(
            build_report(&[case]),
            Err(ReportBuildError::UnsupportedReportCase {
                format: REPORT_FORMAT_V1,
            })
        ));
    }
}
