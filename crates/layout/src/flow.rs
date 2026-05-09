//! Advanced normal-flow contract primitives for Milestone Y.
//!
//! These types name the layout-owned decisions that margins, overflow,
//! positioned containing blocks, and out-of-flow descendants must use as they
//! become implemented. They are intentionally independent of CSS parsing: CSS
//! computes property values, while layout maps those values to these contracts.

use std::fmt::Write;

use css::BoxMetrics;

use crate::sizing::{CssPx, SignedCssPx};

/// Positioning scheme after computed CSS `position` has been interpreted by
/// layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositioningScheme {
    /// `position: static`; ordinary in-flow layout.
    Static,
    /// `position: relative`; in-flow sizing/placement with a later layout-owned
    /// visual offset. The offset must not change normal-flow contribution.
    Relative,
    /// `position: absolute`; removed from normal-flow sizing and block
    /// stacking, then laid out against a positioned containing block.
    Absolute,
    /// `position: fixed`; removed from normal flow and currently defined
    /// against the initial containing block until viewport/transform rules are
    /// introduced.
    Fixed,
    /// `position: sticky`; in-flow layout with later scroll-dependent offsets.
    Sticky,
}

impl PositioningScheme {
    pub fn flow_participation(self) -> FlowParticipation {
        match self {
            Self::Absolute => FlowParticipation::OutOfFlow(OutOfFlowKind::AbsolutelyPositioned),
            Self::Fixed => FlowParticipation::OutOfFlow(OutOfFlowKind::FixedPositioned),
            Self::Static | Self::Relative | Self::Sticky => FlowParticipation::InFlow,
        }
    }

    /// Whether this box can establish the containing block for positioned
    /// descendants.
    pub fn establishes_positioned_containing_block(self) -> bool {
        !matches!(self, Self::Static)
    }

    /// Containing-block lookup strategy used when laying out this box if it is
    /// positioned.
    pub fn positioned_containing_block_strategy(self) -> PositionedContainingBlockStrategy {
        match self {
            Self::Absolute => PositionedContainingBlockStrategy::NearestPositionedAncestor,
            Self::Fixed => PositionedContainingBlockStrategy::InitialContainingBlock,
            Self::Static | Self::Relative | Self::Sticky => {
                PositionedContainingBlockStrategy::NormalFlowContainingBlock
            }
        }
    }

    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Relative => "relative",
            Self::Absolute => "absolute",
            Self::Fixed => "fixed",
            Self::Sticky => "sticky",
        }
    }
}

/// How a generated box contributes to normal-flow layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowParticipation {
    /// The box contributes to parent auto sizes, block stacking, inline line
    /// building, and normal-flow paint order according to its formatting role.
    InFlow,
    /// The box is excluded from parent normal-flow size contribution and is
    /// queued for a later out-of-flow positioning pass.
    OutOfFlow(OutOfFlowKind),
}

impl FlowParticipation {
    pub fn contributes_to_parent_flow(self) -> bool {
        matches!(self, Self::InFlow)
    }

    pub fn out_of_flow_kind(self) -> Option<OutOfFlowKind> {
        match self {
            Self::InFlow => None,
            Self::OutOfFlow(kind) => Some(kind),
        }
    }

    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::InFlow => "in-flow",
            Self::OutOfFlow(OutOfFlowKind::AbsolutelyPositioned) => "out-of-flow:absolute",
            Self::OutOfFlow(OutOfFlowKind::FixedPositioned) => "out-of-flow:fixed",
        }
    }
}

/// Supported out-of-flow families for Milestone Y positioning groundwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutOfFlowKind {
    AbsolutelyPositioned,
    FixedPositioned,
}

impl OutOfFlowKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::AbsolutelyPositioned => "absolute",
            Self::FixedPositioned => "fixed",
        }
    }
}

/// Containing-block lookup strategy for positioned boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionedContainingBlockStrategy {
    /// Static, relative, and sticky boxes use the ordinary normal-flow
    /// containing block for their in-flow geometry.
    NormalFlowContainingBlock,
    /// Absolutely positioned boxes use the nearest ancestor that establishes a
    /// positioned containing block. The initial containing block is the
    /// fallback when no such ancestor exists.
    NearestPositionedAncestor,
    /// Fixed positioned boxes resolve against the initial containing block in
    /// the current supported subset.
    InitialContainingBlock,
}

