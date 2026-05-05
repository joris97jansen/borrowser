//! Layout-phase box-tree and geometry primitives.
//!
//! The architecture contract for Borrowser's box tree, formatting-context
//! model, and layout responsibility boundaries is documented in
//! `docs/rendering/w1-box-tree-layout-model-contract.md`; the W2 data model is
//! documented in `docs/rendering/w2-structured-box-tree-data-structures.md`;
//! anonymous generation is documented in
//! `docs/rendering/w4-anonymous-box-generation-supported-subset.md`;
//! containing-block relationships are documented in
//! `docs/rendering/w5-containing-block-relationships.md`; block formatting
//! context foundations are documented in
//! `docs/rendering/w6-block-formatting-context-foundations.md`; inline
//! formatting context foundations are documented in
//! `docs/rendering/w7-inline-formatting-context-foundations.md`; deterministic
//! box-generation debug surfaces are documented in
//! `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`; Milestone
//! W close-out invariants and extension hooks are documented in
//! `docs/rendering/w9-box-tree-invariants-extension-hooks.md`; Milestone X
//! sizing architecture and flow-correctness contracts are documented in
//! `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`.
//! `BoxTree` is the frame-local generated box-tree structure; `LayoutBox` is
//! the current geometry projection consumed by paint and hit testing.

mod box_kind;
mod box_tree;
mod debug;
mod document;
mod geometry;
mod layout_box;
mod phase;
mod replaced_element;
mod sizing;
mod text;

pub mod hit_test;
pub mod inline;
pub mod replaced;

pub use box_kind::{BoxKind, ListMarker};
pub use box_tree::{
    AnonymousBoxKind, BlockFormattingParticipation, BoxGenerationRole, BoxId, BoxNode, BoxSource,
    BoxSuppressionReason, BoxTree, ContainingBlockId, DisplayBoxBehavior, DisplayBoxGeneration,
    FormattingContextId, FormattingContextKind, InlineFormattingContextId,
    InlineFormattingParticipation, PrincipalBox,
};
pub use document::{layout_block_tree, layout_document};
pub use geometry::{Rectangle, content_height, content_x_and_width, content_y};
pub use hit_test::{HitKind, hit_test};
pub use inline::{LineBox, layout_inline_for_paint};
pub use layout_box::LayoutBox;
pub use phase::{LayoutPhaseInput, LayoutPhaseOutput};
pub use replaced_element::{ReplacedElementInfoProvider, ReplacedKind};
pub use sizing::{
    AspectRatio, AvailableSize, AxisSizeConstraints, ConstraintSpace, CssPx, IntrinsicSizes,
    SizeAxis, SizeConstraints, SizeResolutionReason, UsedAxisSize, UsedContentSize,
};
pub use text::TextMeasurer;

pub(crate) use debug::{
    box_kind_debug_label, intrinsic_size_debug_label, list_marker_debug_label, node_debug_label,
    replaced_kind_debug_label,
};
pub(crate) use replaced_element::classify_replaced_kind;
