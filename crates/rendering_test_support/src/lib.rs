//! Controlled Layout/Paint conformance host for AG6.
//!
//! Rendering semantics remain in HTML, CSS, Layout, and Paint. This crate owns
//! strict fixture transport, deterministic host inputs, bounded observation
//! capture, and structural snapshot comparison.

mod comparison;
mod environment;
mod execute;
mod fixture;
mod observation;
mod paired_fixture;

pub use comparison::{
    REFERENCE_DIFFERENCE_EXCERPT_UTF8_BYTES_V1, RenderingComparisonFailure,
    RenderingDifferenceEvidenceFailure, RenderingDifferenceLine, RenderingDifferenceLocator,
    RenderingFirstDifference, RenderingOracleComparison, RenderingOracleVerdict,
    compare_canonical_rendering_captures, materialize_rendering_first_difference,
};
pub use environment::{AvailableWidthCssPx, RenderingExecutionVariantId, SyntheticTextMetricsV1};
pub use execute::{
    CanonicalRenderingCapture, PairedRenderingCaptureOutcome, RenderingCaptureOutcome,
    RenderingExecutionFailure, RenderingExecutionPhase, RenderingExecutionStorage,
    RenderingFinalInvariantFailure, RenderingIncompleteObservationReason,
    RenderingMismatchEvidence, RenderingObservedExecutionOutcome, RenderingProfileObservation,
    RenderingSnapshotDifference, capture_paired_variant, evaluate_variant,
};
pub use fixture::{
    RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1,
    RENDERING_CUMULATIVE_STYLESHEET_INPUT_BYTES_V1, RENDERING_EXPECTATION_PAIR_COUNT_V1,
    RENDERING_FIXTURE_FORMAT_V1, RENDERING_HTML_INPUT_BYTES_V1,
    RENDERING_SELECTED_PROFILE_COUNT_V1, RENDERING_STYLESHEET_COUNT_V1, RENDERING_VARIANT_COUNT_V1,
    RenderingFixtureLimitConfigurationError, RenderingFixtureLimits, RenderingFixtureLoadError,
    RenderingFixturePackage, RenderingFixtureProblem, RenderingStylesheetOrigin,
    RenderingVariantExecution, RenderingVariantHandle, load_fixture_package,
    load_variant_execution,
};
pub use observation::{
    BoundedObservationSink, LayoutObservationProfile, ObservationSinkFailure,
    PaintObservationProfile, RenderingObservationOwner, RenderingObservationProfile,
};
pub use paired_fixture::{
    PAIRED_RENDERING_COMBINED_HTML_BYTES_V1, PAIRED_RENDERING_COMBINED_STYLESHEET_BYTES_V1,
    PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1,
    PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1, PAIRED_RENDERING_FIXTURE_FORMAT_V1,
    PairedRenderingFixtureLimitConfigurationError, PairedRenderingFixtureLimits,
    PairedRenderingFixtureLoadError, PairedRenderingFixturePackage, PairedRenderingFixtureProblem,
    PairedRenderingVariantHandle, load_paired_fixture_package,
};