impl PositionedContainingBlockStrategy {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::NormalFlowContainingBlock => "normal-flow-containing-block",
            Self::NearestPositionedAncestor => "nearest-positioned-ancestor",
            Self::InitialContainingBlock => "initial-containing-block",
        }
    }
}

/// Canonical overflow keyword consumed by layout after CSS cascade/computed
/// value resolution has interpreted `overflow`, `overflow-x`, and `overflow-y`.
///
/// Layout maps this keyword to layout/paint effects. It must not reimplement
/// CSS shorthand expansion or computed-value axis coupling. Until those CSS
/// properties exist, tests may construct this policy directly as architecture
/// vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowKeyword {
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

impl OverflowKeyword {
    pub fn clips_paint(self) -> bool {
        !matches!(self, Self::Visible)
    }

    pub fn creates_scroll_container(self) -> bool {
        matches!(self, Self::Hidden | Self::Scroll | Self::Auto)
    }

    pub fn establishes_independent_formatting_context(self) -> bool {
        matches!(self, Self::Hidden | Self::Scroll | Self::Auto)
    }

    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::Clip => "clip",
            Self::Scroll => "scroll",
            Self::Auto => "auto",
        }
    }
}

/// Layout-owned interpretation of canonical inline/block overflow behavior.
///
/// This is the contract passed toward layout and paint. It is not a raw CSS
/// declaration representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverflowPolicy {
    inline: OverflowKeyword,
    block: OverflowKeyword,
}

impl OverflowPolicy {
    pub fn uniform(keyword: OverflowKeyword) -> Self {
        Self {
            inline: keyword,
            block: keyword,
        }
    }

    pub fn new(inline: OverflowKeyword, block: OverflowKeyword) -> Self {
        Self { inline, block }
    }

    pub fn inline(self) -> OverflowKeyword {
        self.inline
    }

    pub fn block(self) -> OverflowKeyword {
        self.block
    }

    pub fn clips_paint(self) -> bool {
        self.inline.clips_paint() || self.block.clips_paint()
    }

    pub fn creates_scroll_container(self) -> bool {
        self.inline.creates_scroll_container() || self.block.creates_scroll_container()
    }

    /// Whether this overflow policy should isolate normal-flow descendants in
    /// an independent block formatting context.
    pub fn establishes_independent_formatting_context(self) -> bool {
        self.inline.establishes_independent_formatting_context()
            || self.block.establishes_independent_formatting_context()
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "inline={} block={}",
            self.inline.as_debug_label(),
            self.block.as_debug_label()
        )
    }
}

/// Supported adjoining-margin collapse cases for Milestone Y.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginCollapseCase {
    AdjacentBlockSiblings,
    ParentBlockStartWithFirstInFlowChild,
    ParentBlockEndWithLastInFlowChild,
    EmptyBlockSelfCollapse,
}

impl MarginCollapseCase {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::AdjacentBlockSiblings => "adjacent-block-siblings",
            Self::ParentBlockStartWithFirstInFlowChild => {
                "parent-block-start-with-first-in-flow-child"
            }
            Self::ParentBlockEndWithLastInFlowChild => "parent-block-end-with-last-in-flow-child",
            Self::EmptyBlockSelfCollapse => "empty-block-self-collapse",
        }
    }
}

/// Boundaries that prevent margins from adjoining in the supported Y subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarginCollapseBoundary {
    RootElement,
    IndependentFormattingContext,
    OutOfFlow,
    InlineFormattingContent,
    NonZeroPaddingOrBorder,
    Clearance,
    OverflowFormattingContext,
    Fragmentation,
}

impl MarginCollapseBoundary {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::RootElement => "root-element",
            Self::IndependentFormattingContext => "independent-formatting-context",
            Self::OutOfFlow => "out-of-flow",
            Self::InlineFormattingContent => "inline-formatting-content",
            Self::NonZeroPaddingOrBorder => "non-zero-padding-or-border",
            Self::Clearance => "clearance",
            Self::OverflowFormattingContext => "overflow-formatting-context",
            Self::Fragmentation => "fragmentation",
        }
    }
}

/// Logical side names for margins in the current horizontal writing-mode
/// subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowMarginSide {
    BlockStart,
    InlineEnd,
    BlockEnd,
    InlineStart,
}

impl FlowMarginSide {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::BlockStart => "block-start",
            Self::InlineEnd => "inline-end",
            Self::BlockEnd => "block-end",
            Self::InlineStart => "inline-start",
        }
    }
}

/// Error returned when box metrics cannot be materialized as finite layout
/// margins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlowMarginError {
    side: FlowMarginSide,
}

