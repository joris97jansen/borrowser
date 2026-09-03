use std::cmp::Ordering;
use std::io::Write;

use conformance_test_support::{
    LanePolicyScope, ObservationSurface, ReferenceKind, ReferenceRelation, SubsystemOwner,
};

use crate::report_writer::{CanonicalReportWriter, CanonicalReportWriterFailure};
use crate::{
    AgExpectation, CapabilityAvailability, ClassificationCompleteness, DerivedPolicyResult,
    Eligibility, EligibilityFact, HarnessReadiness, Stability,
};

use super::{
    AggregateAccounting, AggregateCaseResult, AggregateComparisonKind, AggregateExecutionAttempt,
    AggregateExecutionVariantId, AggregateNotAttemptedReason, AggregateRun,
    AggregateTerminalOutcome, AggregateVariantResult, LaneSelection,
    model::{aggregate_variant_result_cmp, validate_ag3_case_state},
};

pub const AGGREGATE_SUMMARY_FORMAT_V1: &str = "borrowser-conformance-aggregate-summary-v1";
pub const AGGREGATE_DETAIL_FORMAT_V1: &str = "borrowser-conformance-aggregate-detail-v1";
pub const AGGREGATE_POPULATION_IDENTITY_CONTRACT_V1: &str =
    "borrowser-conformance-logical-case-membership-v1";
pub const AGGREGATE_GRANULARITY_CONTRACT_V1: &str =
    "borrowser-conformance-aggregate-granularity-v1";
pub const AGGREGATE_SUMMARY_MAX_BYTES_V1: usize = 6_073;
pub const AGGREGATE_DETAIL_MAX_BYTES_V1: usize = 32 * 1024 * 1024;

type AggregateWriter = CanonicalReportWriter<AggregateReportBuildError>;

const OWNERS_V1: [SubsystemOwner; 5] = [
    SubsystemOwner::HtmlParser,
    SubsystemOwner::Css,
    SubsystemOwner::Layout,
    SubsystemOwner::Paint,
    SubsystemOwner::BrowserRuntime,
];
const OBSERVATIONS_V1: [ObservationSurface; 10] = [
    ObservationSurface::HtmlTokenizer,
    ObservationSurface::HtmlTreeConstruction,
    ObservationSurface::DomTree,
    ObservationSurface::CssParsing,
    ObservationSurface::CssSelectors,
    ObservationSurface::CssCascade,
    ObservationSurface::ComputedStyle,
    ObservationSurface::LayoutGeometry,
    ObservationSurface::PaintOperations,
    ObservationSurface::BrowserRuntimeSemantic,
];
const COMPARISONS_V1: [AggregateComparisonKind; 5] = [
    AggregateComparisonKind::AuthoredExpectedObservation,
    AggregateComparisonKind::StaticDocumentReference {
        reference_kind: ReferenceKind::Semantic,
        relation: ReferenceRelation::Match,
    },
    AggregateComparisonKind::StaticDocumentReference {
        reference_kind: ReferenceKind::Semantic,
        relation: ReferenceRelation::Mismatch,
    },
    AggregateComparisonKind::StaticDocumentReference {
        reference_kind: ReferenceKind::Structural,
        relation: ReferenceRelation::Match,
    },
    AggregateComparisonKind::StaticDocumentReference {
        reference_kind: ReferenceKind::Structural,
        relation: ReferenceRelation::Mismatch,
    },
];
const TERMINALS_V1: [AggregateTerminalOutcome; 7] = [
    AggregateTerminalOutcome::SemanticPass,
    AggregateTerminalOutcome::SemanticFail,
    AggregateTerminalOutcome::ExecutionFailure,
    AggregateTerminalOutcome::ResourceFailure,
    AggregateTerminalOutcome::IncompleteObservation,
    AggregateTerminalOutcome::InvariantFailure,
    AggregateTerminalOutcome::Timeout,
];

#[derive(Debug)]
pub enum AggregateReportBuildError {
    ReportTooLarge { maximum: usize },
    CountConversion,
    AllocationFailure,
    InvalidAggregateRun(&'static str),
}

impl std::fmt::Display for AggregateReportBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReportTooLarge { maximum } => {
                write!(
                    formatter,
                    "aggregate report exceeds its fixed {maximum}-byte limit"
                )
            }
            Self::CountConversion => {
                formatter.write_str("aggregate report count conversion failed")
            }
            Self::AllocationFailure => formatter.write_str("aggregate report allocation failed"),
            Self::InvalidAggregateRun(problem) => {
                write!(formatter, "aggregate report invariant failed: {problem}")
            }
        }
    }
}

impl std::error::Error for AggregateReportBuildError {}

impl CanonicalReportWriterFailure for AggregateReportBuildError {
    fn report_too_large(maximum: usize) -> Self {
        Self::ReportTooLarge { maximum }
    }

    fn allocation_failure() -> Self {
        Self::AllocationFailure
    }
}

#[derive(Debug)]
pub enum AggregateReportPublicationError {
    Build(AggregateReportBuildError),
    OutputWrite(std::io::Error),
}

impl std::fmt::Display for AggregateReportPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Build(error) => {
                write!(formatter, "aggregate report construction failed: {error}")
            }
            Self::OutputWrite(error) => {
                write!(formatter, "aggregate report output failed: {error}")
            }
        }
    }
}

impl std::error::Error for AggregateReportPublicationError {}

pub fn build_aggregate_summary_v1(
    run: &AggregateRun,
) -> Result<Vec<u8>, AggregateReportBuildError> {
    run.validate_ag3_projection_invariants().map_err(|_| {
        AggregateReportBuildError::InvalidAggregateRun("AG3 case-state invariant failed")
    })?;
    let mut writer = AggregateWriter::new(AGGREGATE_SUMMARY_MAX_BYTES_V1)?;
    write_common_header(&mut writer, run, AGGREGATE_SUMMARY_FORMAT_V1)?;
    write_accounting(&mut writer, run.accounting())?;
    Ok(writer.finish())
}

