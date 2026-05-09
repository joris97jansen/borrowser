//! Advanced normal-flow contract primitives for Milestone Y.
//!
//! These types name the layout-owned decisions that margins, overflow,
//! positioned containing blocks, and out-of-flow descendants must use as they
//! become implemented. They are intentionally independent of CSS parsing: CSS
//! computes property values, while layout maps those values to these contracts.

use std::fmt::Write;

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