impl FlowMarginError {
    fn new(side: FlowMarginSide) -> Self {
        Self { side }
    }

    pub fn side(self) -> FlowMarginSide {
        self.side
    }
}

impl std::fmt::Display for FlowMarginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "flow margin '{}' must be finite",
            self.side.as_debug_label()
        )
    }
}

impl std::error::Error for FlowMarginError {}

/// Explicit margins used by normal-flow placement.
///
/// Values are logical in the current horizontal writing-mode subset:
/// block-start maps to physical top, inline-end to right, block-end to bottom,
/// and inline-start to left. Negative values are valid and remain signed until
/// a non-negative size, such as available inline space, is produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowMargins {
    block_start: SignedCssPx,
    inline_end: SignedCssPx,
    block_end: SignedCssPx,
    inline_start: SignedCssPx,
}

impl FlowMargins {
    pub fn zero() -> Self {
        Self {
            block_start: SignedCssPx::ZERO,
            inline_end: SignedCssPx::ZERO,
            block_end: SignedCssPx::ZERO,
            inline_start: SignedCssPx::ZERO,
        }
    }

    pub fn new(
        block_start: SignedCssPx,
        inline_end: SignedCssPx,
        block_end: SignedCssPx,
        inline_start: SignedCssPx,
    ) -> Self {
        Self {
            block_start,
            inline_end,
            block_end,
            inline_start,
        }
    }

    pub fn from_box_metrics(metrics: BoxMetrics) -> Result<Self, FlowMarginError> {
        Ok(Self::new(
            signed_margin(metrics.margin_top, FlowMarginSide::BlockStart)?,
            signed_margin(metrics.margin_right, FlowMarginSide::InlineEnd)?,
            signed_margin(metrics.margin_bottom, FlowMarginSide::BlockEnd)?,
            signed_margin(metrics.margin_left, FlowMarginSide::InlineStart)?,
        ))
    }

    pub fn block_start(self) -> SignedCssPx {
        self.block_start
    }

    pub fn inline_end(self) -> SignedCssPx {
        self.inline_end
    }

    pub fn block_end(self) -> SignedCssPx {
        self.block_end
    }

    pub fn inline_start(self) -> SignedCssPx {
        self.inline_start
    }

    pub fn top(self) -> SignedCssPx {
        self.block_start
    }

    pub fn right(self) -> SignedCssPx {
        self.inline_end
    }

    pub fn bottom(self) -> SignedCssPx {
        self.block_end
    }

    pub fn left(self) -> SignedCssPx {
        self.inline_start
    }

    /// Position and available-space inputs for an in-flow child inside a
    /// parent's content box.
    ///
    /// The containing inline size is intentionally preserved as the parent
    /// content-box size. Margins only move the child border box and narrow, or
    /// with negative margins expand, the available inline space offered to
    /// auto/stretch and shrink-to-fit sizing.
    pub fn apply_to_child_inline_axis(
        self,
        parent_content_inline_start: SignedCssPx,
        parent_content_inline_size: CssPx,
    ) -> MarginAdjustedChildInline {
        let border_inline_start = signed_px_sum(parent_content_inline_start, self.inline_start());
        let available = (parent_content_inline_size.get()
            - self.inline_start().get()
            - self.inline_end().get())
        .max(0.0);

        MarginAdjustedChildInline {
            border_inline_start,
            containing_inline_size: parent_content_inline_size,
            available_inline_size: CssPx::new(available).expect("available size is non-negative"),
        }
    }

    pub fn apply_block_start(self, current_block_position: SignedCssPx) -> SignedCssPx {
        signed_px_sum(current_block_position, self.block_start())
    }

    pub fn advance_after_border_box(
        self,
        border_block_start: SignedCssPx,
        border_block_size: CssPx,
    ) -> SignedCssPx {
        signed_px_sum(
            signed_px_sum(border_block_start, signed_from_css_px(border_block_size)),
            self.block_end(),
        )
    }

    pub fn margin_box_inline_size(self, border_inline_size: CssPx) -> CssPx {
        let size = (border_inline_size.get() + self.inline_start().get() + self.inline_end().get())
            .max(0.0);
        CssPx::new(size).expect("margin box inline size is non-negative")
    }

    pub fn margin_box_block_size(self, border_block_size: CssPx) -> CssPx {
        let size =
            (border_block_size.get() + self.block_start().get() + self.block_end().get()).max(0.0);
        CssPx::new(size).expect("margin box block size is non-negative")
    }

