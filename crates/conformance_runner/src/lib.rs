//! Subsystem-neutral execution orchestration for Borrowser conformance cases.
//!
//! This crate consumes generic AG inventory and classification metadata and
//! delegates subsystem semantics to subsystem-owned test-support adapters.
//! It is not production browser code.

#[cfg(any(feature = "html-parser", feature = "css", feature = "rendering"))]
mod metadata;
mod model;
mod report;
mod report_writer;

#[cfg(feature = "aggregate")]
mod aggregate;

#[cfg(feature = "css")]
mod css_package;
#[cfg(feature = "css")]
mod css_report;
#[cfg(feature = "css")]
mod css_runner;
#[cfg(feature = "html-parser")]
mod html_parser;
#[cfg(feature = "rendering")]
mod rendering_package;
#[cfg(feature = "rendering")]
mod rendering_report;
#[cfg(feature = "rendering")]
mod rendering_runner;

pub use model::{
    AgCaseState, AgExpectation, CapabilityAvailability, ClassificationCompleteness,
    DerivedPolicyResult, Eligibility, EligibilityFact, ExecutionAttempt, ExecutionVariantId,
    HarnessReadiness, NormalizedAeDispositionContext, NormalizedCaseResult,
    NormalizedExecutionFailureCategory, NormalizedIncompleteObservationReason, NotAttemptedReason,
    ObservationArtifact, ObservedExecutionOutcome, ObservedPolicyClass, ParserObservationProfile,
    ParserObservationSurface, PreAttemptEvaluationOutcome, ReasonedCapability,
    ReasonedEnvironmentRequirement, ReasonedHarnessLimitation, ReasonedLaneExclusion,
    SingletonExecutionVariant, Stability, SubsystemExecutionAttempt,
};
pub use report::{
    DEFAULT_REPORT_LIMITS, REPORT_FORMAT_V1, ReportBuildError, ReportLimits,
    ReportPublicationError, build_and_write_report, build_report,
};

#[cfg(feature = "aggregate")]
pub use aggregate::{
    AGGREGATE_DETAIL_FORMAT_V1, AGGREGATE_DETAIL_MAX_BYTES_V1, AGGREGATE_GRANULARITY_CONTRACT_V1,
    AGGREGATE_LOGICAL_CASE_MEMBER_IDENTITY_V1, AGGREGATE_LOGICAL_CASE_SOURCE_SET_IDENTITY_V1,
    AGGREGATE_POPULATION_IDENTITY_CONTRACT_V1, AGGREGATE_SUMMARY_FORMAT_V1,
    AGGREGATE_SUMMARY_MAX_BYTES_V1, AggregateAccounting, AggregateCaseResult,
    AggregateComparisonKind, AggregateEnvironmentAssessmentMode, AggregateExecutionAttempt,
    AggregateExecutionRequest, AggregateExecutionVariantId, AggregateGroupingAccounting,
    AggregateIdentityError, AggregateLogicalCaseMemberDigest, AggregateLogicalCaseSourceSetDigest,
    AggregateLogicalSourceIdentity, AggregateNotAttemptedReason, AggregateReconciliationError,
    AggregateRenderingCaseEvidence, AggregateReportBuildError, AggregateReportPublicationError,
    AggregateRun, AggregateRunError, AggregateRunInvariantError, AggregateSubsystemResult,
    AggregateTerminalOutcome, AggregateVariantKey, AggregateVariantPopulationCounts,
    AggregateVariantResult, LaneSelection, LogicalHeadlineCounts, TerminalOutcomeCounts,
    build_aggregate_detail_v1, build_aggregate_summary_v1, build_and_write_aggregate_detail_v1,
    build_and_write_aggregate_summary_v1, run_repository_aggregate,
};

#[cfg(feature = "html-parser")]
pub use html_parser::{ParserRunError, ParserRunSummary, run_repository_parser_cases};

#[cfg(feature = "css")]
pub use css_package::{CssPackageReconciliationError, reconcile_css_package};
#[cfg(feature = "css")]
pub use css_report::{CSS_REPORT_FORMAT_V1, build_and_write_css_report, build_css_report};
#[cfg(feature = "css")]
pub use css_runner::{
    CssCaseResult, CssExecutionAttempt, CssNotAttemptedReason, CssRunError, CssRunSummary,
    run_repository_css_cases,
};

#[cfg(feature = "rendering")]
pub use rendering_package::{
    ReconciledPairedRenderingPackage, RenderingPackageReconciliationError,
    reconcile_paired_rendering_package, reconcile_rendering_package,
};
#[cfg(feature = "rendering")]
pub use rendering_report::{
    REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1, RENDERING_REPORT_FORMAT_V1,
    RENDERING_REPORT_FORMAT_V2, build_and_write_rendering_report, build_rendering_report,
    build_rendering_report_v1,
};
#[cfg(feature = "rendering")]
pub use rendering_runner::{
    RenderingCaptureSummary, RenderingCaseResult, RenderingExecutionAttempt,
    RenderingNotAttemptedReason, RenderingObservationSummary, RenderingOracleKind,
    RenderingReferenceObservedOutcome, RenderingRelationResult, RenderingRunError,
    RenderingRunSummary, RenderingVariantObservedOutcome, RenderingVariantResult,
    run_repository_rendering_cases,
};
