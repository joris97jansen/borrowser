//! CSS-domain conformance fixture adaptation.
//!
//! This test/tooling crate owns strict fixture packages, neutral target
//! addressing, phase-correct production adaptation, bounded observations, and
//! semantic comparison. CSS semantics remain exclusively in `css`.

mod execute;
mod fixture;
mod target;

pub use execute::{
    CssExecutionFailure, CssExecutionFailureClass, CssExecutionPhase, CssExecutionResourceLimit,
    CssExecutionStorage, CssFixtureEvaluation, CssObservedExecutionOutcome,
    CssRequiredObservationFailure, classify_computed_style_failure, classify_execution_failure,
    classify_rule_collection_failure, classify_style_resolution_failure, evaluate_fixture,
};
pub use fixture::{
    CSS_FIXTURE_FORMAT_V1, CSS_NESTED_MAX_HTML_INPUT_BYTES, CSS_NESTED_MAX_TARGETS,
    CssExecutionProfile, CssFixtureLimitConfigurationError, CssFixtureLimits, CssFixtureLoadError,
    CssFixturePackage, CssFixtureProblem, CssFragmentContext, CssHostNamespace, CssHtmlInputKind,
    CssStylesheetOrigin, load_fixture_package,
};
pub use target::{
    CSS_NESTED_MAX_TARGET_LABEL_BYTES, CssTargetAddress, CssTargetAddressStep, CssTargetChildKind,
    CssTargetLabel, CssTargetResolutionFailure,
};

/// Current authoritative selector-list representation limit used to derive
/// boundary fixtures without mirroring a CSS-owned constant.
pub fn selector_list_count_limit() -> usize {
    css::SyntaxLimits::default().max_selectors_per_rule
}
