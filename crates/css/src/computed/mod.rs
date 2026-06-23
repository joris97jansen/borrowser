//! Typed computed-style contract plus the current legacy bridge implementation.
//!
//! `ResolvedStyle` from Milestone R is the normative cascade handoff into this
//! layer. The long-term property pipeline is:
//! - `css::model::DeclarationValue` holds authored parsed syntax
//! - property parsing converts authored syntax into `SpecifiedPropertyValue`
//!   values selected by `PropertySpecifiedValueKind`; `CascadeSpecifiedValue`
//!   carries those values for supported winners
//! - computed-style assembly resolves those specified values, inheritance, and
//!   initial/default values into typed, normalized `ComputedStyle`
//!
//! `compute_document_styles(...)` and
//! `compute_style_from_resolved_style(...)` are the production typed assembly
//! paths. During the current bridge phase, `compute_style(...)` still consumes
//! the legacy DOM-attached `(String, String)` declaration vector for
//! compatibility consumers that have not moved to structured cascade output yet,
//! but supported values still pass through the property-aware specified and
//! computed-value layers.

mod builder;
mod document;
mod format;
#[cfg(any(test, feature = "css-fuzzing"))]
pub mod fuzz;
mod impact;
mod legacy;
mod normalize;
mod style;
mod style_tree;
mod value;

pub use builder::ComputedStyleBuilder;
pub use document::{
    ComputedDocumentStyle, ComputedDocumentStyleWithStats, ComputedElementStyle,
    ComputedStyleResolutionError, ComputedStyleReuseStats, IncrementalComputedDocumentStyle,
    compute_document_styles, compute_document_styles_from_resolved_styles,
    compute_document_styles_from_resolved_styles_with_reuse_stats,
    compute_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
    compute_document_styles_incremental_suffix_with_limits, compute_document_styles_with_limits,
    compute_style_from_resolved_style,
};
pub use format::computed_value_debug_snapshot;
pub use impact::{ComputedDocumentStyleLayoutImpact, ComputedStyleLayoutImpact};
pub use legacy::{build_style_tree, compute_style};
pub use style::{
    BorderEdges, BorderSide, BoxMetrics, ComputedStyle, ComputedStyleBuildError,
    ComputedStyleEntry, Outline,
};
pub use style_tree::{
    StylePhaseOutput, StyledNode, build_style_tree_from_computed_styles,
    build_style_tree_with_stylesheets,
};
pub use value::{
    ComputedValue, ComputedValueDiscriminant, ComputedValueNormalizationError,
    ComputedValueNormalizationErrorKind, normalize_specified_value,
};

#[cfg(test)]
mod tests;