    pub fn positive_inline_sum(self) -> CssPx {
        CssPx::new(self.inline_start().get().max(0.0) + self.inline_end().get().max(0.0))
            .expect("positive inline margin sum is non-negative")
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "(block-start={} inline-end={} block-end={} inline-start={})",
            signed_px_debug_label(self.block_start),
            signed_px_debug_label(self.inline_end),
            signed_px_debug_label(self.block_end),
            signed_px_debug_label(self.inline_start),
        )
    }
}

/// Inline-axis inputs derived from a child's margins and parent content box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarginAdjustedChildInline {
    border_inline_start: SignedCssPx,
    containing_inline_size: CssPx,
    available_inline_size: CssPx,
}

impl MarginAdjustedChildInline {
    pub fn border_inline_start(self) -> SignedCssPx {
        self.border_inline_start
    }

    pub fn containing_inline_size(self) -> CssPx {
        self.containing_inline_size
    }

    pub fn available_inline_size(self) -> CssPx {
        self.available_inline_size
    }
}

/// Result of collapsing one adjoining margin set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollapsedMargin {
    value: SignedCssPx,
    max_positive: CssPx,
    min_negative: SignedCssPx,
}

impl CollapsedMargin {
    pub fn zero() -> Self {
        Self {
            value: SignedCssPx::ZERO,
            max_positive: CssPx::ZERO,
            min_negative: SignedCssPx::ZERO,
        }
    }

    /// Collapse adjoining margins using the CSS positive/negative rule:
    /// largest positive margin plus most negative margin, with zero used for a
    /// missing sign group.
    pub fn from_adjoining(margins: &[SignedCssPx]) -> Self {
        let mut max_positive = 0.0_f32;
        let mut min_negative = 0.0_f32;

        for margin in margins {
            let value = margin.get();
            if value > max_positive {
                max_positive = value;
            } else if value < min_negative {
                min_negative = value;
            }
        }

        Self {
            value: SignedCssPx::new(max_positive + min_negative)
                .expect("collapsed margin inputs are finite"),
            max_positive: CssPx::new(max_positive).expect("positive margin contribution"),
            min_negative: SignedCssPx::new(min_negative).expect("negative margin contribution"),
        }
    }

    pub fn value(self) -> SignedCssPx {
        self.value
    }

    pub fn max_positive(self) -> CssPx {
        self.max_positive
    }

    pub fn min_negative(self) -> SignedCssPx {
        self.min_negative
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "value={} positive={} negative={}",
            signed_px_debug_label(self.value),
            css_px_debug_label(self.max_positive),
            signed_px_debug_label(self.min_negative),
        )
    }
}

/// Deterministic record for one permitted margin-collapse operation.
///
/// The case name identifies the category of adjoining margins already validated
/// by the caller. It is not itself permission to collapse arbitrary margins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarginCollapseDecision {
    case: MarginCollapseCase,
    previous_margin: SignedCssPx,
    next_margin: SignedCssPx,
    collapsed: CollapsedMargin,
}

impl MarginCollapseDecision {
    /// Collapse the block-end margin of one in-flow block sibling with the
    /// block-start margin of the next in-flow block sibling.
    pub fn adjacent_block_siblings(
        previous_block_end_margin: SignedCssPx,
        next_block_start_margin: SignedCssPx,
    ) -> Self {
        Self {
            case: MarginCollapseCase::AdjacentBlockSiblings,
            previous_margin: previous_block_end_margin,
            next_margin: next_block_start_margin,
            collapsed: CollapsedMargin::from_adjoining(&[
                previous_block_end_margin,
                next_block_start_margin,
            ]),
        }
    }

    pub fn case(self) -> MarginCollapseCase {
        self.case
    }

    pub fn previous_margin(self) -> SignedCssPx {
        self.previous_margin
    }

    pub fn next_margin(self) -> SignedCssPx {
        self.next_margin
    }

    pub fn collapsed_margin(self) -> CollapsedMargin {
        self.collapsed
    }

    pub fn block_offset(self) -> SignedCssPx {
        self.collapsed.value()
    }

    pub fn as_debug_label(self) -> String {
        format!(
            "case={} previous={} next={} collapsed=({})",
            self.case.as_debug_label(),
            signed_px_debug_label(self.previous_margin),
            signed_px_debug_label(self.next_margin),
            self.collapsed.as_debug_label(),
        )
    }
}