pub fn build_aggregate_detail_v1(run: &AggregateRun) -> Result<Vec<u8>, AggregateReportBuildError> {
    run.validate_ag3_projection_invariants().map_err(|_| {
        AggregateReportBuildError::InvalidAggregateRun("AG3 case-state invariant failed")
    })?;
    let mut writer = AggregateWriter::new(AGGREGATE_DETAIL_MAX_BYTES_V1)?;
    write_common_header(&mut writer, run, AGGREGATE_DETAIL_FORMAT_V1)?;
    write_accounting(&mut writer, run.accounting())?;
    let mut cases = Vec::new();
    cases
        .try_reserve(run.cases().len())
        .map_err(|_| AggregateReportBuildError::AllocationFailure)?;
    cases.extend(run.cases());
    cases.sort_unstable_by(|left, right| test_id_cmp(left, right));
    if cases
        .windows(2)
        .any(|pair| test_id_cmp(pair[0], pair[1]) == Ordering::Equal)
    {
        return Err(AggregateReportBuildError::InvalidAggregateRun(
            "logical TestId values are not unique",
        ));
    }
    write_usize_number(&mut writer, "logical-case-detail-count", cases.len())?;
    for case in cases {
        write_case(&mut writer, case)?;
    }
    Ok(writer.finish())
}

pub fn build_and_write_aggregate_summary_v1(
    run: &AggregateRun,
    output: &mut impl Write,
) -> Result<(), AggregateReportPublicationError> {
    let report = build_aggregate_summary_v1(run).map_err(AggregateReportPublicationError::Build)?;
    output
        .write_all(&report)
        .map_err(AggregateReportPublicationError::OutputWrite)
}

pub fn build_and_write_aggregate_detail_v1(
    run: &AggregateRun,
    output: &mut impl Write,
) -> Result<(), AggregateReportPublicationError> {
    let report = build_aggregate_detail_v1(run).map_err(AggregateReportPublicationError::Build)?;
    output
        .write_all(&report)
        .map_err(AggregateReportPublicationError::OutputWrite)
}

fn write_common_header(
    writer: &mut AggregateWriter,
    run: &AggregateRun,
    format: &'static str,
) -> Result<(), AggregateReportBuildError> {
    writer.line("format", format)?;
    writer.line("inventory-scope", run.inventory_scope().as_str())?;
    writer.line(
        "aggregate-granularity-contract",
        AGGREGATE_GRANULARITY_CONTRACT_V1,
    )?;
    writer.line("named-lane", run.request().lane.as_str())?;
    writer.line(
        "environment-assessment",
        run.environment_assessment_mode().as_str(),
    )?;
    writer.line(
        "population-identity-contract",
        AGGREGATE_POPULATION_IDENTITY_CONTRACT_V1,
    )?;
    writer.prefixed_hex_line(
        "logical-case-source-set-digest",
        "sha256:",
        run.logical_case_source_set_digest().as_sha256().as_bytes(),
    )?;
    writer.boolean("headline-counts-overlap", true)
}

fn write_accounting(
    writer: &mut AggregateWriter,
    accounting: &AggregateAccounting,
) -> Result<(), AggregateReportBuildError> {
    writer.line("logical-case-population", "logical-case")?;
    writer.line("execution-variant-population", "execution-variant")?;
    writer.u64_number("accounting-count-field-count", 59)?;
    writer.u64_number("subsystem-row-count", 5)?;
    writer.u64_number("observation-row-count", 10)?;
    writer.u64_number("comparison-row-count", 5)?;
    writer.u64_number("terminal-row-count", 7)?;
    writer.raw(b"BEGIN logical-accounting\n")?;
    writer.u64_number("total", accounting.logical.total_tests)?;
    writer.u64_number("pass", accounting.logical.pass_count)?;
    writer.u64_number("fail", accounting.logical.fail_count)?;
    writer.u64_number("expected-fail", accounting.logical.expected_fail_count)?;
    writer.u64_number("unsupported", accounting.logical.unsupported_count)?;
    writer.u64_number("skipped", accounting.logical.skipped_count)?;
    writer.u64_number("flaky", accounting.logical.flaky_count)?;
    writer.u64_number("unclassified", accounting.logical.unclassified_count)?;
    writer.raw(b"END logical-accounting\n")?;

    writer.raw(b"BEGIN execution-variant-accounting\n")?;
    writer.u64_number("materialized", accounting.variants.materialized_variants)?;
    writer.u64_number("runnable", accounting.variants.runnable_variants)?;
    writer.u64_number("not-runnable", accounting.variants.not_runnable_variants)?;
    writer.u64_number(
        "eligibility-not-established",
        accounting.variants.eligibility_not_established_variants,
    )?;
    writer.u64_number("selected", accounting.variants.selected_variants)?;
    writer.u64_number("excluded", accounting.variants.excluded_variants)?;
    writer.u64_number(
        "selection-not-applicable",
        accounting.variants.selection_not_applicable_variants,
    )?;
    writer.u64_number("attempted", accounting.variants.attempted_variants)?;
    writer.u64_number("not-attempted", accounting.variants.not_attempted_variants)?;
    writer.raw(b"END execution-variant-accounting\n")?;

    for owner in OWNERS_V1 {
        writer.raw(b"BEGIN subsystem\n")?;
        writer.line("owner", owner.as_str())?;
        writer.line("logical-domain", "logical-case")?;
        writer.line("variant-domain", "execution-variant")?;
        writer.u64_number(
            "logical-cases",
            map_count(&accounting.groupings.logical_cases_by_subsystem, &owner),
        )?;
        writer.u64_number(
            "execution-variants",
            map_count(&accounting.groupings.variants_by_subsystem, &owner),
        )?;
        writer.raw(b"END subsystem\n")?;
    }
    for observation in OBSERVATIONS_V1 {
        writer.raw(b"BEGIN observation\n")?;
        writer.line("surface", observation.as_str())?;
        writer.line("logical-domain", "logical-case")?;
        writer.line("variant-domain", "execution-variant")?;
        writer.u64_number(
            "logical-cases",
            map_count(
                &accounting.groupings.logical_cases_by_observation,
                &observation,
            ),
        )?;
        writer.u64_number(
            "execution-variants",
            map_count(&accounting.groupings.variants_by_observation, &observation),
        )?;
        writer.raw(b"END observation\n")?;
    }
    for comparison in COMPARISONS_V1 {
        writer.raw(b"BEGIN comparison\n")?;
        write_comparison_identity(writer, comparison)?;
        writer.u64_number(
            "execution-variants",
            map_count(&accounting.groupings.variants_by_comparison, &comparison),
        )?;
        writer.raw(b"END comparison\n")?;
    }
    for terminal in TERMINALS_V1 {
        writer.raw(b"BEGIN terminal\n")?;
        writer.line("outcome", terminal_label(terminal))?;
        writer.u64_number("attempted-variants", terminal_count(accounting, terminal))?;
        writer.raw(b"END terminal\n")?;
    }
    Ok(())
}

