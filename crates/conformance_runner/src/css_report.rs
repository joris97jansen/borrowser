use std::io::Write;

use css_test_support::{CssExecutionFailure, CssExecutionPhase, CssObservedExecutionOutcome};

use crate::css_runner::{CssCaseResult, CssExecutionAttempt};
use crate::model::*;
use crate::report::{
    BoundedWriter, DEFAULT_REPORT_LIMITS, ReportBuildError, ReportPublicationError,
};

pub const CSS_REPORT_FORMAT_V1: &str = "borrowser-conformance-css-report-v1";

pub fn build_css_report(cases: &[CssCaseResult]) -> Result<Vec<u8>, ReportBuildError> {
    for case in cases {
        if let Some(observation) = &case.observation
            && observation.bytes.len() > DEFAULT_REPORT_LIMITS.observation_bytes
        {
            return Err(ReportBuildError::ObservationTooLarge {
                test_id: case.ag.test_id.as_str().to_owned(),
                surface: case.ag.observation.as_str().to_owned(),
                actual: observation.bytes.len(),
                maximum: DEFAULT_REPORT_LIMITS.observation_bytes,
            });
        }
        if let CssExecutionAttempt::Attempted {
            outcome: CssObservedExecutionOutcome::ExpectationMismatch { difference },
        } = &case.execution
            && difference.len() > DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes
        {
            return Err(ReportBuildError::MismatchDiagnosticTooLarge {
                test_id: case.ag.test_id.as_str().to_owned(),
                actual: difference.len(),
                maximum: DEFAULT_REPORT_LIMITS.mismatch_diagnostic_bytes,
            });
        }
    }
    let mut writer = BoundedWriter::new(DEFAULT_REPORT_LIMITS.total_bytes)?;
    writer.line("format", CSS_REPORT_FORMAT_V1)?;
    writer.number("case-count", cases.len())?;
    for case in cases {
        writer.raw("\nBEGIN case\n")?;
        writer.line("test-id", case.ag.test_id.as_str())?;
        writer.line("observation", case.ag.observation.as_str())?;
        writer.optional_line("profile", case.profile.map(profile_name))?;
        write_ag(&mut writer, &case.ag)?;
        write_execution(&mut writer, &case.execution)?;
        writer.line("policy", policy_name(case.policy))?;
        match &case.observation {
            Some(observation) => {
                writer.number("observation-count", 1)?;
                writer.raw("BEGIN observation\n")?;
                writer.line("snapshot-format", observation.format)?;
                writer.line("bytes", &observation.bytes)?;
                writer.raw("END observation\n")?;
            }
            None => writer.number("observation-count", 0)?,
        }
        writer.raw("END case\n")?;
    }
    Ok(writer.finish())
}

pub fn build_and_write_css_report(
    cases: &[CssCaseResult],
    output: &mut impl Write,
) -> Result<(), ReportPublicationError> {
    let bytes = build_css_report(cases).map_err(ReportPublicationError::Build)?;
    output
        .write_all(&bytes)
        .map_err(ReportPublicationError::OutputWrite)
}

fn write_ag(writer: &mut BoundedWriter, ag: &AgCaseState) -> Result<(), ReportBuildError> {
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
        None => writer.optional_line("engine", None)?,
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
        None => writer.optional_line("harness", None)?,
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
    match &ag.stability {
        None => writer.optional_line("stability", None)?,
        Some(Stability::Stable) => writer.line("stability", "stable")?,
        Some(Stability::NotYetEstablished) => writer.line("stability", "not-yet-established")?,
        Some(Stability::Flaky { reason }) => {
            writer.line("stability", "flaky")?;
            writer.line("stability-reason", reason)?;
        }
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
                write_fact(writer, "blocker", fact)?;
            }
            for fact in unresolved {
                write_fact(writer, "unresolved", fact)?;
            }
        }
        Eligibility::NotYetEstablished { unresolved } => {
            writer.line("eligibility", "not-yet-established")?;
            for fact in unresolved {
                write_fact(writer, "unresolved", fact)?;
            }
        }
    }
    match &ag.expectation {
        AgExpectation::ExpectedPass => writer.line("expectation", "expected-pass")?,
        AgExpectation::NotEstablished => writer.line("expectation", "not-established")?,
        AgExpectation::ExpectedFail { failure, reason } => {
            writer.line("expectation", "expected-fail")?;
            writer.line("expected-failure", failure.as_str())?;
            writer.line("expectation-reason", reason)?;
        }
    }
    Ok(())
}

