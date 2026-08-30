//! Subsystem-neutral execution orchestration for Borrowser conformance cases.
//!
//! This crate consumes generic AG inventory and classification metadata and
//! delegates subsystem semantics to subsystem-owned test-support adapters.
//! It is not production browser code.

#[cfg(any(feature = "html-parser", feature = "css"))]
mod metadata;
mod model;
mod report;

#[cfg(feature = "css")]
mod css_package;
#[cfg(feature = "css")]
mod css_report;
#[cfg(feature = "css")]
mod css_runner;
#[cfg(feature = "html-parser")]
mod html_parser;

pub use model::{
    AgCaseState, AgExpectation, CapabilityAvailability, ClassificationCompleteness,
    DerivedPolicyResult, Eligibility, EligibilityFact, ExecutionAttempt, HarnessReadiness,
    NormalizedAeDispositionContext, NormalizedCaseResult, NormalizedExecutionFailureCategory,
    NormalizedIncompleteObservationReason, NotAttemptedReason, ObservationArtifact,
    ObservedExecutionOutcome, ObservedPolicyClass, ParserObservationProfile,
    ParserObservationSurface, PreAttemptEvaluationOutcome, ReasonedCapability,
    ReasonedEnvironmentRequirement, ReasonedHarnessLimitation, ReasonedLaneExclusion, Stability,
    SubsystemExecutionAttempt,
};
pub use report::{
    DEFAULT_REPORT_LIMITS, REPORT_FORMAT_V1, ReportBuildError, ReportLimits,
    ReportPublicationError, build_and_write_report, build_report,
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