fn write_case(
    writer: &mut AggregateWriter,
    case: &AggregateCaseResult,
) -> Result<(), AggregateReportBuildError> {
    writer.raw(b"BEGIN logical-case\n")?;
    writer.line("test-id", case.ag.test_id.as_str())?;
    writer.prefixed_hex_line(
        "logical-case-member-digest",
        "sha256:",
        case.member_digest.as_sha256().as_bytes(),
    )?;
    writer.line("source-kind", case.source_identity.kind_label())?;
    writer.optional_line(
        "external-source-record",
        case.source_identity
            .source_record()
            .map(|value| value.as_str()),
    )?;
    writer.optional_line(
        "external-lineage",
        case.source_identity.lineage().map(|value| value.as_str()),
    )?;
    writer.optional_line(
        "external-adapter",
        case.source_identity.adapter().map(|value| value.as_str()),
    )?;
    writer.optional_line(
        "external-adapter-version",
        case.source_identity
            .adapter_version()
            .map(|value| value.as_str()),
    )?;
    writer.line("subsystem-owner", case.owner.as_str())?;
    writer.line("observation-surface", case.ag.observation.as_str())?;
    write_case_metadata(writer, case)?;
    write_eligibility(writer, &case.ag.eligibility)?;
    write_expectation(writer, &case.ag.expectation)?;

    let mut variants = Vec::new();
    variants
        .try_reserve(case.variants.len())
        .map_err(|_| AggregateReportBuildError::AllocationFailure)?;
    variants.extend(&case.variants);
    variants.sort_unstable_by(|left, right| aggregate_variant_result_cmp(left, right));
    if variants
        .windows(2)
        .any(|pair| aggregate_variant_result_cmp(pair[0], pair[1]) == Ordering::Equal)
    {
        return Err(AggregateReportBuildError::InvalidAggregateRun(
            "execution variant keys are not unique",
        ));
    }
    write_usize_number(writer, "execution-variant-count", variants.len())?;
    for variant in variants {
        write_variant(writer, variant)?;
    }
    writer.raw(b"END logical-case\n")
}

fn write_case_metadata(
    writer: &mut AggregateWriter,
    case: &AggregateCaseResult,
) -> Result<(), AggregateReportBuildError> {
    validate_ag3_case_state(&case.ag).map_err(|_| {
        AggregateReportBuildError::InvalidAggregateRun("AG3 case-state invariant failed")
    })?;
    match &case.ag.classification {
        ClassificationCompleteness::NotYetClassified { reason } => {
            writer.line("classification", "not-yet-classified")?;
            writer.line("classification-reason", reason)?;
            writer.null("requirements")?;
            writer.null("capability")?;
            writer.null("capability-missing-count")?;
            writer.null("harness")?;
            writer.null("harness-limitation-count")?;
            writer.null("environment-requirement-count")?;
            writer.null("stability")?;
            writer.null("stability-reason")?;
            writer.null("lane-exclusion-count")?;
        }
        ClassificationCompleteness::Classified => {
            writer.line("classification", "classified")?;
            writer.null("classification-reason")?;
            let mut requirements = Vec::new();
            requirements
                .try_reserve(case.ag.requirements.len())
                .map_err(|_| AggregateReportBuildError::AllocationFailure)?;
            requirements.extend(case.ag.requirements.iter().copied());
            requirements.sort_unstable_by(|left, right| {
                left.as_str().as_bytes().cmp(right.as_str().as_bytes())
            });
            writer.list(
                "requirements",
                requirements.iter().map(|requirement| requirement.as_str()),
            )?;
            write_capability(writer, case.ag.capability.as_ref())?;
            write_harness(writer, case.ag.harness.as_ref())?;
            write_environment_requirements(writer, case)?;
            write_stability(writer, case.ag.stability.as_ref())?;
            write_lane_exclusions(writer, case)?;
        }
    }
    Ok(())
}

fn write_capability(
    writer: &mut AggregateWriter,
    capability: Option<&CapabilityAvailability>,
) -> Result<(), AggregateReportBuildError> {
    let capability = capability.ok_or(AggregateReportBuildError::InvalidAggregateRun(
        "classified case lacks capability",
    ))?;
    match capability {
        CapabilityAvailability::Available => {
            writer.line("capability", "available")?;
            writer.u64_number("capability-missing-count", 0)
        }
        CapabilityAvailability::NotYetEstablished => {
            writer.line("capability", "not-yet-established")?;
            writer.u64_number("capability-missing-count", 0)
        }
        CapabilityAvailability::Unavailable { missing } => {
            writer.line("capability", "unavailable")?;
            write_usize_number(writer, "capability-missing-count", missing.len())?;
            let mut ordered = fallible_refs(missing)?;
            ordered.sort_unstable_by(|left, right| {
                left.kind
                    .as_str()
                    .as_bytes()
                    .cmp(right.kind.as_str().as_bytes())
                    .then_with(|| {
                        optional_bytes(left.feature.as_deref())
                            .cmp(&optional_bytes(right.feature.as_deref()))
                    })
                    .then_with(|| left.reason.as_bytes().cmp(right.reason.as_bytes()))
            });
            for item in ordered {
                writer.raw(b"BEGIN capability-missing\n")?;
                writer.line("kind", item.kind.as_str())?;
                writer.optional_line("feature", item.feature.as_deref())?;
                writer.line("reason", &item.reason)?;
                writer.raw(b"END capability-missing\n")?;
            }
            Ok(())
        }
    }
}

fn write_harness(
    writer: &mut AggregateWriter,
    harness: Option<&HarnessReadiness>,
) -> Result<(), AggregateReportBuildError> {
    let harness = harness.ok_or(AggregateReportBuildError::InvalidAggregateRun(
        "classified case lacks harness readiness",
    ))?;
    match harness {
        HarnessReadiness::Ready => {
            writer.line("harness", "ready")?;
            writer.u64_number("harness-limitation-count", 0)
        }
        HarnessReadiness::NotYetEstablished => {
            writer.line("harness", "not-yet-established")?;
            writer.u64_number("harness-limitation-count", 0)
        }
        HarnessReadiness::NotReady { limitations } => {
            writer.line("harness", "not-ready")?;
            write_usize_number(writer, "harness-limitation-count", limitations.len())?;
            let mut ordered = fallible_refs(limitations)?;
            ordered.sort_unstable_by(|left, right| {
                left.kind
                    .as_str()
                    .as_bytes()
                    .cmp(right.kind.as_str().as_bytes())
                    .then_with(|| left.reason.as_bytes().cmp(right.reason.as_bytes()))
            });
            for item in ordered {
                writer.raw(b"BEGIN harness-limitation\n")?;
                writer.line("kind", item.kind.as_str())?;
                writer.line("reason", &item.reason)?;
                writer.raw(b"END harness-limitation\n")?;
            }
            Ok(())
        }
    }
}