fn write_fact(
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

fn write_execution(
    writer: &mut BoundedWriter,
    execution: &CssExecutionAttempt,
) -> Result<(), ReportBuildError> {
    match execution {
        CssExecutionAttempt::NotAttempted {
            reason,
            pre_attempt,
        } => {
            writer.line("attempt", "not-attempted")?;
            writer.line(
                "not-attempted-reason",
                match reason {
                    crate::css_runner::CssNotAttemptedReason::Eligibility => "eligibility",
                    crate::css_runner::CssNotAttemptedReason::FragmentCapabilityUnavailable => {
                        "fragment-capability-unavailable"
                    }
                    crate::css_runner::CssNotAttemptedReason::LaneExcluded => {
                        return Err(ReportBuildError::UnsupportedReportCase {
                            format: CSS_REPORT_FORMAT_V1,
                        });
                    }
                },
            )?;
            writer.optional_line(
                "pre-attempt-outcome",
                pre_attempt.map(|_| "fragment-capability-unavailable"),
            )?;
            writer.optional_line("observed", None)?;
        }
        CssExecutionAttempt::Attempted { outcome } => {
            writer.line("attempt", "attempted")?;
            writer.optional_line("not-attempted-reason", None)?;
            writer.optional_line("pre-attempt-outcome", None)?;
            match outcome {
                CssObservedExecutionOutcome::SemanticPass => {
                    writer.line("observed", "semantic-pass")?
                }
                CssObservedExecutionOutcome::ExpectationMismatch { difference } => {
                    writer.line("observed", "expectation-mismatch")?;
                    writer.line("observed-difference", difference)?;
                }
                CssObservedExecutionOutcome::ExecutionFailure { phase, failure } => {
                    writer.line("observed", "execution-failure")?;
                    writer.line("failure-phase", phase_name(*phase))?;
                    writer.line("failure-kind", failure_name(failure))?;
                    write_failure_evidence(writer, failure)?;
                }
                CssObservedExecutionOutcome::IncompleteObservation { phase, failure } => {
                    writer.line("observed", "incomplete-observation")?;
                    writer.line("failure-phase", phase_name(*phase))?;
                    writer.line("failure-kind", failure_name(failure))?;
                    write_failure_evidence(writer, failure)?;
                }
                CssObservedExecutionOutcome::FinalInvariantFailure { phase, failure } => {
                    writer.line("observed", "final-invariant-failure")?;
                    writer.line("failure-phase", phase_name(*phase))?;
                    writer.line("failure-kind", failure_name(failure))?;
                    write_failure_evidence(writer, failure)?;
                }
            }
        }
    }
    Ok(())
}

fn write_failure_evidence(
    writer: &mut BoundedWriter,
    failure: &CssExecutionFailure,
) -> Result<(), ReportBuildError> {
    match failure {
        CssExecutionFailure::HtmlParser(_)
        | CssExecutionFailure::SelectorProjection(_)
        | CssExecutionFailure::SelectorMatching(_)
        | CssExecutionFailure::RuleCollection(_)
        | CssExecutionFailure::StyleResolution(_)
        | CssExecutionFailure::ProjectionArtifact(_)
        | CssExecutionFailure::ComputedMaterialization(_)
        | CssExecutionFailure::RequiredObservation(_) => writer.line(
            "failure-identity",
            failure
                .stable_identity()
                .expect("typed identity-bearing failure"),
        )?,
        CssExecutionFailure::HtmlSemanticInputResourceLimited(reasons) => writer.list(
            "html-semantic-degradation-reasons",
            reasons.reasons().map(|reason| reason.stable_label()),
        )?,
        CssExecutionFailure::TargetResolution { label, failure } => {
            writer.line("target-label", label)?;
            match failure {
                css_test_support::CssTargetResolutionFailure::EmptyAddress => {
                    writer.line("target-failure", "empty-address")?
                }
                css_test_support::CssTargetResolutionFailure::ChildMissing {
                    depth,
                    child_index,
                } => {
                    writer.line("target-failure", "child-missing")?;
                    writer.number("target-depth", *depth)?;
                    writer.number("target-child-index", *child_index)?;
                }
                css_test_support::CssTargetResolutionFailure::ChildIsNotElement {
                    depth,
                    child_index,
                    actual,
                } => {
                    writer.line("target-failure", "child-is-not-element")?;
                    writer.number("target-depth", *depth)?;
                    writer.number("target-child-index", *child_index)?;
                    writer.line(
                        "target-actual-child-kind",
                        match actual {
                            css_test_support::CssTargetChildKind::Document => "document",
                            css_test_support::CssTargetChildKind::DocumentType => "document-type",
                            css_test_support::CssTargetChildKind::Text => "text",
                            css_test_support::CssTargetChildKind::Comment => "comment",
                            css_test_support::CssTargetChildKind::ProcessingInstruction => {
                                "processing-instruction"
                            }
                        },
                    )?;
                }
                css_test_support::CssTargetResolutionFailure::NamespaceMismatch {
                    depth,
                    child_index,
                    expected,
                    actual,
                } => {
                    writer.line("target-failure", "namespace-mismatch")?;
                    writer.number("target-depth", *depth)?;
                    writer.number("target-child-index", *child_index)?;
                    writer.line("target-expected-namespace", expected.stable_label())?;
                    writer.line("target-actual-namespace", actual.snapshot_name())?;
                }
                css_test_support::CssTargetResolutionFailure::LocalNameMismatch {
                    depth,
                    child_index,
                    expected,
                    actual,
                } => {
                    writer.line("target-failure", "local-name-mismatch")?;
                    writer.number("target-depth", *depth)?;
                    writer.number("target-child-index", *child_index)?;
                    writer.line("target-expected-local-name", expected)?;
                    writer.line("target-actual-local-name", actual)?;
                }
            }
        }
        CssExecutionFailure::ResourceLimit { resource } => {
            writer.line("resource-limit", resource.stable_label())?
        }
        CssExecutionFailure::StorageAllocation { storage } => writer.line(
            "failure-storage",
            match storage {
                css_test_support::CssExecutionStorage::ParsedStylesheets => "parsed-stylesheets",
                css_test_support::CssExecutionStorage::StylesheetInputs => "stylesheet-inputs",
            },
        )?,
        CssExecutionFailure::ObservationLimitExceeded { actual, maximum } => {
            writer.number("observation-actual-bytes", *actual)?;
            writer.number("observation-maximum-bytes", *maximum)?;
        }
        CssExecutionFailure::ObservationAllocationFailure => {}
    }
    Ok(())
}

fn profile_name(value: css_test_support::CssExecutionProfile) -> &'static str {
    value.stable_label()
}
fn policy_name(value: DerivedPolicyResult) -> &'static str {
    match value {
        DerivedPolicyResult::ExpectedPass => "expected-pass",
        DerivedPolicyResult::UnexpectedFail => "unexpected-fail",
        DerivedPolicyResult::ExpectedFail => "expected-fail",
        DerivedPolicyResult::UnexpectedPass => "unexpected-pass",
        DerivedPolicyResult::NotRun => "not-run",
        DerivedPolicyResult::NotYetEstablished => "not-yet-established",
        DerivedPolicyResult::UnexpectedOutcome => "unexpected-outcome",
    }
}
fn phase_name(value: CssExecutionPhase) -> &'static str {
    match value {
        CssExecutionPhase::HtmlDocumentParsing => "html-document-parsing",
        CssExecutionPhase::TargetResolution => "target-resolution",
        CssExecutionPhase::CssModelParsing => "css-model-parsing",
        CssExecutionPhase::SelectorParsing => "selector-parsing",
        CssExecutionPhase::SelectorProjection => "selector-projection",
        CssExecutionPhase::SelectorMatching => "selector-matching",
        CssExecutionPhase::RuleCollection => "rule-collection",
        CssExecutionPhase::Cascade => "cascade",
        CssExecutionPhase::ResolvedStyleObservation => "resolved-style-observation",
        CssExecutionPhase::ComputedStyle => "computed-style",
        CssExecutionPhase::ObservationSerialization => "observation-serialization",
    }
}
fn failure_name(value: &CssExecutionFailure) -> &'static str {
    match value {
        CssExecutionFailure::HtmlParser(_) => "html-parser",
        CssExecutionFailure::HtmlSemanticInputResourceLimited(_) => {
            "html-semantic-input-resource-limited"
        }
        CssExecutionFailure::TargetResolution { .. } => "target-resolution",
        CssExecutionFailure::ResourceLimit { .. } => "resource-limit",
        CssExecutionFailure::SelectorProjection(_) => "selector-projection",
        CssExecutionFailure::SelectorMatching(_) => "selector-matching",
        CssExecutionFailure::RuleCollection(_) => "rule-collection",
        CssExecutionFailure::StyleResolution(_) => "style-resolution",
        CssExecutionFailure::ProjectionArtifact(_) => "projection-artifact",
        CssExecutionFailure::ComputedMaterialization(_) => "computed-materialization",
        CssExecutionFailure::RequiredObservation(_) => "required-observation",
        CssExecutionFailure::StorageAllocation { .. } => "storage-allocation",
        CssExecutionFailure::ObservationLimitExceeded { .. } => "observation-limit-exceeded",
        CssExecutionFailure::ObservationAllocationFailure => "observation-allocation-failure",
    }
}