/// Block-axis cursor that applies the Y3 supported margin-collapse subset.
///
/// Supported today:
/// - the first in-flow block child keeps its own block-start margin
/// - adjacent in-flow block siblings collapse the previous block-end margin
///   with the next block-start margin
/// - the last in-flow block child keeps its own block-end margin
///
/// Parent/child collapse and empty-block self-collapse need additional
/// boundary and content checks, so they are intentionally not folded into this
/// cursor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockFlowMarginCollapseCursor {
    content_block_start: SignedCssPx,
    previous_in_flow_block: Option<PreviousInFlowBlockMargin>,
}

impl BlockFlowMarginCollapseCursor {
    pub fn new(content_block_start: SignedCssPx) -> Self {
        Self {
            content_block_start,
            previous_in_flow_block: None,
        }
    }

    pub fn next_in_flow_block(self, margins: FlowMargins) -> BlockFlowBlockPlacement {
        match self.previous_in_flow_block {
            Some(previous) => {
                let decision = MarginCollapseDecision::adjacent_block_siblings(
                    previous.block_end_margin,
                    margins.block_start(),
                );
                BlockFlowBlockPlacement {
                    border_block_start: signed_px_sum(
                        previous.border_block_end,
                        decision.block_offset(),
                    ),
                    margin_collapse: Some(decision),
                }
            }
            None => BlockFlowBlockPlacement {
                border_block_start: margins.apply_block_start(self.content_block_start),
                margin_collapse: None,
            },
        }
    }

    pub fn finish_in_flow_block(
        &mut self,
        border_block_start: SignedCssPx,
        border_block_size: CssPx,
        margins: FlowMargins,
    ) {
        self.previous_in_flow_block = Some(PreviousInFlowBlockMargin {
            border_block_end: signed_px_sum(
                border_block_start,
                signed_from_css_px(border_block_size),
            ),
            block_end_margin: margins.block_end(),
        });
    }

    pub fn current_block_position(self) -> SignedCssPx {
        match self.previous_in_flow_block {
            Some(previous) => signed_px_sum(previous.border_block_end, previous.block_end_margin),
            None => self.content_block_start,
        }
    }

    pub fn auto_content_block_size(self) -> CssPx {
        CssPx::new((self.current_block_position().get() - self.content_block_start.get()).max(0.0))
            .expect("auto content block size is non-negative")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreviousInFlowBlockMargin {
    border_block_end: SignedCssPx,
    block_end_margin: SignedCssPx,
}

/// Placement decision for one in-flow block child.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockFlowBlockPlacement {
    border_block_start: SignedCssPx,
    margin_collapse: Option<MarginCollapseDecision>,
}

impl BlockFlowBlockPlacement {
    pub fn border_block_start(self) -> SignedCssPx {
        self.border_block_start
    }