fn write_environment_requirements(
    writer: &mut AggregateWriter,
    case: &AggregateCaseResult,
) -> Result<(), AggregateReportBuildError> {
    write_usize_number(
        writer,
        "environment-requirement-count",
        case.ag.environment_requirements.len(),
    )?;
    let mut ordered = fallible_refs(&case.ag.environment_requirements)?;
    ordered.sort_unstable_by(|left, right| {
        left.kind
            .as_str()
            .as_bytes()
            .cmp(right.kind.as_str().as_bytes())
            .then_with(|| left.profile.as_bytes().cmp(right.profile.as_bytes()))
            .then_with(|| left.reason.as_bytes().cmp(right.reason.as_bytes()))
    });
    for item in ordered {
        writer.raw(b"BEGIN environment-requirement\n")?;
        writer.line("kind", item.kind.as_str())?;
        writer.line("profile", &item.profile)?;
        writer.line("reason", &item.reason)?;
        writer.raw(b"END environment-requirement\n")?;
    }
    Ok(())
}

fn write_stability(
    writer: &mut AggregateWriter,
    stability: Option<&Stability>,
) -> Result<(), AggregateReportBuildError> {
    let stability = stability.ok_or(AggregateReportBuildError::InvalidAggregateRun(
        "classified case lacks stability",
    ))?;
    match stability {
        Stability::Stable => {
            writer.line("stability", "stable")?;
            writer.null("stability-reason")
        }
        Stability::NotYetEstablished => {
            writer.line("stability", "not-yet-established")?;
            writer.null("stability-reason")
        }
        Stability::Flaky { reason } => {
            writer.line("stability", "flaky")?;
            writer.line("stability-reason", reason)
        }
    }
}

fn write_lane_exclusions(
    writer: &mut AggregateWriter,
    case: &AggregateCaseResult,
) -> Result<(), AggregateReportBuildError> {
    write_usize_number(
        writer,
        "lane-exclusion-count",
        case.ag.lane_exclusions.len(),
    )?;
    let mut ordered = fallible_refs(&case.ag.lane_exclusions)?;
    ordered.sort_unstable_by(|left, right| {
        lane_rank(left.policy)
            .cmp(&lane_rank(right.policy))
            .then_with(|| left.reason.as_bytes().cmp(right.reason.as_bytes()))
    });
    for item in ordered {
        writer.raw(b"BEGIN lane-exclusion\n")?;
        writer.line("lane", item.policy.as_str())?;
        writer.line("reason", &item.reason)?;
        writer.raw(b"END lane-exclusion\n")?;
    }
    Ok(())
}

fn write_eligibility(
    writer: &mut AggregateWriter,
    eligibility: &Eligibility,
) -> Result<(), AggregateReportBuildError> {
    match eligibility {
        Eligibility::Runnable => {
            writer.line("eligibility", "runnable")?;
            writer.u64_number("eligibility-blocker-count", 0)?;
            writer.u64_number("eligibility-unresolved-count", 0)
        }
        Eligibility::NotRunnable {
            blockers,
            unresolved,
        } => {
            writer.line("eligibility", "not-runnable")?;
            write_usize_number(writer, "eligibility-blocker-count", blockers.len())?;
            write_usize_number(writer, "eligibility-unresolved-count", unresolved.len())?;
            write_eligibility_facts(writer, "blocker", blockers)?;
            write_eligibility_facts(writer, "unresolved", unresolved)
        }
        Eligibility::NotYetEstablished { unresolved } => {
            writer.line("eligibility", "not-yet-established")?;
            writer.u64_number("eligibility-blocker-count", 0)?;
            write_usize_number(writer, "eligibility-unresolved-count", unresolved.len())?;
            write_eligibility_facts(writer, "unresolved", unresolved)
        }
    }
}

fn write_eligibility_facts(
    writer: &mut AggregateWriter,
    role: &str,
    facts: &[EligibilityFact],
) -> Result<(), AggregateReportBuildError> {
    let mut ordered = fallible_refs(facts)?;
    ordered.sort_unstable_by(|left, right| {
        eligibility_fact_key(left).cmp(&eligibility_fact_key(right))
    });
    for fact in ordered {
        writer.raw(b"BEGIN eligibility-fact\n")?;
        writer.line("role", role)?;
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
                writer.line("harness-kind", kind.as_str())?;
                writer.line("reason", reason)?;
            }
            EligibilityFact::Environment {
                kind,
                profile,
                requirement_reason,
                assessment_reason,
            } => {
                writer.line("kind", "environment")?;
                writer.line("environment-kind", kind.as_str())?;
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
            }
            EligibilityFact::HarnessReadiness => {
                writer.line("kind", "harness-readiness")?;
            }
            EligibilityFact::EnvironmentRequirement {
                kind,
                profile,
                reason,
            } => {
                writer.line("kind", "environment-requirement")?;
                writer.line("environment-kind", kind.as_str())?;
                writer.line("profile", profile)?;
                writer.line("reason", reason)?;
            }
        }
        writer.raw(b"END eligibility-fact\n")?;
    }
    Ok(())
}

fn write_expectation(
    writer: &mut AggregateWriter,
    expectation: &AgExpectation,
) -> Result<(), AggregateReportBuildError> {
    match expectation {
        AgExpectation::ExpectedPass => {
            writer.line("expectation", "expected-pass")?;
            writer.null("expected-failure-kind")?;
            writer.null("expectation-reason")
        }
        AgExpectation::ExpectedFail { failure, reason } => {
            writer.line("expectation", "expected-fail")?;
            writer.line("expected-failure-kind", failure.as_str())?;
            writer.line("expectation-reason", reason)
        }
        AgExpectation::NotEstablished => {
            writer.line("expectation", "not-established")?;
            writer.null("expected-failure-kind")?;
            writer.null("expectation-reason")
        }
    }
}

