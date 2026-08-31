//! Controlled Layout/Paint conformance host for AG6.
//!
//! Rendering semantics remain in HTML, CSS, Layout, and Paint. This crate owns
//! strict fixture transport, deterministic host inputs, bounded observation
//! capture, and structural snapshot comparison.

mod environment;
mod execute;
mod fixture;
mod observation;

pub use environment::{AvailableWidthCssPx, RenderingExecutionVariantId, SyntheticTextMetricsV1};
pub use execute::{
    RenderingExecutionFailure, RenderingExecutionPhase, RenderingExecutionStorage,
    RenderingFinalInvariantFailure, RenderingIncompleteObservationReason,
    RenderingMismatchEvidence, RenderingObservedExecutionOutcome, RenderingProfileObservation,
    RenderingSnapshotDifference, evaluate_variant,
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
