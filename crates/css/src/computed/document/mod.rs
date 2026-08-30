//! Document-level computed style materialization.
//!
//! This module owns the handoff from structured cascade output to total
//! computed styles for selector-addressable DOM elements. It keeps full-pass
//! computation, incremental suffix recomputation, style reuse, diagnostics, and
//! debug output behind one stable public API.

mod compute;
mod debug;
mod error;
mod incremental;
mod materialize;
mod model;
mod reuse;

pub(crate) use compute::compute_document_styles_from_resolved_styles_with_index;
pub use compute::{
    compute_document_styles, compute_document_styles_from_resolved_styles,
    compute_document_styles_from_resolved_styles_with_reuse_stats,
    compute_document_styles_with_limits,
};
pub use error::ComputedStyleResolutionError;
pub use incremental::{
    IncrementalComputedDocumentStyle, StylePlanExecution,
    compute_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
    compute_document_styles_incremental_suffix_from_execution_with_limits,
    compute_document_styles_incremental_suffix_from_rule_collection_with_limits,
    compute_document_styles_incremental_suffix_with_limits,
    try_compute_document_styles_for_invalidation_plan_from_execution_with_limits,
    try_compute_document_styles_for_invalidation_plan_from_rule_collection_with_limits,
    try_compute_document_styles_for_invalidation_plan_with_limits,
};
pub use materialize::compute_style_from_resolved_style;
pub use model::{
    ComputedDocumentStyle, ComputedDocumentStyleWithStats, ComputedElementStyle,
    ComputedStyleReuseStats,
};