fn write_variant(
    writer: &mut AggregateWriter,
    variant: &AggregateVariantResult,
) -> Result<(), AggregateReportBuildError> {
    writer.raw(b"BEGIN execution-variant\n")?;
    match &variant.key.variant {
        AggregateExecutionVariantId::Singleton(_) => {
            writer.line("variant-kind", "singleton")?;
            writer.null("rendering-environment")?;
            writer.null("available-width-css-px")?;
        }
        AggregateExecutionVariantId::Rendering(rendering) => {
            writer.line("variant-kind", "rendering")?;
            writer.line(
                "rendering-environment",
                rendering.value().stable_environment_label(),
            )?;
            writer.u64_number(
                "available-width-css-px",
                u64::from(rendering.value().available_width_css_px.get()),
            )?;
        }
    }
    write_comparison_identity(writer, variant.comparison)?;
    match &variant.selection {
        LaneSelection::NotApplicable => {
            writer.line("lane-selection", "not-applicable")?;
            writer.null("selection-lane")?;
            writer.null("lane-selection-reason")?;
        }
        LaneSelection::Selected { lane } => {
            writer.line("lane-selection", "selected")?;
            writer.line("selection-lane", lane.as_str())?;
            writer.null("lane-selection-reason")?;
        }
        LaneSelection::Excluded { lane, reason } => {
            writer.line("lane-selection", "excluded")?;
            writer.line("selection-lane", lane.as_str())?;
            writer.line("lane-selection-reason", reason)?;
        }
    }
    match variant.execution {
        AggregateExecutionAttempt::NotAttempted { reason } => {
            writer.line("attempt", "not-attempted")?;
            writer.line("not-attempted-reason", not_attempted_label(reason))?;
            writer.null("terminal-outcome")?;
        }
        AggregateExecutionAttempt::Attempted { outcome } => {
            writer.line("attempt", "attempted")?;
            writer.null("not-attempted-reason")?;
            writer.line("terminal-outcome", terminal_label(outcome))?;
        }
    }
    writer.line("derived-policy", policy_label(variant.policy))?;
    writer.raw(b"END execution-variant\n")
}

fn test_id_cmp(left: &AggregateCaseResult, right: &AggregateCaseResult) -> Ordering {
    left.ag
        .test_id
        .as_str()
        .as_bytes()
        .cmp(right.ag.test_id.as_str().as_bytes())
}

fn eligibility_fact_key(fact: &EligibilityFact) -> (u8, &[u8], &[u8], &[u8], &[u8]) {
    match fact {
        EligibilityFact::EngineCapability {
            kind,
            feature,
            reason,
        } => (
            0,
            kind.as_str().as_bytes(),
            feature.as_deref().unwrap_or("").as_bytes(),
            reason.as_bytes(),
            b"",
        ),
        EligibilityFact::Harness { kind, reason } => {
            (1, kind.as_str().as_bytes(), reason.as_bytes(), b"", b"")
        }
        EligibilityFact::Environment {
            kind,
            profile,
            requirement_reason,
            assessment_reason,
        } => (
            2,
            kind.as_str().as_bytes(),
            profile.as_bytes(),
            requirement_reason.as_bytes(),
            assessment_reason.as_bytes(),
        ),
        EligibilityFact::Classification { reason } => (3, reason.as_bytes(), b"", b"", b""),
        EligibilityFact::EngineCapabilityAvailability => (4, b"", b"", b"", b""),
        EligibilityFact::HarnessReadiness => (5, b"", b"", b"", b""),
        EligibilityFact::EnvironmentRequirement {
            kind,
            profile,
            reason,
        } => (
            6,
            kind.as_str().as_bytes(),
            profile.as_bytes(),
            reason.as_bytes(),
            b"",
        ),
    }
}

fn fallible_refs<T>(items: &[T]) -> Result<Vec<&T>, AggregateReportBuildError> {
    let mut ordered = Vec::new();
    ordered
        .try_reserve(items.len())
        .map_err(|_| AggregateReportBuildError::AllocationFailure)?;
    ordered.extend(items);
    Ok(ordered)
}

fn optional_bytes(value: Option<&str>) -> (u8, &[u8]) {
    match value {
        None => (0, b""),
        Some(value) => (1, value.as_bytes()),
    }
}

fn map_count<K: Ord>(map: &std::collections::BTreeMap<K, u64>, key: &K) -> u64 {
    map.get(key).copied().unwrap_or(0)
}

const fn lane_rank(lane: LanePolicyScope) -> u8 {
    match lane {
        LanePolicyScope::NormalCi => 0,
        LanePolicyScope::LocalExtended => 1,
        LanePolicyScope::ScheduledExtended => 2,
        LanePolicyScope::ManualExtended => 3,
    }
}

fn write_comparison_identity(
    writer: &mut AggregateWriter,
    value: AggregateComparisonKind,
) -> Result<(), AggregateReportBuildError> {
    match value {
        AggregateComparisonKind::AuthoredExpectedObservation => {
            writer.line("comparison-kind", "authored-expected-observation")?;
            writer.null("reference-kind")?;
            writer.null("reference-relation")
        }
        AggregateComparisonKind::StaticDocumentReference {
            reference_kind,
            relation,
        } => {
            writer.line("comparison-kind", "static-document-reference")?;
            writer.line("reference-kind", reference_kind.as_str())?;
            writer.line("reference-relation", relation.as_str())
        }
    }
}

fn write_usize_number(
    writer: &mut AggregateWriter,
    key: &str,
    value: usize,
) -> Result<(), AggregateReportBuildError> {
    let value = u64::try_from(value).map_err(|_| AggregateReportBuildError::CountConversion)?;
    writer.u64_number(key, value)
}

const fn terminal_label(value: AggregateTerminalOutcome) -> &'static str {
    match value {
        AggregateTerminalOutcome::SemanticPass => "semantic-pass",
        AggregateTerminalOutcome::SemanticFail => "semantic-fail",
        AggregateTerminalOutcome::ExecutionFailure => "execution-failure",
        AggregateTerminalOutcome::ResourceFailure => "resource-failure",
        AggregateTerminalOutcome::IncompleteObservation => "incomplete-observation",
        AggregateTerminalOutcome::InvariantFailure => "invariant-failure",
        AggregateTerminalOutcome::Timeout => "timeout",
    }
}

const fn not_attempted_label(value: AggregateNotAttemptedReason) -> &'static str {
    match value {
        AggregateNotAttemptedReason::Eligibility => "eligibility",
        AggregateNotAttemptedReason::LaneExcluded => "lane-excluded",
        AggregateNotAttemptedReason::ParserPreAttemptEvaluation => "parser-pre-attempt-evaluation",
        AggregateNotAttemptedReason::CssFragmentCapabilityUnavailable => {
            "css-fragment-capability-unavailable"
        }
    }
}