    pub fn margin_collapse(self) -> Option<MarginCollapseDecision> {
        self.margin_collapse
    }
}

/// Stable architecture/debug surface for Y1.
pub fn advanced_flow_contract_debug_snapshot() -> String {
    let mut out = String::new();
    writeln!(&mut out, "version: 1").expect("write snapshot");
    writeln!(&mut out, "advanced-flow-contract").expect("write snapshot");
    writeln!(
        &mut out,
        "margin-collapse-cases: {}, {}, {}, {}",
        MarginCollapseCase::AdjacentBlockSiblings.as_debug_label(),
        MarginCollapseCase::ParentBlockStartWithFirstInFlowChild.as_debug_label(),
        MarginCollapseCase::ParentBlockEndWithLastInFlowChild.as_debug_label(),
        MarginCollapseCase::EmptyBlockSelfCollapse.as_debug_label(),
    )
    .expect("write snapshot");
    writeln!(
        &mut out,
        "margin-collapse-boundaries: {}, {}, {}, {}, {}, {}, {}, {}",
        MarginCollapseBoundary::RootElement.as_debug_label(),
        MarginCollapseBoundary::IndependentFormattingContext.as_debug_label(),
        MarginCollapseBoundary::OutOfFlow.as_debug_label(),
        MarginCollapseBoundary::InlineFormattingContent.as_debug_label(),
        MarginCollapseBoundary::NonZeroPaddingOrBorder.as_debug_label(),
        MarginCollapseBoundary::Clearance.as_debug_label(),
        MarginCollapseBoundary::OverflowFormattingContext.as_debug_label(),
        MarginCollapseBoundary::Fragmentation.as_debug_label(),
    )
    .expect("write snapshot");
    writeln!(
        &mut out,
        "overflow: visible(clip=no scroll-container=no bfc=no), hidden(clip=yes scroll-container=yes bfc=yes), clip(clip=yes scroll-container=no bfc=no), scroll(clip=yes scroll-container=yes bfc=yes), auto(clip=yes scroll-container=yes bfc=yes)"
    )
    .expect("write snapshot");
    writeln!(
        &mut out,
        "positioning: static(flow=in-flow cb=normal-flow-containing-block establishes-positioned-cb=no), relative(flow=in-flow cb=normal-flow-containing-block establishes-positioned-cb=yes), absolute(flow=out-of-flow:absolute cb=nearest-positioned-ancestor establishes-positioned-cb=yes), fixed(flow=out-of-flow:fixed cb=initial-containing-block establishes-positioned-cb=yes), sticky(flow=in-flow cb=normal-flow-containing-block establishes-positioned-cb=yes)"
    )
    .expect("write snapshot");
    writeln!(
        &mut out,
        "out-of-flow: queue-after-normal-flow=yes contributes-to-parent-auto-size=no final-geometry-from-positioning-phase=yes"
    )
    .expect("write snapshot");
    out
}

fn css_px_debug_label(value: CssPx) -> String {
    format!("{:.2}px", value.get())
}

fn signed_px_debug_label(value: SignedCssPx) -> String {
    format!("{:.2}px", value.get())
}

fn signed_margin(value: f32, side: FlowMarginSide) -> Result<SignedCssPx, FlowMarginError> {
    SignedCssPx::new(value).ok_or_else(|| FlowMarginError::new(side))
}

fn signed_px_sum(a: SignedCssPx, b: SignedCssPx) -> SignedCssPx {
    SignedCssPx::new(a.get() + b.get()).expect("signed px sum is finite")
}

fn signed_from_css_px(value: CssPx) -> SignedCssPx {
    SignedCssPx::new(value.get()).expect("css px is finite")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed(value: f32) -> SignedCssPx {
        SignedCssPx::new(value).expect("finite signed px")
    }

    fn css(value: f32) -> CssPx {
        CssPx::new(value).expect("non-negative css px")
    }

    #[test]
    fn flow_margins_materialize_finite_signed_box_metrics() {
        let margins = FlowMargins::from_box_metrics(BoxMetrics {
            margin_top: 5.0,
            margin_right: -3.0,
            margin_bottom: 7.0,
            margin_left: -2.0,
            ..BoxMetrics::zero()
        })
        .expect("flow margins");

        assert_eq!(margins.block_start(), signed(5.0));
        assert_eq!(margins.inline_end(), signed(-3.0));
        assert_eq!(margins.block_end(), signed(7.0));
        assert_eq!(margins.inline_start(), signed(-2.0));
        assert_eq!(
            margins.as_debug_label(),
            "(block-start=5.00px inline-end=-3.00px block-end=7.00px inline-start=-2.00px)"
        );
    }

    #[test]
    fn flow_margins_reject_non_finite_values_with_side_metadata() {
        let error = FlowMargins::from_box_metrics(BoxMetrics {
            margin_top: f32::INFINITY,
            ..BoxMetrics::zero()
        })
        .expect_err("non-finite margin must be rejected");

        assert_eq!(error.side(), FlowMarginSide::BlockStart);
        assert_eq!(
            error.to_string(),
            "flow margin 'block-start' must be finite"
        );
    }

    #[test]
    fn flow_margins_apply_child_inline_axis_without_changing_containing_size() {
        let margins = FlowMargins::new(signed(0.0), signed(30.0), signed(0.0), signed(20.0));
        let child = margins.apply_to_child_inline_axis(signed(10.0), css(200.0));

        assert_eq!(child.border_inline_start(), signed(30.0));
        assert_eq!(child.containing_inline_size(), css(200.0));
        assert_eq!(child.available_inline_size(), css(150.0));
    }

    #[test]
    fn negative_flow_margins_expand_available_inline_space() {
        let margins = FlowMargins::new(signed(0.0), signed(30.0), signed(0.0), signed(-20.0));
        let child = margins.apply_to_child_inline_axis(signed(0.0), css(200.0));

        assert_eq!(child.border_inline_start(), signed(-20.0));
        assert_eq!(child.containing_inline_size(), css(200.0));
        assert_eq!(child.available_inline_size(), css(190.0));
    }

    #[test]
    fn flow_margins_apply_block_axis_positions_and_margin_box_sizes() {
        let margins = FlowMargins::new(signed(5.0), signed(3.0), signed(7.0), signed(2.0));
        let border_start = margins.apply_block_start(signed(10.0));
        let after = margins.advance_after_border_box(border_start, css(20.0));

        assert_eq!(border_start, signed(15.0));
        assert_eq!(after, signed(42.0));
        assert_eq!(margins.margin_box_inline_size(css(100.0)), css(105.0));
        assert_eq!(margins.margin_box_block_size(css(20.0)), css(32.0));
        assert_eq!(margins.positive_inline_sum(), css(5.0));
    }

    #[test]
    fn collapsed_margin_uses_css_positive_negative_rule() {
        let mixed = CollapsedMargin::from_adjoining(&[
            signed(10.0),
            signed(-8.0),
            signed(5.0),
            signed(-2.0),
        ]);
        assert_eq!(mixed.value(), signed(2.0));
        assert_eq!(mixed.max_positive(), css(10.0));
        assert_eq!(mixed.min_negative(), signed(-8.0));
        assert_eq!(
            mixed.as_debug_label(),
            "value=2.00px positive=10.00px negative=-8.00px"
        );

        let positive = CollapsedMargin::from_adjoining(&[signed(4.0), signed(12.0)]);
        assert_eq!(positive.value(), signed(12.0));
        assert_eq!(positive.max_positive(), css(12.0));
        assert_eq!(positive.min_negative(), SignedCssPx::ZERO);

        let negative = CollapsedMargin::from_adjoining(&[signed(-4.0), signed(-12.0)]);
        assert_eq!(negative.value(), signed(-12.0));
        assert_eq!(negative.max_positive(), CssPx::ZERO);
        assert_eq!(negative.min_negative(), signed(-12.0));

        assert_eq!(
            CollapsedMargin::from_adjoining(&[]),
            CollapsedMargin::zero()
        );
    }

    #[test]
    fn margin_collapse_decision_records_adjacent_sibling_inputs() {
        let decision = MarginCollapseDecision::adjacent_block_siblings(signed(10.0), signed(-8.0));

        assert_eq!(decision.case(), MarginCollapseCase::AdjacentBlockSiblings);
        assert_eq!(decision.previous_margin(), signed(10.0));
        assert_eq!(decision.next_margin(), signed(-8.0));
        assert_eq!(decision.block_offset(), signed(2.0));
        assert_eq!(
            decision.as_debug_label(),
            "case=adjacent-block-siblings previous=10.00px next=-8.00px collapsed=(value=2.00px positive=10.00px negative=-8.00px)"
        );
    }

    #[test]
    fn block_flow_margin_cursor_collapses_adjacent_sibling_margins() {
        let first_margins = FlowMargins::new(signed(5.0), signed(0.0), signed(10.0), signed(0.0));
        let second_margins = FlowMargins::new(signed(20.0), signed(0.0), signed(7.0), signed(0.0));
        let mut cursor = BlockFlowMarginCollapseCursor::new(signed(0.0));

        let first = cursor.next_in_flow_block(first_margins);
        assert_eq!(first.border_block_start(), signed(5.0));
        assert_eq!(first.margin_collapse(), None);
        cursor.finish_in_flow_block(first.border_block_start(), css(10.0), first_margins);

        let second = cursor.next_in_flow_block(second_margins);
        let collapse = second.margin_collapse().expect("sibling collapse");
        assert_eq!(collapse.block_offset(), signed(20.0));
        assert_eq!(second.border_block_start(), signed(35.0));
        cursor.finish_in_flow_block(second.border_block_start(), css(15.0), second_margins);

        assert_eq!(cursor.current_block_position(), signed(57.0));
        assert_eq!(cursor.auto_content_block_size(), css(57.0));
    }

    #[test]
    fn block_flow_margin_cursor_uses_css_negative_collapse_rule() {
        let first_margins = FlowMargins::new(signed(0.0), signed(0.0), signed(8.0), signed(0.0));
        let second_margins =
            FlowMargins::new(signed(-13.0), signed(0.0), signed(-4.0), signed(0.0));
        let mut cursor = BlockFlowMarginCollapseCursor::new(signed(0.0));

        let first = cursor.next_in_flow_block(first_margins);
        cursor.finish_in_flow_block(first.border_block_start(), css(10.0), first_margins);

        let second = cursor.next_in_flow_block(second_margins);
        let collapse = second.margin_collapse().expect("sibling collapse");
        assert_eq!(collapse.block_offset(), signed(-5.0));
        assert_eq!(second.border_block_start(), signed(5.0));
        cursor.finish_in_flow_block(second.border_block_start(), css(10.0), second_margins);

        assert_eq!(cursor.current_block_position(), signed(11.0));
        assert_eq!(cursor.auto_content_block_size(), css(11.0));
    }

    #[test]
    fn positioning_scheme_defines_flow_and_containing_block_contracts() {
        assert_eq!(
            PositioningScheme::Static.flow_participation(),
            FlowParticipation::InFlow
        );
        assert_eq!(
            PositioningScheme::Relative.flow_participation(),
            FlowParticipation::InFlow
        );
        assert_eq!(
            PositioningScheme::Sticky.flow_participation(),
            FlowParticipation::InFlow
        );
        assert_eq!(
            PositioningScheme::Absolute.flow_participation(),
            FlowParticipation::OutOfFlow(OutOfFlowKind::AbsolutelyPositioned)
        );
        assert_eq!(
            PositioningScheme::Fixed.flow_participation(),
            FlowParticipation::OutOfFlow(OutOfFlowKind::FixedPositioned)
        );

        assert!(!PositioningScheme::Static.establishes_positioned_containing_block());
        assert!(PositioningScheme::Relative.establishes_positioned_containing_block());
        assert!(PositioningScheme::Absolute.establishes_positioned_containing_block());
        assert!(PositioningScheme::Fixed.establishes_positioned_containing_block());
        assert!(PositioningScheme::Sticky.establishes_positioned_containing_block());

        assert_eq!(
            PositioningScheme::Absolute.positioned_containing_block_strategy(),
            PositionedContainingBlockStrategy::NearestPositionedAncestor
        );
        assert_eq!(
            PositioningScheme::Fixed.positioned_containing_block_strategy(),
            PositionedContainingBlockStrategy::InitialContainingBlock
        );
    }

    #[test]
    fn overflow_policy_separates_layout_and_paint_effects() {
        let visible = OverflowPolicy::uniform(OverflowKeyword::Visible);
        assert!(!visible.clips_paint());
        assert!(!visible.creates_scroll_container());
        assert!(!visible.establishes_independent_formatting_context());

        let hidden = OverflowPolicy::uniform(OverflowKeyword::Hidden);
        assert!(hidden.clips_paint());
        assert!(hidden.creates_scroll_container());
        assert!(hidden.establishes_independent_formatting_context());

        let clip = OverflowPolicy::uniform(OverflowKeyword::Clip);
        assert!(clip.clips_paint());
        assert!(!clip.creates_scroll_container());
        assert!(!clip.establishes_independent_formatting_context());

        let mixed = OverflowPolicy::new(OverflowKeyword::Visible, OverflowKeyword::Auto);
        assert!(mixed.clips_paint());
        assert!(mixed.creates_scroll_container());
        assert!(mixed.establishes_independent_formatting_context());
        assert_eq!(mixed.as_debug_label(), "inline=visible block=auto");
    }

    #[test]
    fn flow_contract_snapshot_is_stable() {
        assert_eq!(
            advanced_flow_contract_debug_snapshot(),
            concat!(
                "version: 1\n",
                "advanced-flow-contract\n",
                "margin-collapse-cases: adjacent-block-siblings, parent-block-start-with-first-in-flow-child, parent-block-end-with-last-in-flow-child, empty-block-self-collapse\n",
                "margin-collapse-boundaries: root-element, independent-formatting-context, out-of-flow, inline-formatting-content, non-zero-padding-or-border, clearance, overflow-formatting-context, fragmentation\n",
                "overflow: visible(clip=no scroll-container=no bfc=no), hidden(clip=yes scroll-container=yes bfc=yes), clip(clip=yes scroll-container=no bfc=no), scroll(clip=yes scroll-container=yes bfc=yes), auto(clip=yes scroll-container=yes bfc=yes)\n",
                "positioning: static(flow=in-flow cb=normal-flow-containing-block establishes-positioned-cb=no), relative(flow=in-flow cb=normal-flow-containing-block establishes-positioned-cb=yes), absolute(flow=out-of-flow:absolute cb=nearest-positioned-ancestor establishes-positioned-cb=yes), fixed(flow=out-of-flow:fixed cb=initial-containing-block establishes-positioned-cb=yes), sticky(flow=in-flow cb=normal-flow-containing-block establishes-positioned-cb=yes)\n",
                "out-of-flow: queue-after-normal-flow=yes contributes-to-parent-auto-size=no final-geometry-from-positioning-phase=yes\n",
            )
        );
    }
}
