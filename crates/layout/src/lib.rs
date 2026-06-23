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
//! `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`; X2
//! structured sizing inputs are documented in
//! `docs/rendering/x2-structured-size-resolution-model-inputs.md`; X3
//! supported width/height resolution is documented in
//! `docs/rendering/x3-width-height-resolution-supported-subset.md`; X4
//! intrinsic sizing for supported content is documented in
//! `docs/rendering/x4-intrinsic-sizing-supported-content.md`; X5 min/max
//! sizing constraints are documented in
//! `docs/rendering/x5-min-max-sizing-constraints.md`; X6 percentage sizing is
//! documented in `docs/rendering/x6-percentage-sizing-targeted-subset.md`; X7
//! shrink-to-fit and containing-size-dependent sizing are documented in
//! `docs/rendering/x7-shrink-to-fit-containing-size-dependent-sizing.md`; X8
//! flow correctness under varied sizing conditions is documented in
//! `docs/rendering/x8-flow-correctness-varied-sizing.md`; X9 deterministic
//! sizing debug and regression surfaces are documented in
//! `docs/rendering/x9-deterministic-sizing-debug-regressions.md`; X10
//! Milestone X close-out invariants and extension hooks are documented in
//! `docs/rendering/x10-sizing-invariants-extension-hooks.md`; Y1 advanced flow
//! architecture and contracts are documented in
//! `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`; Y2
//! structured margin handling is documented in
//! `docs/rendering/y2-structured-margin-handling.md`; Y3 adjacent block
//! sibling margin collapsing is documented in
//! `docs/rendering/y3-margin-collapsing-supported-subset.md`; Y4 overflow
//! layout and paint semantics are documented in
//! `docs/rendering/y4-overflow-semantics-supported-subset.md`; Y5 positioned
//! containing-block logic is documented in
//! `docs/rendering/y5-positioned-containing-block-logic.md`; Y6 out-of-flow
//! layout participation groundwork is documented in
//! `docs/rendering/y6-out-of-flow-layout-participation-groundwork.md`; Y8
//! deterministic advanced-flow debug regressions are documented in
//! `docs/rendering/y8-deterministic-advanced-flow-debug-regressions.md`; Y9
//! Milestone Y close-out invariants and extension points are documented in
//! `docs/rendering/y9-advanced-flow-invariants-extension-points.md`; Z1 flex
//! layout architecture is documented in
//! `docs/rendering/z1-flex-layout-architecture-contract.md`; Z2 flex box-tree
//! structure is documented in
//! `docs/rendering/z2-flex-box-tree-structure.md`; Z3 flex main-axis layout is
//! documented in
//! `docs/rendering/z3-flex-main-axis-layout-core-subset.md`; Z4 flex
//! cross-axis layout is documented in
//! `docs/rendering/z4-flex-cross-axis-layout-core-subset.md`; Z5 flex
//! integration hardening is documented in
//! `docs/rendering/z5-flex-layout-integration-hardening.md`; Z6 unsupported
//! flex feature handling is documented in
//! `docs/rendering/z6-flex-unsupported-feature-handling.md`.
//! `BoxTree` is the frame-local generated box-tree structure; `LayoutBox` is
//! the current geometry projection consumed by paint and hit testing.

mod box_kind;
mod box_tree;
mod debug;
mod document;
mod flex;
mod flow;
mod geometry;
mod layout_box;
mod phase;
mod replaced_element;
mod retained;
mod sizing;
mod text;

pub mod hit_test;
pub mod inline;
pub mod replaced;

pub use box_kind::{BoxKind, ListMarker};
pub use box_tree::{
    AnonymousBoxKind, BlockFormattingParticipation, BoxGenerationRole, BoxId, BoxNode, BoxSource,
    BoxSuppressionReason, BoxTree, ContainingBlockId, DisplayBoxBehavior, DisplayBoxGeneration,
    FlexFormattingParticipation, FormattingContextId, FormattingContextKind,
    InlineFormattingContextId, InlineFormattingParticipation, PositionedContainingBlockId,
    PrincipalBox,
};
pub use document::{layout_block_tree, layout_document};
pub use flex::{
    FlexContainerCrossAxisLayout, FlexContainerMainAxisLayout, FlexCrossAxis,
    FlexCrossAxisAlignment, FlexCrossAxisLayout, FlexFreeSpaceDistribution, FlexItemCrossAxisInput,
    FlexItemCrossAxisLayout, FlexItemMainAxisInput, FlexItemMainAxisLayout, FlexMainAxis,
    FlexMainAxisLayout, resolve_flex_cross_axis_layout, resolve_flex_main_axis_layout,
};
pub use flow::{
    BlockFlowBlockPlacement, BlockFlowMarginCollapseCursor, CollapsedMargin, FlowMarginError,
    FlowMarginSide, FlowMargins, FlowParticipation, MarginAdjustedChildInline,
    MarginCollapseBoundary, MarginCollapseCase, MarginCollapseDecision, OutOfFlowKind,
    OutOfFlowLayoutParticipant, OverflowClip, OverflowKeyword, OverflowPolicy,
    PositionedContainingBlockStrategy, PositioningScheme, advanced_flow_contract_debug_snapshot,
};
pub use geometry::{Rectangle, content_height, content_x_and_width, content_y};
pub use hit_test::{HitKind, hit_test};
pub use inline::{LineBox, layout_inline_for_paint};
pub use layout_box::LayoutBox;
pub use phase::{LayoutPhaseInput, LayoutPhaseOutput};
pub use replaced_element::{ReplacedElementInfoProvider, ReplacedKind};
pub use retained::{
    RetainedLayoutArtifact, RetainedLayoutFallbackReason, RetainedLayoutFrameAction,
    RetainedLayoutFrameResult, RetainedLayoutKey, RetainedLayoutKeySeed,
    RetainedLayoutMaterializationError, RetainedViewportWidthKey,
};
pub use sizing::{
    AppliedSizeConstraint, AspectRatio, AvailableSize, AvailableSpace, AxisSizeConstraints,
    AxisStyleSizeInput, ConstraintSpace, ContainingSize, CssPx, IntrinsicSizes,
    NormalFlowSizingMode, Percentage, PhysicalSides, ResolvedAxisSize, ShrinkToFitDecision,
    ShrinkToFitInput, ShrinkToFitResult, SignedCssPx, SizeAxis, SizeConstraints,
    SizeResolutionInput, SizeResolutionReason, StyleBoxMetrics, StyleMaximumSize, StyleMinimumSize,
    StylePreferredSize, StyleSizeInputError, StyleSizeInputProperty, StyleSizeInputs, UsedAxisSize,
    UsedContentSize, resolve_flex_distributed_block_size, resolve_flex_distributed_inline_size,
    resolve_normal_flow_block_size, resolve_normal_flow_inline_size,
    resolve_shrink_to_fit_inline_size,
};
pub use text::TextMeasurer;

pub(crate) use debug::{
    box_kind_debug_label, intrinsic_size_debug_label, list_marker_debug_label, node_debug_label,
    replaced_kind_debug_label,
};
pub(crate) use replaced_element::classify_replaced_kind;