const fn policy_label(value: DerivedPolicyResult) -> &'static str {
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

fn terminal_count(accounting: &AggregateAccounting, terminal: AggregateTerminalOutcome) -> u64 {
    match terminal {
        AggregateTerminalOutcome::SemanticPass => accounting.terminals.semantic_pass,
        AggregateTerminalOutcome::SemanticFail => accounting.terminals.semantic_fail,
        AggregateTerminalOutcome::ExecutionFailure => accounting.terminals.execution_failure,
        AggregateTerminalOutcome::ResourceFailure => accounting.terminals.resource_failure,
        AggregateTerminalOutcome::IncompleteObservation => {
            accounting.terminals.incomplete_observation
        }
        AggregateTerminalOutcome::InvariantFailure => accounting.terminals.invariant_failure,
        AggregateTerminalOutcome::Timeout => accounting.terminals.timeout,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use conformance_test_support::{
        EngineCapabilityKind, EnvironmentRequirementKind, HarnessLimitationKind, RequirementTag,
    };

    use super::*;
    use crate::aggregate::model::AggregateRunSealError;
    use crate::{
        AggregateGroupingAccounting, AggregateRunInvariantError, AggregateVariantPopulationCounts,
        EligibilityFact, LogicalHeadlineCounts, ReasonedCapability, ReasonedEnvironmentRequirement,
        ReasonedHarnessLimitation, ReasonedLaneExclusion, TerminalOutcomeCounts,
        run_repository_aggregate,
    };

    fn maximum_accounting() -> AggregateAccounting {
        AggregateAccounting {
            logical: LogicalHeadlineCounts {
                total_tests: u64::MAX,
                pass_count: u64::MAX,
                fail_count: u64::MAX,
                expected_fail_count: u64::MAX,
                unsupported_count: u64::MAX,
                skipped_count: u64::MAX,
                flaky_count: u64::MAX,
                unclassified_count: u64::MAX,
            },
            variants: AggregateVariantPopulationCounts {
                materialized_variants: u64::MAX,
                runnable_variants: u64::MAX,
                not_runnable_variants: u64::MAX,
                eligibility_not_established_variants: u64::MAX,
                selected_variants: u64::MAX,
                excluded_variants: u64::MAX,
                selection_not_applicable_variants: u64::MAX,
                attempted_variants: u64::MAX,
                not_attempted_variants: u64::MAX,
            },
            terminals: TerminalOutcomeCounts {
                semantic_pass: u64::MAX,
                semantic_fail: u64::MAX,
                execution_failure: u64::MAX,
                resource_failure: u64::MAX,
                incomplete_observation: u64::MAX,
                invariant_failure: u64::MAX,
                timeout: u64::MAX,
            },
            groupings: AggregateGroupingAccounting {
                logical_cases_by_subsystem: OWNERS_V1
                    .into_iter()
                    .map(|key| (key, u64::MAX))
                    .collect::<BTreeMap<_, _>>(),
                variants_by_subsystem: OWNERS_V1
                    .into_iter()
                    .map(|key| (key, u64::MAX))
                    .collect::<BTreeMap<_, _>>(),
                logical_cases_by_observation: OBSERVATIONS_V1
                    .into_iter()
                    .map(|key| (key, u64::MAX))
                    .collect::<BTreeMap<_, _>>(),
                variants_by_observation: OBSERVATIONS_V1
                    .into_iter()
                    .map(|key| (key, u64::MAX))
                    .collect::<BTreeMap<_, _>>(),
                variants_by_comparison: COMPARISONS_V1
                    .into_iter()
                    .map(|key| (key, u64::MAX))
                    .collect::<BTreeMap<_, _>>(),
            },
        }
    }

    fn syntactic_summary_envelope(maximum: usize) -> Result<Vec<u8>, AggregateReportBuildError> {
        let mut writer = AggregateWriter::new(maximum)?;
        writer.line("format", AGGREGATE_SUMMARY_FORMAT_V1)?;
        writer.line("inventory-scope", "static-html-css-no-js")?;
        writer.line(
            "aggregate-granularity-contract",
            AGGREGATE_GRANULARITY_CONTRACT_V1,
        )?;
        writer.line("named-lane", "scheduled-extended")?;
        writer.line(
            "environment-assessment",
            crate::AggregateEnvironmentAssessmentMode::EmptyV1.as_str(),
        )?;
        writer.line(
            "population-identity-contract",
            AGGREGATE_POPULATION_IDENTITY_CONTRACT_V1,
        )?;
        writer.prefixed_hex_line("logical-case-source-set-digest", "sha256:", &[0xff; 32])?;
        writer.boolean("headline-counts-overlap", true)?;
        write_accounting(&mut writer, &maximum_accounting())?;
        Ok(writer.finish())
    }

    fn repository_run() -> AggregateRun {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        run_repository_aggregate(
            root,
            super::super::AggregateExecutionRequest {
                lane: LanePolicyScope::NormalCi,
            },
        )
        .unwrap()
    }

    #[test]
    fn summary_ceiling_accepts_the_exact_syntactic_envelope_and_rejects_plus_one() {
        let bytes = syntactic_summary_envelope(AGGREGATE_SUMMARY_MAX_BYTES_V1).unwrap();
        assert_eq!(bytes.len(), AGGREGATE_SUMMARY_MAX_BYTES_V1);

        let mut plus_one = AggregateWriter::new(AGGREGATE_SUMMARY_MAX_BYTES_V1).unwrap();
        plus_one.raw(&bytes).unwrap();
        assert!(matches!(
            plus_one.raw(b"x"),
            Err(AggregateReportBuildError::ReportTooLarge {
                maximum: AGGREGATE_SUMMARY_MAX_BYTES_V1,
                ..
            })
        ));
        assert_eq!(plus_one.finish(), bytes);

        // A 6,073-byte V1 grammar instance is also the maximum-plus-one
        // instance for a deliberately one-byte-smaller bound.
        assert!(matches!(
            syntactic_summary_envelope(AGGREGATE_SUMMARY_MAX_BYTES_V1 - 1),
            Err(AggregateReportBuildError::ReportTooLarge { maximum: 6_072, .. })
        ));
    }

    #[test]
    fn bounded_writer_accepts_exact_size_and_never_truncates() {
        let mut exact = AggregateWriter::new(4).unwrap();
        exact.raw(b"1234").unwrap();
        assert_eq!(exact.finish(), b"1234");
        let mut excess = AggregateWriter::new(4).unwrap();
        assert!(matches!(
            excess.raw(b"12345"),
            Err(AggregateReportBuildError::ReportTooLarge { maximum: 4, .. })
        ));
        assert!(excess.finish().is_empty());
    }

    #[test]
    fn detail_bound_accepts_exact_size_and_rejects_maximum_plus_one() {
        let exact_bytes = vec![b'x'; AGGREGATE_DETAIL_MAX_BYTES_V1];
        let mut exact = AggregateWriter::new(AGGREGATE_DETAIL_MAX_BYTES_V1).unwrap();
        exact.raw(&exact_bytes).unwrap();
        assert_eq!(exact.finish().len(), AGGREGATE_DETAIL_MAX_BYTES_V1);

        let excess_bytes = vec![b'x'; AGGREGATE_DETAIL_MAX_BYTES_V1 + 1];
        let mut excess = AggregateWriter::new(AGGREGATE_DETAIL_MAX_BYTES_V1).unwrap();
        assert!(matches!(
            excess.raw(&excess_bytes),
            Err(AggregateReportBuildError::ReportTooLarge {
                maximum: AGGREGATE_DETAIL_MAX_BYTES_V1,
                ..
            })
        ));
        assert!(excess.finish().is_empty());
    }

    #[test]
    fn both_reports_are_repeatable_and_input_order_independent() {
        let run = repository_run();
        let summary = build_aggregate_summary_v1(&run).unwrap();
        let detail = build_aggregate_detail_v1(&run).unwrap();
        assert_eq!(build_aggregate_summary_v1(&run).unwrap(), summary);
        assert_eq!(build_aggregate_detail_v1(&run).unwrap(), detail);

        let mut reversed_cases = run.cases().to_vec();
        reversed_cases.reverse();
        for case in &mut reversed_cases {
            case.variants.reverse();
        }
        let reordered = AggregateRun::try_seal(
            run.inventory_scope(),
            run.request(),
            run.environment_assessment_mode(),
            reversed_cases,
        )
        .unwrap();
        assert_eq!(build_aggregate_summary_v1(&reordered).unwrap(), summary);
        assert_eq!(build_aggregate_detail_v1(&reordered).unwrap(), detail);
    }

    #[test]
    fn detail_order_is_independent_for_every_nested_unordered_collection() {
        let run = repository_run();
        let forward = adversarial_order_case(&run, false);
        let reverse = adversarial_order_case(&run, true);
        let mut forward_writer = AggregateWriter::new(AGGREGATE_DETAIL_MAX_BYTES_V1).unwrap();
        let mut reverse_writer = AggregateWriter::new(AGGREGATE_DETAIL_MAX_BYTES_V1).unwrap();
        write_case(&mut forward_writer, &forward).unwrap();
        write_case(&mut reverse_writer, &reverse).unwrap();
        assert_eq!(forward_writer.finish(), reverse_writer.finish());
    }

    fn adversarial_order_case(run: &AggregateRun, reverse: bool) -> AggregateCaseResult {
        let mut case = run
            .cases()
            .iter()
            .find(|case| case.ag.test_id.as_str() == "layout-geometry-basic-block-flow")
            .unwrap()
            .clone();

        case.ag.requirements = ordered(
            vec![
                RequirementTag::RequiresLayoutFeature,
                RequirementTag::NoJs,
                RequirementTag::RequiresCssFeature,
            ],
            reverse,
        );
        case.ag.capability = Some(CapabilityAvailability::Unavailable {
            missing: ordered(
                vec![
                    ReasonedCapability {
                        kind: EngineCapabilityKind::LayoutFeature,
                        feature: Some("css-grid".to_owned()),
                        reason: "layout capability z".to_owned(),
                    },
                    ReasonedCapability {
                        kind: EngineCapabilityKind::CssFeature,
                        feature: Some("css-cascade".to_owned()),
                        reason: "CSS capability a".to_owned(),
                    },
                ],
                reverse,
            ),
        });
        case.ag.harness = Some(HarnessReadiness::NotReady {
            limitations: ordered(
                vec![
                    ReasonedHarnessLimitation {
                        kind: HarnessLimitationKind::MissingComparisonSurface,
                        reason: "comparison z".to_owned(),
                    },
                    ReasonedHarnessLimitation {
                        kind: HarnessLimitationKind::MissingExpectedObservation,
                        reason: "expectation a".to_owned(),
                    },
                ],
                reverse,
            ),
        });
        case.ag.environment_requirements = ordered(
            vec![
                ReasonedEnvironmentRequirement {
                    kind: EnvironmentRequirementKind::ViewportConfiguration,
                    profile: "viewport-z".to_owned(),
                    reason: "viewport reason".to_owned(),
                },
                ReasonedEnvironmentRequirement {
                    kind: EnvironmentRequirementKind::ControlledFontSet,
                    profile: "font-a".to_owned(),
                    reason: "font reason".to_owned(),
                },
            ],
            reverse,
        );
        case.ag.lane_exclusions = ordered(
            vec![
                ReasonedLaneExclusion {
                    policy: LanePolicyScope::ScheduledExtended,
                    reason: "scheduled z".to_owned(),
                },
                ReasonedLaneExclusion {
                    policy: LanePolicyScope::NormalCi,
                    reason: "normal a".to_owned(),
                },
            ],
            reverse,
        );
        case.ag.eligibility = Eligibility::NotRunnable {
            blockers: ordered(
                vec![
                    EligibilityFact::Harness {
                        kind: HarnessLimitationKind::MissingComparisonSurface,
                        reason: "blocker z".to_owned(),
                    },
                    EligibilityFact::EngineCapability {
                        kind: EngineCapabilityKind::CssFeature,
                        feature: Some("css-cascade".to_owned()),
                        reason: "blocker a".to_owned(),
                    },
                ],
                reverse,
            ),
            unresolved: ordered(
                vec![
                    EligibilityFact::EnvironmentRequirement {
                        kind: EnvironmentRequirementKind::ViewportConfiguration,
                        profile: "viewport-z".to_owned(),
                        reason: "unresolved z".to_owned(),
                    },
                    EligibilityFact::Classification {
                        reason: "unresolved a".to_owned(),
                    },
                ],
                reverse,
            ),
        };
        if reverse {
            case.variants.reverse();
        }
        case
    }

    fn ordered<T>(mut values: Vec<T>, reverse: bool) -> Vec<T> {
        if reverse {
            values.reverse();
        }
        values
    }

    #[test]
    fn aggregate_run_seals_all_ag3_classification_expectation_invariants() {
        let run = repository_run();
        let browser_index = run
            .cases()
            .iter()
            .position(|case| case.ag.test_id.as_str() == "browser-controlled-static-page-basic")
            .unwrap();
        let classified_index = run
            .cases()
            .iter()
            .position(|case| case.ag.test_id.as_str() == "css-parsing-basic-stylesheet")
            .unwrap();

        let mut cases = run.cases().to_vec();
        cases[browser_index].ag.expectation = AgExpectation::ExpectedPass;
        assert!(matches!(
            validate_run_with_cases(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::NotYetClassifiedHasEstablishedExpectation { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[browser_index].ag.capability = Some(CapabilityAvailability::Available);
        assert!(matches!(
            validate_run_with_cases(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::NotYetClassifiedHasClassifiedDimensions { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[classified_index].ag.expectation = AgExpectation::NotEstablished;
        assert!(matches!(
            validate_run_with_cases(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::ClassifiedLacksEstablishedExpectation { .. }
            ))
        ));

        let mut cases = run.cases().to_vec();
        cases[classified_index].ag.harness = None;
        assert!(matches!(
            validate_run_with_cases(&run, cases),
            Err(AggregateRunSealError::Invariant(
                AggregateRunInvariantError::ClassifiedLacksRequiredDimensions { .. }
            ))
        ));
    }

    #[test]
    fn detail_projection_rechecks_the_ag3_branch_invariant_defensively() {
        let run = repository_run();
        let mut case = run
            .cases()
            .iter()
            .find(|case| case.ag.test_id.as_str() == "browser-controlled-static-page-basic")
            .unwrap()
            .clone();
        case.ag.expectation = AgExpectation::ExpectedPass;
        let mut writer = AggregateWriter::new(AGGREGATE_DETAIL_MAX_BYTES_V1).unwrap();
        assert!(matches!(
            write_case_metadata(&mut writer, &case),
            Err(AggregateReportBuildError::InvalidAggregateRun(
                "AG3 case-state invariant failed"
            ))
        ));
    }

    fn validate_run_with_cases(
        run: &AggregateRun,
        cases: Vec<AggregateCaseResult>,
    ) -> Result<AggregateRun, AggregateRunSealError> {
        AggregateRun::try_seal(
            run.inventory_scope(),
            run.request(),
            run.environment_assessment_mode(),
            cases,
        )
    }

    #[test]
    fn run_domain_comparison_structure_and_whitespace_are_projected_exactly() {
        let run = repository_run();
        assert_eq!(
            run.environment_assessment_mode(),
            crate::AggregateEnvironmentAssessmentMode::EmptyV1
        );
        let summary = String::from_utf8(build_aggregate_summary_v1(&run).unwrap()).unwrap();
        let detail = String::from_utf8(build_aggregate_detail_v1(&run).unwrap()).unwrap();
        for report in [&summary, &detail] {
            assert!(report.contains("environment-assessment = \"ag9-empty-assessment-v1\"\n"));
            assert!(!report.contains("\n\n"));
            assert!(report.ends_with('\n'));
            assert!(!report.ends_with("\n\n"));
            assert!(!report.contains("static-document-reference-semantic-"));
            assert!(!report.contains("static-document-reference-structural-"));
        }
        assert!(summary.contains(
            "comparison-kind = \"authored-expected-observation\"\nreference-kind = null\nreference-relation = null\n"
        ));
        assert!(summary.contains(
            "comparison-kind = \"static-document-reference\"\nreference-kind = \"semantic\"\nreference-relation = \"match\"\n"
        ));
        assert_eq!(summary.matches("BEGIN comparison\n").count(), 5);
    }

    struct PrefixFailingWriter {
        accepted: usize,
        fail_after: usize,
    }

    impl Write for PrefixFailingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let allowed = self
                .fail_after
                .saturating_sub(self.accepted)
                .min(bytes.len());
            if allowed == 0 {
                return Err(std::io::Error::other("synthetic output failure"));
            }
            self.accepted += allowed;
            Ok(allowed)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn construction_failure_publishes_nothing_and_sink_failure_may_retain_a_prefix() {
        let run = repository_run();
        let mut untouched = PrefixFailingWriter {
            accepted: 0,
            fail_after: usize::MAX,
        };
        let failure = crate::report_writer::with_forced_allocation_failure(|| {
            build_and_write_aggregate_summary_v1(&run, &mut untouched)
        });
        assert!(matches!(
            failure,
            Err(AggregateReportPublicationError::Build(
                AggregateReportBuildError::AllocationFailure
            ))
        ));
        assert_eq!(untouched.accepted, 0);

        let mut prefix = PrefixFailingWriter {
            accepted: 0,
            fail_after: 7,
        };
        assert!(matches!(
            build_and_write_aggregate_summary_v1(&run, &mut prefix),
            Err(AggregateReportPublicationError::OutputWrite(_))
        ));
        assert_eq!(prefix.accepted, 7);
    }

    #[test]
    fn detail_preserves_absent_classification_and_population_granularity() {
        let detail =
            String::from_utf8(build_aggregate_detail_v1(&repository_run()).unwrap()).unwrap();
        let browser_start = detail
            .find("test-id = \"browser-controlled-static-page-basic\"")
            .unwrap();
        let browser_end =
            browser_start + detail[browser_start..].find("END logical-case\n").unwrap();
        let browser = &detail[browser_start..browser_end];
        for absent in [
            "requirements = null",
            "capability = null",
            "capability-missing-count = null",
            "harness = null",
            "harness-limitation-count = null",
            "environment-requirement-count = null",
            "stability = null",
            "lane-exclusion-count = null",
        ] {
            assert!(browser.contains(absent), "missing {absent}");
        }
        assert!(browser.contains("execution-variant-count = 0"));
        assert!(!browser.contains("not-classified"));

        let multi_start = detail
            .find("test-id = \"layout-geometry-basic-block-flow\"")
            .unwrap();
        let multi_end = multi_start + detail[multi_start..].find("END logical-case\n").unwrap();
        let multi = &detail[multi_start..multi_end];
        assert!(multi.contains("execution-variant-count = 2"));
        assert_eq!(multi.matches("BEGIN execution-variant\n").count(), 2);
    }
}