#[cfg(test)]
mod tests {
    use conformance_test_support::ObservationSurface;

    use super::*;

    fn attempted_failure(phase: CssExecutionPhase, failure: CssExecutionFailure) -> CssCaseResult {
        CssCaseResult {
            ag: AgCaseState {
                test_id: conformance_test_support::TestId::parse("typed-css-failure").unwrap(),
                observation: ObservationSurface::CssSelectors,
                classification: ClassificationCompleteness::Classified,
                requirements: vec![],
                capability: Some(CapabilityAvailability::Available),
                harness: Some(HarnessReadiness::Ready),
                environment_requirements: vec![],
                stability: Some(Stability::NotYetEstablished),
                lane_exclusions: vec![],
                eligibility: Eligibility::Runnable,
                expectation: AgExpectation::ExpectedPass,
            },
            variant: ExecutionVariantId::new(SingletonExecutionVariant::Singleton),
            profile: Some(css_test_support::CssExecutionProfile::SelectorMatching),
            execution: CssExecutionAttempt::Attempted {
                outcome: CssObservedExecutionOutcome::ExecutionFailure { phase, failure },
            },
            observation: None,
            policy: DerivedPolicyResult::UnexpectedOutcome,
        }
    }

    #[test]
    fn css_report_v1_has_an_exact_byte_contract_and_omits_singleton_variant_identity() {
        let case = CssCaseResult {
            ag: AgCaseState {
                test_id: conformance_test_support::TestId::parse("typed-css-case").unwrap(),
                observation: ObservationSurface::CssSelectors,
                classification: ClassificationCompleteness::NotYetClassified {
                    reason: "classification pending".to_owned(),
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
            profile: None,
            execution: CssExecutionAttempt::NotAttempted {
                reason: crate::css_runner::CssNotAttemptedReason::Eligibility,
                pre_attempt: None,
            },
            observation: None,
            policy: DerivedPolicyResult::NotYetEstablished,
        };
        assert_eq!(
            build_css_report(&[case]).unwrap(),
            concat!(
                "format = \"borrowser-conformance-css-report-v1\"\n",
                "case-count = 1\n",
                "\nBEGIN case\n",
                "test-id = \"typed-css-case\"\n",
                "observation = \"css-selectors\"\n",
                "profile = null\n",
                "classification = \"not-yet-classified\"\n",
                "classification-reason = \"classification pending\"\n",
                "requirements = []\n",
                "engine = null\n",
                "harness = null\n",
                "stability = null\n",
                "eligibility = \"not-yet-established\"\n",
                "expectation = \"not-established\"\n",
                "attempt = \"not-attempted\"\n",
                "not-attempted-reason = \"eligibility\"\n",
                "pre-attempt-outcome = null\n",
                "observed = null\n",
                "policy = \"not-yet-established\"\n",
                "observation-count = 0\n",
                "END case\n",
            )
            .as_bytes()
        );
    }

    #[test]
    fn css_report_v1_rejects_named_lane_only_not_attempted_state() {
        let mut case = attempted_failure(
            CssExecutionPhase::SelectorParsing,
            CssExecutionFailure::ResourceLimit {
                resource: css_test_support::CssExecutionResourceLimit::SelectorParsing,
            },
        );
        case.execution = CssExecutionAttempt::NotAttempted {
            reason: crate::css_runner::CssNotAttemptedReason::LaneExcluded,
            pre_attempt: None,
        };
        assert!(matches!(
            build_css_report(&[case]),
            Err(ReportBuildError::UnsupportedReportCase {
                format: CSS_REPORT_FORMAT_V1,
            })
        ));
    }

    #[test]
    fn css_report_serializes_typed_failures_without_debug_codec() {
        let report = build_css_report(&[attempted_failure(
            CssExecutionPhase::SelectorParsing,
            CssExecutionFailure::ResourceLimit {
                resource: css_test_support::CssExecutionResourceLimit::SelectorParsing,
            },
        )])
        .expect("CSS failure report");
        let report = std::str::from_utf8(&report).expect("UTF-8 report");
        assert!(report.starts_with("format = \"borrowser-conformance-css-report-v1\"\n"));
        assert!(report.contains("resource-limit = \"selector-parsing\""));
        assert!(report.contains("attempt = \"attempted\""));
        assert!(report.contains("observed = \"execution-failure\""));
        assert!(!report.contains("ResourceLimit"));
    }

    #[test]
    fn css_report_preserves_structural_target_failure_coordinates() {
        let report = build_css_report(&[attempted_failure(
            CssExecutionPhase::TargetResolution,
            CssExecutionFailure::TargetResolution {
                label: "article".to_owned(),
                failure: css_test_support::CssTargetResolutionFailure::ChildIsNotElement {
                    depth: 3,
                    child_index: 1,
                    actual: css_test_support::CssTargetChildKind::Comment,
                },
            },
        )])
        .expect("structural target failure report");
        let report = std::str::from_utf8(&report).expect("UTF-8 report");
        assert!(report.contains("target-failure = \"child-is-not-element\""));
        assert!(report.contains("target-depth = 3"));
        assert!(report.contains("target-child-index = 1"));
        assert!(report.contains("target-actual-child-kind = \"comment\""));
    }
}
