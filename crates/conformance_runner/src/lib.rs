//! Subsystem-neutral execution orchestration for Borrowser conformance cases.
//!
//! This crate consumes generic AG inventory and classification metadata and
//! delegates subsystem semantics to subsystem-owned test-support adapters.
//! It is not production browser code.

mod model;
mod report;

#[cfg(feature = "html-parser")]
mod html_parser;

pub use model::{
    AgExpectation, CapabilityAvailability, ClassificationCompleteness, DerivedPolicyResult,
    Eligibility, EligibilityFact, ExecutionAttempt, HarnessReadiness,
    NormalizedAeDispositionContext, NormalizedCaseResult, NormalizedExecutionFailureCategory,
    NormalizedIncompleteObservationReason, NotAttemptedReason, ObservationArtifact,
    ObservedExecutionOutcome, ParserObservationProfile, ParserObservationSurface,
    PreAttemptEvaluationOutcome, ReasonedCapability, ReasonedEnvironmentRequirement,
    ReasonedHarnessLimitation, ReasonedLaneExclusion, Stability,
};
pub use report::{
    DEFAULT_REPORT_LIMITS, REPORT_FORMAT_V1, ReportBuildError, ReportLimits,
    ReportPublicationError, build_and_write_report, build_report,
};

#[cfg(feature = "html-parser")]
pub use html_parser::{ParserRunError, ParserRunSummary, run_repository_parser_cases};
