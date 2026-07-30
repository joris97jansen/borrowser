//! Transitional bridge for legacy pipeline integration.

mod adapters;

pub(crate) use adapters::PatchEmitterAdapter;
#[cfg(any(test, feature = "parser-conformance"))]
pub(crate) use adapters::{
    PatchHistoryCaptureFailure, PatchHistoryObservationConfig, RawPatchHistoryCapture,
};
