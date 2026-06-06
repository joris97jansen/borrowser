//! Sizing contract types for Milestone X.
//!
//! This module defines the shared vocabulary future sizing resolvers must use.
//! It deliberately does not replace the current geometry pass yet; X1 is the
//! architecture boundary that later X issues will implement against.

use std::fmt::Write;

use crate::ContainingBlockId;
use css::{BoxMetrics, ComputedStyle, Length, LengthPercentage};

/// Non-negative finite CSS px value used by sizing algorithms.
///
/// `Rectangle` remains f32-backed for the current geometry projection, but
/// sizing algorithms should validate scalar inputs before producing used sizes.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CssPx(f32);

impl CssPx {
    pub const ZERO: Self = Self(0.0);

    pub fn new(value: f32) -> Option<Self> {
        if value.is_finite() && value >= 0.0 {
            Some(Self(if value == 0.0 { 0.0 } else { value }))
        } else {
            None
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Finite CSS px scalar that may be negative.
///
/// This is used for style inputs such as margins where negative values are
/// valid. Used sizes and available sizes must continue to use `CssPx`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct SignedCssPx(f32);

impl SignedCssPx {
    pub const ZERO: Self = Self(0.0);

    pub fn new(value: f32) -> Option<Self> {
        if value.is_finite() {
            Some(Self(if value == 0.0 { 0.0 } else { value }))
        } else {
            None
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Non-negative finite CSS percentage represented as a fraction.
///
/// `1.0` is 100%, `0.5` is 50%.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Percentage(f32);

impl Percentage {
    pub const ZERO: Self = Self(0.0);

    pub fn from_fraction(value: f32) -> Option<Self> {
        if value.is_finite() && value >= 0.0 {
            Some(Self(if value == 0.0 { 0.0 } else { value }))
        } else {
            None
        }
    }

    pub fn from_percent(value: f32) -> Option<Self> {
        Self::from_fraction(value / 100.0)
    }

    pub fn fraction(self) -> f32 {
        self.0
    }

    /// Resolve against an already-selected size basis.
    ///
    /// Callers resolving CSS sizing percentages should normally pass the
    /// relevant `ContainingSize` axis, not narrowed `AvailableSpace`, unless a
    /// specific CSS rule explicitly defines available space as its basis.
    pub fn resolve_against(self, basis: AvailableSize) -> Option<CssPx> {
        basis
            .definite_value()
            .and_then(|basis| CssPx::new(basis.get() * self.fraction()))
    }
}

/// Positive finite aspect ratio, represented as inline-size / block-size.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct AspectRatio(f32);

impl AspectRatio {
    pub fn new(value: f32) -> Option<Self> {
        if value.is_finite() && value > 0.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// Logical axis used by the sizing model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeAxis {
    Inline,
    Block,
}

/// Available size from a containing block or formatting context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvailableSize {
    /// A definite size can resolve lengths and percentages for the same axis.
    Definite(CssPx),
    /// An indefinite size cannot resolve percentages during this pass.
    Indefinite,
}

impl AvailableSize {
    pub fn definite(value: f32) -> Option<Self> {
        CssPx::new(value).map(Self::Definite)
    }

    pub fn definite_value(self) -> Option<CssPx> {
        match self {
            Self::Definite(value) => Some(value),
            Self::Indefinite => None,
        }
    }

    pub fn is_definite(self) -> bool {
        matches!(self, Self::Definite(_))
    }
}

/// Explicit containing block content-box dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContainingSize {
    containing_block: Option<ContainingBlockId>,
    inline_size: AvailableSize,
    block_size: AvailableSize,
}

impl ContainingSize {
    pub fn new(
        containing_block: Option<ContainingBlockId>,
        inline_size: AvailableSize,
        block_size: AvailableSize,
    ) -> Self {
        Self {
            containing_block,
            inline_size,
            block_size,
        }
    }

    pub fn containing_block(self) -> Option<ContainingBlockId> {
        self.containing_block
    }

    pub fn inline_size(self) -> AvailableSize {
        self.inline_size
    }

    pub fn block_size(self) -> AvailableSize {
        self.block_size
    }

    pub fn size(self, axis: SizeAxis) -> AvailableSize {
        match axis {
            SizeAxis::Inline => self.inline_size,
            SizeAxis::Block => self.block_size,
        }
    }
}

/// Available content-box space offered by the formatting context.
///
/// This normally matches the containing content size for simple normal flow,
/// but the separate type lets later layout modes narrow available space without
/// changing the containing block percentage basis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvailableSpace {
    inline_size: AvailableSize,
    block_size: AvailableSize,
}

impl AvailableSpace {
    pub fn new(inline_size: AvailableSize, block_size: AvailableSize) -> Self {
        Self {
            inline_size,
            block_size,
        }
    }

    pub fn from_containing_size(containing_size: ContainingSize) -> Self {
        Self::new(containing_size.inline_size(), containing_size.block_size())
    }

    pub fn inline_size(self) -> AvailableSize {
        self.inline_size
    }

    pub fn block_size(self) -> AvailableSize {
        self.block_size
    }

    pub fn size(self, axis: SizeAxis) -> AvailableSize {
        match axis {
            SizeAxis::Inline => self.inline_size,
            SizeAxis::Block => self.block_size,
        }
    }
}

/// The sizing environment for a box before used-size resolution.
///
/// Available sizes are content-box bases from the containing formatting
/// context. They are the basis for percentage resolution and line-width
/// selection; a box's own margins, borders, and padding are handled by the
/// used-size resolver when converting style and box metrics into content-box
/// sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstraintSpace {
    containing_size: ContainingSize,
    available_space: AvailableSpace,
}

impl ConstraintSpace {
    pub fn new(
        containing_block: Option<ContainingBlockId>,
        available_inline_size: AvailableSize,
        available_block_size: AvailableSize,
    ) -> Self {
        let containing_size = ContainingSize::new(
            containing_block,
            available_inline_size,
            available_block_size,
        );
        Self::from_containing_size(containing_size)
    }

    pub fn from_containing_size(containing_size: ContainingSize) -> Self {
        Self {
            containing_size,
            available_space: AvailableSpace::from_containing_size(containing_size),
        }
    }

    pub fn with_available_space(mut self, available_space: AvailableSpace) -> Self {
        self.available_space = available_space;
        self
    }

    pub fn containing_size(self) -> ContainingSize {
        self.containing_size
    }

    pub fn available_space(self) -> AvailableSpace {
        self.available_space
    }

    pub fn containing_block(self) -> Option<ContainingBlockId> {
        self.containing_size.containing_block()
    }

    pub fn available_inline_size(self) -> AvailableSize {
        self.available_space.inline_size()
    }

    pub fn available_block_size(self) -> AvailableSize {
        self.available_space.block_size()
    }

    pub fn available_size(self, axis: SizeAxis) -> AvailableSize {
        self.available_space.size(axis)
    }

    pub fn containing_size_for_axis(self, axis: SizeAxis) -> AvailableSize {
        self.containing_size.size(axis)
    }

    /// Available content-box size after subtracting the box's own non-margin
    /// edges for the selected axis.
    ///
    /// This uses `AvailableSpace`, not `ContainingSize`: it is suitable for
    /// auto-stretch sizing, line-width selection, and shrink-to-fit available
    /// space. CSS percentage resolution must continue to use
    /// `containing_size_for_axis`.
    pub fn available_size_after_edges(
        self,
        axis: SizeAxis,
        start_edge: CssPx,
        end_edge: CssPx,
    ) -> AvailableSize {
        match self.available_size(axis) {
            AvailableSize::Definite(value) => {
                AvailableSize::Definite(subtract_css_px(value, css_px_sum(start_edge, end_edge)))
            }
            AvailableSize::Indefinite => AvailableSize::Indefinite,
        }
    }
}

/// Physical four-sided style input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalSides<T> {
    top: T,
    right: T,
    bottom: T,
    left: T,
}

impl<T: Copy> PhysicalSides<T> {
    pub fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn top(self) -> T {
        self.top
    }

    pub fn right(self) -> T {
        self.right
    }

    pub fn bottom(self) -> T {
        self.bottom
    }

    pub fn left(self) -> T {
        self.left
    }
}

/// Box metric style inputs required by size resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleBoxMetrics {
    margin: PhysicalSides<SignedCssPx>,
    padding: PhysicalSides<CssPx>,
}

impl StyleBoxMetrics {
    pub fn zero() -> Self {
        Self {
            margin: PhysicalSides::new(
                SignedCssPx::ZERO,
                SignedCssPx::ZERO,
                SignedCssPx::ZERO,
                SignedCssPx::ZERO,
            ),
            padding: PhysicalSides::new(CssPx::ZERO, CssPx::ZERO, CssPx::ZERO, CssPx::ZERO),
        }
    }

    pub fn new(margin: PhysicalSides<SignedCssPx>, padding: PhysicalSides<CssPx>) -> Self {
        Self { margin, padding }
    }

    pub fn from_box_metrics(metrics: BoxMetrics) -> Result<Self, StyleSizeInputError> {
        Ok(Self {
            margin: PhysicalSides::new(
                signed_length(metrics.margin_top, StyleSizeInputProperty::MarginTop)?,
                signed_length(metrics.margin_right, StyleSizeInputProperty::MarginRight)?,
                signed_length(metrics.margin_bottom, StyleSizeInputProperty::MarginBottom)?,
                signed_length(metrics.margin_left, StyleSizeInputProperty::MarginLeft)?,
            ),
            padding: PhysicalSides::new(
                non_negative_length(metrics.padding_top, StyleSizeInputProperty::PaddingTop)?,
                non_negative_length(metrics.padding_right, StyleSizeInputProperty::PaddingRight)?,
                non_negative_length(
                    metrics.padding_bottom,
                    StyleSizeInputProperty::PaddingBottom,
                )?,
                non_negative_length(metrics.padding_left, StyleSizeInputProperty::PaddingLeft)?,
            ),
        })
    }

    pub fn margin(self) -> PhysicalSides<SignedCssPx> {
        self.margin
    }

    pub fn padding(self) -> PhysicalSides<CssPx> {
        self.padding
    }
}

/// Style-provided preferred size for one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StylePreferredSize {
    Auto,
    Length(CssPx),
    Percentage(Percentage),
}

/// Style-provided minimum size for one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleMinimumSize {
    Auto,
    Length(CssPx),
    Percentage(Percentage),
}

/// Style-provided maximum size for one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleMaximumSize {
    None,
    Length(CssPx),
    Percentage(Percentage),
}

/// Style sizing inputs for one logical axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisStyleSizeInput {
    preferred: StylePreferredSize,
    min: StyleMinimumSize,
    max: StyleMaximumSize,
}

impl AxisStyleSizeInput {
    pub fn new(
        preferred: StylePreferredSize,
        min: StyleMinimumSize,
        max: StyleMaximumSize,
    ) -> Self {
        Self {
            preferred,
            min,
            max,
        }
    }

    pub fn preferred(self) -> StylePreferredSize {
        self.preferred
    }

    pub fn min(self) -> StyleMinimumSize {
        self.min
    }

    pub fn max(self) -> StyleMaximumSize {
        self.max
    }
}

/// Style-driven sizing inputs for both axes in the current horizontal subset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleSizeInputs {
    inline: AxisStyleSizeInput,
    block: AxisStyleSizeInput,
    box_metrics: StyleBoxMetrics,
}

impl StyleSizeInputs {
    pub fn auto_zero() -> Self {
        let axis = AxisStyleSizeInput::new(
            StylePreferredSize::Auto,
            StyleMinimumSize::Auto,
            StyleMaximumSize::None,
        );
        Self::new(axis, axis, StyleBoxMetrics::zero())
    }

    pub fn new(
        inline: AxisStyleSizeInput,
        block: AxisStyleSizeInput,
        box_metrics: StyleBoxMetrics,
    ) -> Self {
        Self {
            inline,
            block,
            box_metrics,
        }
    }

    pub fn from_computed_style(style: &ComputedStyle) -> Result<Self, StyleSizeInputError> {
        let inline = AxisStyleSizeInput::new(
            preferred_size_from_length_percentage_or_auto(
                style.width(),
                StyleSizeInputProperty::Width,
            )?,
            minimum_size_from_length_percentage_or_auto(
                style.min_width(),
                StyleSizeInputProperty::MinWidth,
            )?,
            maximum_size_from_length_percentage_or_none(
                style.max_width(),
                StyleSizeInputProperty::MaxWidth,
            )?,
        );
        let block = AxisStyleSizeInput::new(
            preferred_size_from_length_percentage_or_auto(
                style.height(),
                StyleSizeInputProperty::Height,
            )?,
            StyleMinimumSize::Auto,
            StyleMaximumSize::None,
        );

        Ok(Self::new(
            inline,
            block,
            StyleBoxMetrics::from_box_metrics(style.box_metrics())?,
        ))
    }

    pub fn inline(self) -> AxisStyleSizeInput {
        self.inline
    }

    pub fn block(self) -> AxisStyleSizeInput {
        self.block
    }

    pub fn axis(self, axis: SizeAxis) -> AxisStyleSizeInput {
        match axis {
            SizeAxis::Inline => self.inline,
            SizeAxis::Block => self.block,
        }
    }

    pub fn box_metrics(self) -> StyleBoxMetrics {
        self.box_metrics
    }
}

/// Intrinsic contribution produced by a supported content type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntrinsicSizes {
    min_content_inline_size: CssPx,
    max_content_inline_size: CssPx,
    preferred_inline_size: Option<CssPx>,
    preferred_block_size: Option<CssPx>,
    aspect_ratio: Option<AspectRatio>,
}

impl IntrinsicSizes {
    pub fn new(
        min_content_inline_size: CssPx,
        max_content_inline_size: CssPx,
        preferred_inline_size: Option<CssPx>,
        preferred_block_size: Option<CssPx>,
        aspect_ratio: Option<AspectRatio>,
    ) -> Option<Self> {
        if max_content_inline_size < min_content_inline_size {
            return None;
        }

        Some(Self {
            min_content_inline_size,
            max_content_inline_size,
            preferred_inline_size,
            preferred_block_size,
            aspect_ratio,
        })
    }

    pub fn zero() -> Self {
        Self {
            min_content_inline_size: CssPx::ZERO,
            max_content_inline_size: CssPx::ZERO,
            preferred_inline_size: None,
            preferred_block_size: None,
            aspect_ratio: None,
        }
    }

    pub fn min_content_inline_size(self) -> CssPx {
        self.min_content_inline_size
    }

    pub fn max_content_inline_size(self) -> CssPx {
        self.max_content_inline_size
    }

    pub fn preferred_inline_size(self) -> Option<CssPx> {
        self.preferred_inline_size
    }

    pub fn preferred_block_size(self) -> Option<CssPx> {
        self.preferred_block_size
    }

    pub fn aspect_ratio(self) -> Option<AspectRatio> {
        self.aspect_ratio
    }
}

/// Input to the supported shrink-to-fit inline-size algorithm.
///
/// Min/max content sizes are intrinsic content-box contributions. The
/// available inline size is the content-box space offered by the formatting
/// context after the box's own padding has been subtracted. It is deliberately
/// separate from the containing-size percentage basis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShrinkToFitInput {
    min_content_inline_size: CssPx,
    max_content_inline_size: CssPx,
    preferred_inline_size: Option<CssPx>,
    available_inline_size: AvailableSize,
}

impl ShrinkToFitInput {
    pub fn new(
        min_content_inline_size: CssPx,
        max_content_inline_size: CssPx,
        preferred_inline_size: Option<CssPx>,
        available_inline_size: AvailableSize,
    ) -> Option<Self> {
        if max_content_inline_size < min_content_inline_size {
            return None;
        }

        Some(Self {
            min_content_inline_size,
            max_content_inline_size,
            preferred_inline_size,
            available_inline_size,
        })
    }

    pub fn from_intrinsic_sizes(
        intrinsic: IntrinsicSizes,
        available_inline_size: AvailableSize,
    ) -> Self {
        Self {
            min_content_inline_size: intrinsic.min_content_inline_size(),
            max_content_inline_size: intrinsic.max_content_inline_size(),
            preferred_inline_size: intrinsic.preferred_inline_size(),
            available_inline_size,
        }
    }

    pub fn min_content_inline_size(self) -> CssPx {
        self.min_content_inline_size
    }

    pub fn max_content_inline_size(self) -> CssPx {
        self.max_content_inline_size
    }

    pub fn preferred_inline_size(self) -> Option<CssPx> {
        self.preferred_inline_size
    }

    pub fn available_inline_size(self) -> AvailableSize {
        self.available_inline_size
    }
}

/// Deterministic branch taken by shrink-to-fit resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShrinkToFitDecision {
    EmptyIntrinsicContribution,
    IndefiniteAvailableSpace,
    MinContentFloor,
    AvailableSpace,
    PreferredCeiling,
}

/// Result of supported shrink-to-fit inline-size resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShrinkToFitResult {
    value: CssPx,
    decision: ShrinkToFitDecision,
}

impl ShrinkToFitResult {
    fn new(value: CssPx, decision: ShrinkToFitDecision) -> Self {
        Self { value, decision }
    }

    pub fn value(self) -> CssPx {
        self.value
    }

    pub fn decision(self) -> ShrinkToFitDecision {
        self.decision
    }
}

/// Complete input bundle for resolving the used size of one generated box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeResolutionInput {
    constraint_space: ConstraintSpace,
    style: StyleSizeInputs,
    intrinsic: IntrinsicSizes,
}

impl SizeResolutionInput {
    pub fn new(
        constraint_space: ConstraintSpace,
        style: StyleSizeInputs,
        intrinsic: IntrinsicSizes,
    ) -> Self {
        Self {
            constraint_space,
            style,
            intrinsic,
        }
    }

    pub fn constraint_space(self) -> ConstraintSpace {
        self.constraint_space
    }

    pub fn style(self) -> StyleSizeInputs {
        self.style
    }

    pub fn intrinsic(self) -> IntrinsicSizes {
        self.intrinsic
    }

    /// Stable semantic debug snapshot for one normal-flow size-resolution input.
    ///
    /// The snapshot is intentionally derived from the same typed inputs and
    /// resolver outputs used by layout. It is a regression surface for sizing
    /// behavior, not a rendering or pixel-output format.
    pub fn to_debug_snapshot(
        self,
        mode: NormalFlowSizingMode,
        auto_content_block_size: CssPx,
    ) -> String {
        let inline = resolve_normal_flow_inline_size(self, mode);
        let block = resolve_normal_flow_block_size(self, mode, auto_content_block_size);
        let constraint_space = self.constraint_space();
        let containing_size = constraint_space.containing_size();
        let available_space = constraint_space.available_space();
        let style = self.style();
        let metrics = style.box_metrics();
        let intrinsic = self.intrinsic();

        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "size-resolution").expect("write snapshot");
        writeln!(&mut out, "mode: {}", mode.as_debug_label()).expect("write snapshot");
        writeln!(
            &mut out,
            "auto-content-block-size: {}",
            css_px_debug_label(auto_content_block_size)
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "constraint-space: containing-block={} containing-inline={} containing-block-size={} available-inline={} available-block={}",
            optional_containing_block_debug_label(containing_size.containing_block()),
            available_size_debug_label(containing_size.inline_size()),
            available_size_debug_label(containing_size.block_size()),
            available_size_debug_label(available_space.inline_size()),
            available_size_debug_label(available_space.block_size()),
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "style: inline({}) block({})",
            axis_style_input_debug_label(style.inline()),
            axis_style_input_debug_label(style.block()),
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "box-metrics: margin={} padding={}",
            signed_sides_debug_label(metrics.margin()),
            css_px_sides_debug_label(metrics.padding()),
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "intrinsic: min-content-inline={} max-content-inline={} preferred-inline={} preferred-block={} aspect-ratio={}",
            css_px_debug_label(intrinsic.min_content_inline_size()),
            css_px_debug_label(intrinsic.max_content_inline_size()),
            optional_css_px_debug_label(intrinsic.preferred_inline_size()),
            optional_css_px_debug_label(intrinsic.preferred_block_size()),
            optional_aspect_ratio_debug_label(intrinsic.aspect_ratio()),
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "result-inline: {} border={}",
            used_axis_size_debug_label(inline.content()),
            css_px_debug_label(inline.border()),
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "result-block: {} border={}",
            used_axis_size_debug_label(block.content()),
            css_px_debug_label(block.border()),
        )
        .expect("write snapshot");
        out
    }
}

/// Supported normal-flow sizing behavior for the current layout subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalFlowSizingMode {
    Document,
    BlockLevel,
    InlineLevel,
    AtomicInline,
    FlexItemMainAxis,
    Anonymous,
}

impl NormalFlowSizingMode {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::BlockLevel => "block-level",
            Self::InlineLevel => "inline-level",
            Self::AtomicInline => "atomic-inline",
            Self::FlexItemMainAxis => "flex-item-main-axis",
            Self::Anonymous => "anonymous",
        }
    }
}

/// Resolved content-box and border-box size for one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedAxisSize {
    content: UsedAxisSize,
    border: CssPx,
}

impl ResolvedAxisSize {
    pub fn new(content: UsedAxisSize, border: CssPx) -> Self {
        Self { content, border }
    }

    pub fn content(self) -> UsedAxisSize {
        self.content
    }

    pub fn border(self) -> CssPx {
        self.border
    }
}

pub fn resolve_normal_flow_inline_size(
    input: SizeResolutionInput,
    mode: NormalFlowSizingMode,
) -> ResolvedAxisSize {
    let style = input.style();
    let padding = style.box_metrics().padding();
    let padding_inline = css_px_sum(padding.left(), padding.right());
    let available_content_size = input.constraint_space().available_size_after_edges(
        SizeAxis::Inline,
        padding.left(),
        padding.right(),
    );
    let available_content = available_content_size.definite_value();
    let basis = input
        .constraint_space()
        .containing_size_for_axis(SizeAxis::Inline);
    let axis = style.inline();
    let intrinsic = input.intrinsic();

    let (preferred_value, preferred_reason) = match (mode, axis.preferred()) {
        (NormalFlowSizingMode::InlineLevel, _) => auto_stretch_preferred(available_content),
        (_, StylePreferredSize::Length(value)) => (value, SizeResolutionReason::DefiniteLength),
        (_, StylePreferredSize::Percentage(percentage)) => percentage
            .resolve_against(basis)
            .map(|value| {
                (
                    value,
                    SizeResolutionReason::PercentageOfDefiniteContainingBlock,
                )
            })
            .unwrap_or((
                CssPx::ZERO,
                SizeResolutionReason::DeferredIndefinitePercentage,
            )),
        (NormalFlowSizingMode::AtomicInline, StylePreferredSize::Auto) => {
            atomic_inline_auto_preferred(intrinsic, available_content_size)
        }
        (NormalFlowSizingMode::FlexItemMainAxis, StylePreferredSize::Auto) => {
            flex_item_main_axis_auto_preferred(intrinsic)
        }
        (_, StylePreferredSize::Auto) => auto_stretch_preferred(available_content),
    };

    let constraints = inline_constraints(axis, basis);
    let allow_available_space_clamp = !matches!(
        (mode, axis.preferred()),
        (NormalFlowSizingMode::AtomicInline, StylePreferredSize::Auto)
    );
    let atomic_inline_available_space_clamp =
        if allow_available_space_clamp && matches!(mode, NormalFlowSizingMode::AtomicInline) {
            available_content
        } else {
            None
        };
    let (value, applied_constraint) = constraints
        .clamp_with_available_space(preferred_value, atomic_inline_available_space_clamp);
    let used = used_axis_size(preferred_value, preferred_reason, value, applied_constraint);
    let border = css_px_sum(value, padding_inline);

    ResolvedAxisSize::new(used, border)
}

pub fn resolve_flex_distributed_inline_size(
    input: SizeResolutionInput,
    preferred_content_size: CssPx,
) -> ResolvedAxisSize {
    let style = input.style();
    let padding = style.box_metrics().padding();
    let padding_inline = css_px_sum(padding.left(), padding.right());
    let basis = input
        .constraint_space()
        .containing_size_for_axis(SizeAxis::Inline);
    let constraints = inline_constraints(style.inline(), basis);
    let (value, applied_constraint) =
        constraints.clamp_with_applied_constraint(preferred_content_size);
    let used = used_axis_size(
        preferred_content_size,
        SizeResolutionReason::FlexDistributed,
        value,
        applied_constraint,
    );
    let border = css_px_sum(value, padding_inline);

    ResolvedAxisSize::new(used, border)
}

pub fn resolve_flex_distributed_block_size(
    input: SizeResolutionInput,
    preferred_content_size: CssPx,
) -> ResolvedAxisSize {
    let style = input.style();
    let padding = style.box_metrics().padding();
    let padding_block = css_px_sum(padding.top(), padding.bottom());
    let basis = input
        .constraint_space()
        .containing_size_for_axis(SizeAxis::Block);
    let constraints = block_constraints(style.block(), basis);
    let (value, applied_constraint) =
        constraints.clamp_with_applied_constraint(preferred_content_size);
    let used = used_axis_size(
        preferred_content_size,
        SizeResolutionReason::FlexDistributed,
        value,
        applied_constraint,
    );
    let border = css_px_sum(value, padding_block);

    ResolvedAxisSize::new(used, border)
}

pub fn resolve_normal_flow_block_size(
    input: SizeResolutionInput,
    mode: NormalFlowSizingMode,
    auto_content_block_size: CssPx,
) -> ResolvedAxisSize {
    let style = input.style();
    let padding = style.box_metrics().padding();
    let padding_block = css_px_sum(padding.top(), padding.bottom());
    let basis = input
        .constraint_space()
        .containing_size_for_axis(SizeAxis::Block);
    let axis = style.block();

    let (preferred_value, preferred_reason) = match (mode, axis.preferred()) {
        (NormalFlowSizingMode::InlineLevel, _) => {
            (CssPx::ZERO, SizeResolutionReason::AutoContentBased)
        }
        (_, StylePreferredSize::Length(value)) => (value, SizeResolutionReason::DefiniteLength),
        (_, StylePreferredSize::Percentage(percentage)) => percentage
            .resolve_against(basis)
            .map(|value| {
                (
                    value,
                    SizeResolutionReason::PercentageOfDefiniteContainingBlock,
                )
            })
            .unwrap_or((
                auto_content_block_size,
                SizeResolutionReason::DeferredIndefinitePercentage,
            )),
        (_, StylePreferredSize::Auto) => (
            auto_content_block_size,
            SizeResolutionReason::AutoContentBased,
        ),
    };

    let constraints = block_constraints(axis, basis);
    let (value, applied_constraint) = constraints.clamp_with_applied_constraint(preferred_value);
    let used = used_axis_size(preferred_value, preferred_reason, value, applied_constraint);
    let border = css_px_sum(value, padding_block);

    ResolvedAxisSize::new(used, border)
}

fn inline_constraints(axis: AxisStyleSizeInput, basis: AvailableSize) -> AxisSizeConstraints {
    let min = match axis.min() {
        StyleMinimumSize::Auto => None,
        StyleMinimumSize::Length(value) => Some(value),
        StyleMinimumSize::Percentage(percentage) => percentage.resolve_against(basis),
    };

    let style_max = match axis.max() {
        StyleMaximumSize::None => None,
        StyleMaximumSize::Length(value) => Some(value),
        StyleMaximumSize::Percentage(percentage) => percentage.resolve_against(basis),
    };

    AxisSizeConstraints::new(min, style_max)
}

fn block_constraints(axis: AxisStyleSizeInput, basis: AvailableSize) -> AxisSizeConstraints {
    let min = match axis.min() {
        StyleMinimumSize::Auto => None,
        StyleMinimumSize::Length(value) => Some(value),
        StyleMinimumSize::Percentage(percentage) => percentage.resolve_against(basis),
    };
    let max = match axis.max() {
        StyleMaximumSize::None => None,
        StyleMaximumSize::Length(value) => Some(value),
        StyleMaximumSize::Percentage(percentage) => percentage.resolve_against(basis),
    };

    AxisSizeConstraints::new(min, max)
}

fn used_axis_size(
    preferred_value: CssPx,
    preferred_reason: SizeResolutionReason,
    value: CssPx,
    applied_constraint: AppliedSizeConstraint,
) -> UsedAxisSize {
    match applied_constraint {
        AppliedSizeConstraint::None => UsedAxisSize::unconstrained(value, preferred_reason),
        AppliedSizeConstraint::Min
        | AppliedSizeConstraint::Max
        | AppliedSizeConstraint::AvailableSpaceClamp => {
            UsedAxisSize::constrained(preferred_value, preferred_reason, value, applied_constraint)
        }
    }
}

fn auto_stretch_preferred(available_content: Option<CssPx>) -> (CssPx, SizeResolutionReason) {
    available_content
        .map(|available_content| {
            (
                available_content,
                SizeResolutionReason::AutoStretchToContainingBlock,
            )
        })
        .unwrap_or((CssPx::ZERO, SizeResolutionReason::UnsupportedDeferred))
}

fn atomic_inline_auto_preferred(
    intrinsic: IntrinsicSizes,
    available_content: AvailableSize,
) -> (CssPx, SizeResolutionReason) {
    let result = resolve_shrink_to_fit_inline_size(ShrinkToFitInput::from_intrinsic_sizes(
        intrinsic,
        available_content,
    ));

    let reason = match result.decision() {
        ShrinkToFitDecision::EmptyIntrinsicContribution => SizeResolutionReason::AutoContentBased,
        ShrinkToFitDecision::IndefiniteAvailableSpace => {
            SizeResolutionReason::IntrinsicPreferredSize
        }
        ShrinkToFitDecision::MinContentFloor
        | ShrinkToFitDecision::AvailableSpace
        | ShrinkToFitDecision::PreferredCeiling => SizeResolutionReason::ShrinkToFit,
    };

    (result.value(), reason)
}

fn flex_item_main_axis_auto_preferred(intrinsic: IntrinsicSizes) -> (CssPx, SizeResolutionReason) {
    let result = resolve_shrink_to_fit_inline_size(ShrinkToFitInput::from_intrinsic_sizes(
        intrinsic,
        AvailableSize::Indefinite,
    ));

    let reason = match result.decision() {
        ShrinkToFitDecision::EmptyIntrinsicContribution => SizeResolutionReason::AutoContentBased,
        ShrinkToFitDecision::IndefiniteAvailableSpace => {
            SizeResolutionReason::IntrinsicPreferredSize
        }
        ShrinkToFitDecision::MinContentFloor
        | ShrinkToFitDecision::AvailableSpace
        | ShrinkToFitDecision::PreferredCeiling => SizeResolutionReason::ShrinkToFit,
    };

    (result.value(), reason)
}

pub fn resolve_shrink_to_fit_inline_size(input: ShrinkToFitInput) -> ShrinkToFitResult {
    let min_content = input.min_content_inline_size();
    let max_content = input.max_content_inline_size();
    let preferred = input.preferred_inline_size().unwrap_or(max_content);
    let preferred_ceiling = clamp_css_px(preferred, min_content, max_content);

    if min_content == CssPx::ZERO && max_content == CssPx::ZERO && preferred_ceiling == CssPx::ZERO
    {
        return ShrinkToFitResult::new(
            CssPx::ZERO,
            ShrinkToFitDecision::EmptyIntrinsicContribution,
        );
    }

    match input.available_inline_size() {
        AvailableSize::Indefinite => ShrinkToFitResult::new(
            preferred_ceiling,
            ShrinkToFitDecision::IndefiniteAvailableSpace,
        ),
        AvailableSize::Definite(available_content) => {
            let value =
                shrink_to_fit_inline_size(min_content, preferred_ceiling, available_content);
            let decision = if value == min_content && available_content < min_content {
                ShrinkToFitDecision::MinContentFloor
            } else if value == preferred_ceiling && available_content > preferred_ceiling {
                ShrinkToFitDecision::PreferredCeiling
            } else {
                ShrinkToFitDecision::AvailableSpace
            };
            ShrinkToFitResult::new(value, decision)
        }
    }
}

fn shrink_to_fit_inline_size(
    min_content: CssPx,
    preferred_ceiling: CssPx,
    available_content: CssPx,
) -> CssPx {
    if available_content < min_content {
        min_content
    } else if available_content > preferred_ceiling {
        preferred_ceiling
    } else {
        available_content
    }
}

fn clamp_css_px(value: CssPx, min: CssPx, max: CssPx) -> CssPx {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Min/max constraints for one logical axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisSizeConstraints {
    min: Option<CssPx>,
    max: Option<CssPx>,
}

impl AxisSizeConstraints {
    pub const NONE: Self = Self {
        min: None,
        max: None,
    };

    pub fn new(min: Option<CssPx>, max: Option<CssPx>) -> Self {
        Self { min, max }
    }

    pub fn min(self) -> Option<CssPx> {
        self.min
    }

    pub fn max(self) -> Option<CssPx> {
        self.max
    }

    /// Apply max then min so the minimum constraint wins if constraints cross.
    pub fn clamp(self, value: CssPx) -> CssPx {
        self.clamp_with_applied_constraint(value).0
    }

    /// Apply max then min and report which constraint produced the final size.
    pub fn clamp_with_applied_constraint(self, value: CssPx) -> (CssPx, AppliedSizeConstraint) {
        self.clamp_with_available_space(value, None)
    }

    /// Apply style max, then an optional atomic-inline available-space clamp, then style min.
    ///
    /// This preserves the supported normal-flow ordering: style max-width can
    /// cap the preferred size, an atomic-inline available content-space clamp can cap
    /// that result, and style min-width wins last if constraints cross.
    pub fn clamp_with_available_space(
        self,
        value: CssPx,
        atomic_inline_available_space_clamp: Option<CssPx>,
    ) -> (CssPx, AppliedSizeConstraint) {
        let mut out = value;
        let mut applied = AppliedSizeConstraint::None;
        if let Some(max) = self.max.filter(|max| out > *max) {
            out = max;
            applied = AppliedSizeConstraint::Max;
        }
        if let Some(clamp) = atomic_inline_available_space_clamp.filter(|clamp| out > *clamp) {
            out = clamp;
            applied = AppliedSizeConstraint::AvailableSpaceClamp;
        }
        if let Some(min) = self.min.filter(|min| out < *min) {
            out = min;
            applied = AppliedSizeConstraint::Min;
        }
        (out, applied)
    }
}

/// Min/max constraints for both logical axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeConstraints {
    inline: AxisSizeConstraints,
    block: AxisSizeConstraints,
}

impl SizeConstraints {
    pub const NONE: Self = Self {
        inline: AxisSizeConstraints::NONE,
        block: AxisSizeConstraints::NONE,
    };

    pub fn new(inline: AxisSizeConstraints, block: AxisSizeConstraints) -> Self {
        Self { inline, block }
    }

    pub fn inline(self) -> AxisSizeConstraints {
        self.inline
    }

    pub fn block(self) -> AxisSizeConstraints {
        self.block
    }

    pub fn axis(self, axis: SizeAxis) -> AxisSizeConstraints {
        match axis {
            SizeAxis::Inline => self.inline,
            SizeAxis::Block => self.block,
        }
    }
}

/// Why the preferred size was selected before min/max constraint application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeResolutionReason {
    DefiniteLength,
    PercentageOfDefiniteContainingBlock,
    AutoStretchToContainingBlock,
    AutoContentBased,
    IntrinsicPreferredSize,
    IntrinsicAspectRatioTransfer,
    ShrinkToFit,
    FlexDistributed,
    DeferredIndefinitePercentage,
    UnsupportedDeferred,
}

impl SizeResolutionReason {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::DefiniteLength => "definite-length",
            Self::PercentageOfDefiniteContainingBlock => "percentage-of-definite-containing-block",
            Self::AutoStretchToContainingBlock => "auto-stretch-to-containing-block",
            Self::AutoContentBased => "auto-content-based",
            Self::IntrinsicPreferredSize => "intrinsic-preferred-size",
            Self::IntrinsicAspectRatioTransfer => "intrinsic-aspect-ratio-transfer",
            Self::ShrinkToFit => "shrink-to-fit",
            Self::FlexDistributed => "flex-distributed",
            Self::DeferredIndefinitePercentage => "deferred-indefinite-percentage",
            Self::UnsupportedDeferred => "unsupported-deferred",
        }
    }
}

/// Post-preferred size adjustment applied after preferred-size resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppliedSizeConstraint {
    None,
    /// A style `min-width`/`min-height` constraint produced the final size.
    Min,
    /// A style `max-width`/`max-height` constraint produced the final size.
    Max,
    /// The formatting context's definite available space clamped the final size.
    AvailableSpaceClamp,
}

impl AppliedSizeConstraint {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Min => "min",
            Self::Max => "max",
            Self::AvailableSpaceClamp => "available-space-clamp",
        }
    }
}

/// Used content-box size for one axis plus deterministic sizing metadata.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsedAxisSize {
    preferred_value: CssPx,
    preferred_reason: SizeResolutionReason,
    value: CssPx,
    applied_constraint: AppliedSizeConstraint,
}

impl UsedAxisSize {
    pub fn unconstrained(value: CssPx, reason: SizeResolutionReason) -> Self {
        Self {
            preferred_value: value,
            preferred_reason: reason,
            value,
            applied_constraint: AppliedSizeConstraint::None,
        }
    }

    pub fn constrained(
        preferred_value: CssPx,
        preferred_reason: SizeResolutionReason,
        value: CssPx,
        applied_constraint: AppliedSizeConstraint,
    ) -> Self {
        debug_assert!(
            !matches!(applied_constraint, AppliedSizeConstraint::None) || value == preferred_value,
            "unconstrained used size must preserve the preferred value"
        );
        Self {
            preferred_value,
            preferred_reason,
            value,
            applied_constraint,
        }
    }

    pub fn preferred_value(self) -> CssPx {
        self.preferred_value
    }

    pub fn preferred_reason(self) -> SizeResolutionReason {
        self.preferred_reason
    }

    pub fn value(self) -> CssPx {
        self.value
    }

    pub fn applied_constraint(self) -> AppliedSizeConstraint {
        self.applied_constraint
    }
}

/// Used logical content-box size after width/height resolution.
///
/// This is always content-box size. Padding, border, and margin expansion
/// belong to box-metrics conversion and flow placement, not this primitive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsedContentSize {
    inline: UsedAxisSize,
    block: UsedAxisSize,
}

impl UsedContentSize {
    pub fn new(inline: UsedAxisSize, block: UsedAxisSize) -> Self {
        Self { inline, block }
    }

    pub fn inline(self) -> UsedAxisSize {
        self.inline
    }

    pub fn block(self) -> UsedAxisSize {
        self.block
    }
}

/// Supported style input property labels for deterministic validation errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleSizeInputProperty {
    Width,
    Height,
    MinWidth,
    MaxWidth,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
}

impl StyleSizeInputProperty {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Width => "width",
            Self::Height => "height",
            Self::MinWidth => "min-width",
            Self::MaxWidth => "max-width",
            Self::MarginTop => "margin-top",
            Self::MarginRight => "margin-right",
            Self::MarginBottom => "margin-bottom",
            Self::MarginLeft => "margin-left",
            Self::PaddingTop => "padding-top",
            Self::PaddingRight => "padding-right",
            Self::PaddingBottom => "padding-bottom",
            Self::PaddingLeft => "padding-left",
        }
    }
}

/// Validation error for materializing computed style into sizing inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleSizeInputError {
    InvalidFiniteLength {
        property: StyleSizeInputProperty,
        value: f32,
    },
    InvalidNonNegativeLength {
        property: StyleSizeInputProperty,
        value: f32,
    },
}

impl std::fmt::Display for StyleSizeInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::InvalidFiniteLength { property, value } => write!(
                f,
                "sizing input '{}' must be finite CSS px, got {}",
                property.as_debug_label(),
                value
            ),
            Self::InvalidNonNegativeLength { property, value } => write!(
                f,
                "sizing input '{}' must be non-negative finite CSS px, got {}",
                property.as_debug_label(),
                value
            ),
        }
    }
}

impl std::error::Error for StyleSizeInputError {}

fn preferred_size_from_length_percentage_or_auto(
    value: Option<LengthPercentage>,
    property: StyleSizeInputProperty,
) -> Result<StylePreferredSize, StyleSizeInputError> {
    match value {
        Some(LengthPercentage::Length(length)) => {
            non_negative_length_from_css(length, property).map(StylePreferredSize::Length)
        }
        Some(LengthPercentage::Percentage(percentage)) => {
            non_negative_percentage_from_css(percentage, property)
                .map(StylePreferredSize::Percentage)
        }
        None => Ok(StylePreferredSize::Auto),
    }
}

fn minimum_size_from_length_percentage_or_auto(
    value: Option<LengthPercentage>,
    property: StyleSizeInputProperty,
) -> Result<StyleMinimumSize, StyleSizeInputError> {
    match value {
        Some(LengthPercentage::Length(length)) => {
            non_negative_length_from_css(length, property).map(StyleMinimumSize::Length)
        }
        Some(LengthPercentage::Percentage(percentage)) => {
            non_negative_percentage_from_css(percentage, property).map(StyleMinimumSize::Percentage)
        }
        None => Ok(StyleMinimumSize::Auto),
    }
}

fn maximum_size_from_length_percentage_or_none(
    value: Option<LengthPercentage>,
    property: StyleSizeInputProperty,
) -> Result<StyleMaximumSize, StyleSizeInputError> {
    match value {
        Some(LengthPercentage::Length(length)) => {
            non_negative_length_from_css(length, property).map(StyleMaximumSize::Length)
        }
        Some(LengthPercentage::Percentage(percentage)) => {
            non_negative_percentage_from_css(percentage, property).map(StyleMaximumSize::Percentage)
        }
        None => Ok(StyleMaximumSize::None),
    }
}

fn non_negative_length_from_css(
    length: Length,
    property: StyleSizeInputProperty,
) -> Result<CssPx, StyleSizeInputError> {
    match length {
        Length::Px(value) => non_negative_length(value, property),
    }
}

fn non_negative_percentage_from_css(
    percentage: css::Percentage,
    property: StyleSizeInputProperty,
) -> Result<Percentage, StyleSizeInputError> {
    Percentage::from_fraction(percentage.fraction()).ok_or(
        StyleSizeInputError::InvalidNonNegativeLength {
            property,
            value: percentage.fraction(),
        },
    )
}

fn non_negative_length(
    value: f32,
    property: StyleSizeInputProperty,
) -> Result<CssPx, StyleSizeInputError> {
    CssPx::new(value).ok_or(StyleSizeInputError::InvalidNonNegativeLength { property, value })
}

fn signed_length(
    value: f32,
    property: StyleSizeInputProperty,
) -> Result<SignedCssPx, StyleSizeInputError> {
    SignedCssPx::new(value).ok_or(StyleSizeInputError::InvalidFiniteLength { property, value })
}

fn css_px_sum(a: CssPx, b: CssPx) -> CssPx {
    CssPx::new(a.get() + b.get()).expect("sum of finite non-negative CSS px values is valid")
}

fn subtract_css_px(value: CssPx, amount: CssPx) -> CssPx {
    CssPx::new((value.get() - amount.get()).max(0.0))
        .expect("clamped difference of CSS px values is valid")
}

fn optional_containing_block_debug_label(value: Option<ContainingBlockId>) -> String {
    value
        .map(|id| format!("b{}", id.index()))
        .unwrap_or_else(|| "none".to_string())
}

fn available_size_debug_label(value: AvailableSize) -> String {
    match value {
        AvailableSize::Definite(value) => css_px_debug_label(value),
        AvailableSize::Indefinite => "indefinite".to_string(),
    }
}

fn css_px_debug_label(value: CssPx) -> String {
    format!("{:.2}px", value.get())
}

fn signed_css_px_debug_label(value: SignedCssPx) -> String {
    format!("{:.2}px", value.get())
}

fn optional_css_px_debug_label(value: Option<CssPx>) -> String {
    value
        .map(css_px_debug_label)
        .unwrap_or_else(|| "none".to_string())
}

fn optional_aspect_ratio_debug_label(value: Option<AspectRatio>) -> String {
    value
        .map(|ratio| format!("{:.4}", ratio.get()))
        .unwrap_or_else(|| "none".to_string())
}

fn percentage_debug_label(value: Percentage) -> String {
    format!("{:.2}%", value.fraction() * 100.0)
}

fn style_preferred_size_debug_label(value: StylePreferredSize) -> String {
    match value {
        StylePreferredSize::Auto => "auto".to_string(),
        StylePreferredSize::Length(value) => css_px_debug_label(value),
        StylePreferredSize::Percentage(value) => percentage_debug_label(value),
    }
}

fn style_minimum_size_debug_label(value: StyleMinimumSize) -> String {
    match value {
        StyleMinimumSize::Auto => "auto".to_string(),
        StyleMinimumSize::Length(value) => css_px_debug_label(value),
        StyleMinimumSize::Percentage(value) => percentage_debug_label(value),
    }
}

fn style_maximum_size_debug_label(value: StyleMaximumSize) -> String {
    match value {
        StyleMaximumSize::None => "none".to_string(),
        StyleMaximumSize::Length(value) => css_px_debug_label(value),
        StyleMaximumSize::Percentage(value) => percentage_debug_label(value),
    }
}

fn axis_style_input_debug_label(value: AxisStyleSizeInput) -> String {
    format!(
        "preferred={} min={} max={}",
        style_preferred_size_debug_label(value.preferred()),
        style_minimum_size_debug_label(value.min()),
        style_maximum_size_debug_label(value.max()),
    )
}

fn css_px_sides_debug_label(value: PhysicalSides<CssPx>) -> String {
    format!(
        "(top={} right={} bottom={} left={})",
        css_px_debug_label(value.top()),
        css_px_debug_label(value.right()),
        css_px_debug_label(value.bottom()),
        css_px_debug_label(value.left()),
    )
}

fn signed_sides_debug_label(value: PhysicalSides<SignedCssPx>) -> String {
    format!(
        "(top={} right={} bottom={} left={})",
        signed_css_px_debug_label(value.top()),
        signed_css_px_debug_label(value.right()),
        signed_css_px_debug_label(value.bottom()),
        signed_css_px_debug_label(value.left()),
    )
}

pub(crate) fn used_axis_size_debug_label(value: UsedAxisSize) -> String {
    format!(
        "preferred={} reason={} value={} adjustment={}",
        css_px_debug_label(value.preferred_value()),
        value.preferred_reason().as_debug_label(),
        css_px_debug_label(value.value()),
        value.applied_constraint().as_debug_label(),
    )
}

pub(crate) fn used_content_size_debug_label(value: Option<UsedContentSize>) -> String {
    value
        .map(|value| {
            format!(
                "inline({}) block({})",
                used_axis_size_debug_label(value.inline()),
                used_axis_size_debug_label(value.block()),
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::{ComputedStyle, ComputedValue, Length, PropertyId};

    #[test]
    fn css_px_accepts_only_non_negative_finite_values() {
        assert_eq!(CssPx::new(0.0).map(CssPx::get), Some(0.0));
        assert_eq!(CssPx::new(12.5).map(CssPx::get), Some(12.5));
        assert_eq!(CssPx::new(-0.1), None);
        assert_eq!(CssPx::new(f32::INFINITY), None);
        assert_eq!(CssPx::new(f32::NAN), None);
    }

    #[test]
    fn css_px_normalizes_negative_zero() {
        assert_eq!(CssPx::new(-0.0).map(CssPx::get), Some(0.0));
        assert!(
            !CssPx::new(-0.0)
                .expect("negative zero is zero")
                .get()
                .is_sign_negative()
        );
    }

    #[test]
    fn signed_css_px_accepts_negative_lengths_and_normalizes_negative_zero() {
        assert_eq!(SignedCssPx::new(-12.5).map(SignedCssPx::get), Some(-12.5));
        assert_eq!(SignedCssPx::new(-0.0).map(SignedCssPx::get), Some(0.0));
        assert!(
            !SignedCssPx::new(-0.0)
                .expect("negative zero is zero")
                .get()
                .is_sign_negative()
        );
        assert_eq!(SignedCssPx::new(f32::INFINITY), None);
    }

    #[test]
    fn percentages_resolve_only_against_definite_available_size() {
        let half = Percentage::from_percent(50.0).expect("finite percentage");
        let basis = AvailableSize::definite(320.0).expect("finite basis");

        assert_eq!(half.fraction(), 0.5);
        assert_eq!(half.resolve_against(basis), CssPx::new(160.0));
        assert_eq!(half.resolve_against(AvailableSize::Indefinite), None);
    }

    #[test]
    fn aspect_ratio_accepts_only_positive_finite_values() {
        assert_eq!(AspectRatio::new(2.0).map(AspectRatio::get), Some(2.0));
        assert_eq!(AspectRatio::new(0.0), None);
        assert_eq!(AspectRatio::new(-1.0), None);
        assert_eq!(AspectRatio::new(f32::INFINITY), None);
        assert_eq!(AspectRatio::new(f32::NAN), None);
    }

    #[test]
    fn crossed_constraints_resolve_with_minimum_winning() {
        let min = CssPx::new(200.0).expect("finite min");
        let max = CssPx::new(100.0).expect("finite max");
        let value = CssPx::new(150.0).expect("finite value");

        assert_eq!(
            AxisSizeConstraints::new(Some(min), Some(max)).clamp(value),
            min
        );
    }

    #[test]
    fn clamp_reports_applied_constraint() {
        let min = CssPx::new(100.0).expect("finite min");
        let max = CssPx::new(200.0).expect("finite max");
        let constraints = AxisSizeConstraints::new(Some(min), Some(max));

        assert_eq!(
            constraints.clamp_with_applied_constraint(CssPx::new(80.0).expect("finite value")),
            (min, AppliedSizeConstraint::Min)
        );
        assert_eq!(
            constraints.clamp_with_applied_constraint(CssPx::new(240.0).expect("finite value")),
            (max, AppliedSizeConstraint::Max)
        );
        assert_eq!(
            constraints.clamp_with_applied_constraint(CssPx::new(160.0).expect("finite value")),
            (
                CssPx::new(160.0).expect("finite value"),
                AppliedSizeConstraint::None
            )
        );
    }

    #[test]
    fn clamp_reports_available_space_clamp_distinct_from_style_max() {
        let constraints = AxisSizeConstraints::NONE;
        let available = CssPx::new(100.0).expect("finite available clamp");

        assert_eq!(
            constraints.clamp_with_available_space(
                CssPx::new(180.0).expect("finite value"),
                Some(available),
            ),
            (available, AppliedSizeConstraint::AvailableSpaceClamp)
        );
    }

    #[test]
    fn clamp_reports_available_space_when_it_wins_after_style_max() {
        let max = CssPx::new(180.0).expect("finite max");
        let available = CssPx::new(100.0).expect("finite available clamp");
        let constraints = AxisSizeConstraints::new(None, Some(max));

        assert_eq!(
            constraints.clamp_with_available_space(
                CssPx::new(240.0).expect("finite value"),
                Some(available),
            ),
            (available, AppliedSizeConstraint::AvailableSpaceClamp)
        );
    }

    #[test]
    fn clamp_applies_max_then_available_space_then_min() {
        let min = CssPx::new(120.0).expect("finite min");
        let max = CssPx::new(180.0).expect("finite max");
        let available = CssPx::new(100.0).expect("finite available clamp");
        let constraints = AxisSizeConstraints::new(Some(min), Some(max));

        assert_eq!(
            constraints.clamp_with_available_space(
                CssPx::new(240.0).expect("finite value"),
                Some(available),
            ),
            (min, AppliedSizeConstraint::Min)
        );
    }

    #[test]
    fn crossed_constraints_report_min_as_final_constraint() {
        let min = CssPx::new(200.0).expect("finite min");
        let max = CssPx::new(100.0).expect("finite max");
        let value = CssPx::new(150.0).expect("finite value");

        assert_eq!(
            AxisSizeConstraints::new(Some(min), Some(max)).clamp_with_applied_constraint(value),
            (min, AppliedSizeConstraint::Min)
        );
    }

    #[test]
    fn intrinsic_sizes_reject_crossed_min_and_max_content() {
        let min = CssPx::new(20.0).expect("finite min");
        let max = CssPx::new(10.0).expect("finite max");

        assert_eq!(IntrinsicSizes::new(min, max, None, None, None), None);
    }

    #[test]
    fn shrink_to_fit_input_rejects_crossed_intrinsic_bounds() {
        let min = CssPx::new(20.0).expect("finite min");
        let max = CssPx::new(10.0).expect("finite max");

        assert_eq!(
            ShrinkToFitInput::new(min, max, None, AvailableSize::Indefinite),
            None
        );
    }

    #[test]
    fn shrink_to_fit_inline_size_reports_supported_formula_branches() {
        let min = CssPx::new(40.0).expect("min-content");
        let max = CssPx::new(120.0).expect("max-content");

        let below_min = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                min,
                max,
                None,
                AvailableSize::Definite(CssPx::new(20.0).expect("available")),
            )
            .expect("shrink input"),
        );
        assert_eq!(below_min.value(), min);
        assert_eq!(below_min.decision(), ShrinkToFitDecision::MinContentFloor);

        let between = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                min,
                max,
                None,
                AvailableSize::Definite(CssPx::new(80.0).expect("available")),
            )
            .expect("shrink input"),
        );
        assert_eq!(between.value(), CssPx::new(80.0).expect("available"));
        assert_eq!(between.decision(), ShrinkToFitDecision::AvailableSpace);

        let above_max = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                min,
                max,
                None,
                AvailableSize::Definite(CssPx::new(200.0).expect("available")),
            )
            .expect("shrink input"),
        );
        assert_eq!(above_max.value(), max);
        assert_eq!(above_max.decision(), ShrinkToFitDecision::PreferredCeiling);
    }

    #[test]
    fn shrink_to_fit_definite_space_uses_clamped_preferred_ceiling() {
        let result = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(90.0).expect("preferred")),
                AvailableSize::Definite(CssPx::new(200.0).expect("available")),
            )
            .expect("shrink input"),
        );

        assert_eq!(result.value(), CssPx::new(90.0).expect("preferred"));
        assert_eq!(result.decision(), ShrinkToFitDecision::PreferredCeiling);
    }

    #[test]
    fn shrink_to_fit_preferred_ceiling_is_clamped_to_intrinsic_bounds() {
        let below_min = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(20.0).expect("preferred below min")),
                AvailableSize::Indefinite,
            )
            .expect("shrink input"),
        );
        assert_eq!(below_min.value(), CssPx::new(40.0).expect("min-content"));
        assert_eq!(
            below_min.decision(),
            ShrinkToFitDecision::IndefiniteAvailableSpace
        );

        let above_max = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(200.0).expect("preferred above max")),
                AvailableSize::Indefinite,
            )
            .expect("shrink input"),
        );
        assert_eq!(above_max.value(), CssPx::new(120.0).expect("max-content"));
        assert_eq!(
            above_max.decision(),
            ShrinkToFitDecision::IndefiniteAvailableSpace
        );
    }

    #[test]
    fn shrink_to_fit_indefinite_space_uses_intrinsic_preferred_size() {
        let result = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(90.0).expect("preferred")),
                AvailableSize::Indefinite,
            )
            .expect("shrink input"),
        );

        assert_eq!(result.value(), CssPx::new(90.0).expect("preferred"));
        assert_eq!(
            result.decision(),
            ShrinkToFitDecision::IndefiniteAvailableSpace
        );
    }

    #[test]
    fn shrink_to_fit_empty_intrinsic_contribution_is_explicit() {
        let result = resolve_shrink_to_fit_inline_size(
            ShrinkToFitInput::new(
                CssPx::ZERO,
                CssPx::ZERO,
                None,
                AvailableSize::Definite(CssPx::new(100.0).expect("available")),
            )
            .expect("shrink input"),
        );

        assert_eq!(result.value(), CssPx::ZERO);
        assert_eq!(
            result.decision(),
            ShrinkToFitDecision::EmptyIntrinsicContribution
        );
    }

    #[test]
    fn used_axis_size_preserves_preferred_reason_when_constrained() {
        let preferred = CssPx::new(80.0).expect("finite preferred");
        let final_value = CssPx::new(120.0).expect("finite final");
        let used = UsedAxisSize::constrained(
            preferred,
            SizeResolutionReason::PercentageOfDefiniteContainingBlock,
            final_value,
            AppliedSizeConstraint::Min,
        );

        assert_eq!(used.preferred_value(), preferred);
        assert_eq!(
            used.preferred_reason(),
            SizeResolutionReason::PercentageOfDefiniteContainingBlock
        );
        assert_eq!(used.value(), final_value);
        assert_eq!(used.applied_constraint(), AppliedSizeConstraint::Min);
    }

    #[test]
    fn size_resolution_debug_snapshot_is_stable_and_semantic() {
        let containing_size = ContainingSize::new(
            None,
            AvailableSize::definite(400.0).expect("containing inline"),
            AvailableSize::Indefinite,
        );
        let available_space = AvailableSpace::new(
            AvailableSize::definite(160.0).expect("available inline"),
            AvailableSize::Indefinite,
        );
        let constraint_space = ConstraintSpace::from_containing_size(containing_size)
            .with_available_space(available_space);
        let style = StyleSizeInputs::new(
            AxisStyleSizeInput::new(
                StylePreferredSize::Auto,
                StyleMinimumSize::Length(CssPx::new(50.0).expect("min-width")),
                StyleMaximumSize::Percentage(
                    Percentage::from_percent(25.0).expect("max-width percentage"),
                ),
            ),
            AxisStyleSizeInput::new(
                StylePreferredSize::Auto,
                StyleMinimumSize::Auto,
                StyleMaximumSize::None,
            ),
            StyleBoxMetrics::new(
                PhysicalSides::new(
                    SignedCssPx::new(1.0).expect("margin top"),
                    SignedCssPx::new(2.0).expect("margin right"),
                    SignedCssPx::new(3.0).expect("margin bottom"),
                    SignedCssPx::new(4.0).expect("margin left"),
                ),
                PhysicalSides::new(
                    CssPx::new(2.0).expect("padding top"),
                    CssPx::new(10.0).expect("padding right"),
                    CssPx::new(3.0).expect("padding bottom"),
                    CssPx::new(10.0).expect("padding left"),
                ),
            ),
        );
        let intrinsic = IntrinsicSizes::new(
            CssPx::new(40.0).expect("min-content"),
            CssPx::new(220.0).expect("max-content"),
            Some(CssPx::new(220.0).expect("preferred")),
            None,
            None,
        )
        .expect("intrinsic");
        let input = SizeResolutionInput::new(constraint_space, style, intrinsic);

        let snapshot = input.to_debug_snapshot(
            NormalFlowSizingMode::AtomicInline,
            CssPx::new(30.0).expect("auto content block size"),
        );

        assert_eq!(
            snapshot,
            "version: 1\n\
size-resolution\n\
mode: atomic-inline\n\
auto-content-block-size: 30.00px\n\
constraint-space: containing-block=none containing-inline=400.00px containing-block-size=indefinite available-inline=160.00px available-block=indefinite\n\
style: inline(preferred=auto min=50.00px max=25.00%) block(preferred=auto min=auto max=none)\n\
box-metrics: margin=(top=1.00px right=2.00px bottom=3.00px left=4.00px) padding=(top=2.00px right=10.00px bottom=3.00px left=10.00px)\n\
intrinsic: min-content-inline=40.00px max-content-inline=220.00px preferred-inline=220.00px preferred-block=none aspect-ratio=none\n\
result-inline: preferred=140.00px reason=shrink-to-fit value=100.00px adjustment=max border=120.00px\n\
result-block: preferred=30.00px reason=auto-content-based value=30.00px adjustment=none border=35.00px\n"
        );
        assert_eq!(
            snapshot,
            input.to_debug_snapshot(
                NormalFlowSizingMode::AtomicInline,
                CssPx::new(30.0).expect("auto content block size"),
            )
        );
    }

    #[test]
    fn constraint_space_exposes_logical_axis_inputs() {
        let inline = AvailableSize::definite(320.0).expect("finite inline size");
        let space = ConstraintSpace::new(None, inline, AvailableSize::Indefinite);

        assert_eq!(space.available_size(SizeAxis::Inline), inline);
        assert_eq!(
            space.available_size(SizeAxis::Block),
            AvailableSize::Indefinite
        );
    }

    #[test]
    fn constraint_space_separates_containing_size_from_available_space() {
        let containing_inline = AvailableSize::definite(500.0).expect("finite containing inline");
        let available_inline = AvailableSize::definite(320.0).expect("finite available inline");
        let containing_size =
            ContainingSize::new(None, containing_inline, AvailableSize::Indefinite);
        let available_space = AvailableSpace::new(available_inline, AvailableSize::Indefinite);
        let space = ConstraintSpace::from_containing_size(containing_size)
            .with_available_space(available_space);

        assert_eq!(
            space.containing_size_for_axis(SizeAxis::Inline),
            containing_inline
        );
        assert_eq!(space.available_size(SizeAxis::Inline), available_inline);
        assert_eq!(space.containing_size().containing_block(), None);
    }

    #[test]
    fn constraint_space_available_size_after_edges_uses_available_space() {
        let containing_inline = AvailableSize::definite(500.0).expect("finite containing inline");
        let available_inline = AvailableSize::definite(120.0).expect("finite available inline");
        let containing_size =
            ContainingSize::new(None, containing_inline, AvailableSize::Indefinite);
        let available_space = AvailableSpace::new(available_inline, AvailableSize::Indefinite);
        let space = ConstraintSpace::from_containing_size(containing_size)
            .with_available_space(available_space);

        assert_eq!(
            space.available_size_after_edges(
                SizeAxis::Inline,
                CssPx::new(30.0).expect("start edge"),
                CssPx::new(100.0).expect("end edge"),
            ),
            AvailableSize::Definite(CssPx::ZERO)
        );
        assert_eq!(
            space.containing_size_for_axis(SizeAxis::Inline),
            containing_inline
        );
    }

    #[test]
    fn constraint_space_available_size_after_edges_preserves_indefinite_space() {
        let space =
            ConstraintSpace::new(None, AvailableSize::Indefinite, AvailableSize::Indefinite);

        assert_eq!(
            space.available_size_after_edges(
                SizeAxis::Inline,
                CssPx::new(12.0).expect("start edge"),
                CssPx::new(8.0).expect("end edge"),
            ),
            AvailableSize::Indefinite
        );
    }

    #[test]
    fn style_box_metrics_validate_signed_margins_and_non_negative_padding() {
        let metrics = BoxMetrics {
            margin_top: -4.0,
            margin_right: 1.0,
            margin_bottom: -0.0,
            margin_left: 2.0,
            padding_top: 3.0,
            padding_right: 4.0,
            padding_bottom: 5.0,
            padding_left: 6.0,
        };

        let inputs = StyleBoxMetrics::from_box_metrics(metrics).expect("valid box metrics");

        assert_eq!(inputs.margin().top().get(), -4.0);
        assert_eq!(inputs.margin().bottom().get(), 0.0);
        assert_eq!(
            inputs.padding().left(),
            CssPx::new(6.0).expect("finite padding")
        );

        let invalid = StyleBoxMetrics::from_box_metrics(BoxMetrics {
            padding_left: -1.0,
            ..metrics
        });
        assert_eq!(
            invalid,
            Err(StyleSizeInputError::InvalidNonNegativeLength {
                property: StyleSizeInputProperty::PaddingLeft,
                value: -1.0,
            })
        );
    }

    #[test]
    fn style_box_metrics_reject_non_finite_margins() {
        let metrics = BoxMetrics {
            margin_top: f32::INFINITY,
            margin_right: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            padding_top: 0.0,
            padding_right: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
        };

        assert_eq!(
            StyleBoxMetrics::from_box_metrics(metrics),
            Err(StyleSizeInputError::InvalidFiniteLength {
                property: StyleSizeInputProperty::MarginTop,
                value: f32::INFINITY,
            })
        );
    }

    #[test]
    fn style_size_inputs_materialize_current_computed_sizing_properties() {
        let style = ComputedStyle::initial()
            .with_property(
                PropertyId::Width,
                computed_length_percentage_or_auto_px(120.0),
            )
            .expect("width")
            .with_property(
                PropertyId::Height,
                computed_length_percentage_or_auto_px(40.0),
            )
            .expect("height")
            .with_property(
                PropertyId::MinWidth,
                computed_length_percentage_or_auto_px(80.0),
            )
            .expect("min-width")
            .with_property(
                PropertyId::MaxWidth,
                computed_length_percentage_or_none_px(240.0),
            )
            .expect("max-width")
            .with_property(
                PropertyId::MarginLeft,
                ComputedValue::Length(Length::Px(-8.0)),
            )
            .expect("margin-left")
            .with_property(
                PropertyId::PaddingRight,
                ComputedValue::Length(Length::Px(12.0)),
            )
            .expect("padding-right");

        let inputs = StyleSizeInputs::from_computed_style(&style).expect("style inputs");

        assert_eq!(
            inputs.inline().preferred(),
            StylePreferredSize::Length(CssPx::new(120.0).expect("finite width"))
        );
        assert_eq!(
            inputs.inline().min(),
            StyleMinimumSize::Length(CssPx::new(80.0).expect("finite min-width"))
        );
        assert_eq!(
            inputs.inline().max(),
            StyleMaximumSize::Length(CssPx::new(240.0).expect("finite max-width"))
        );
        assert_eq!(
            inputs.block().preferred(),
            StylePreferredSize::Length(CssPx::new(40.0).expect("finite height"))
        );
        assert_eq!(inputs.block().min(), StyleMinimumSize::Auto);
        assert_eq!(inputs.block().max(), StyleMaximumSize::None);
        assert_eq!(inputs.box_metrics().margin().left().get(), -8.0);
        assert_eq!(
            inputs.box_metrics().padding().right(),
            CssPx::new(12.0).expect("finite padding")
        );
    }

    #[test]
    fn style_size_inputs_materialize_percentage_sizing_properties() {
        let style = ComputedStyle::initial()
            .with_property(
                PropertyId::Width,
                ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
                    css::Percentage::from_percent(50.0).expect("finite width percentage"),
                ))),
            )
            .expect("width")
            .with_property(
                PropertyId::Height,
                ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
                    css::Percentage::from_percent(25.0).expect("finite height percentage"),
                ))),
            )
            .expect("height")
            .with_property(
                PropertyId::MinWidth,
                ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
                    css::Percentage::from_percent(40.0).expect("finite min-width percentage"),
                ))),
            )
            .expect("min-width")
            .with_property(
                PropertyId::MaxWidth,
                ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Percentage(
                    css::Percentage::from_percent(75.0).expect("finite max-width percentage"),
                ))),
            )
            .expect("max-width");

        let inputs = StyleSizeInputs::from_computed_style(&style).expect("style inputs");

        assert_eq!(
            inputs.inline().preferred(),
            StylePreferredSize::Percentage(Percentage::from_percent(50.0).expect("width"))
        );
        assert_eq!(
            inputs.block().preferred(),
            StylePreferredSize::Percentage(Percentage::from_percent(25.0).expect("height"))
        );
        assert_eq!(
            inputs.inline().min(),
            StyleMinimumSize::Percentage(Percentage::from_percent(40.0).expect("min-width"))
        );
        assert_eq!(
            inputs.inline().max(),
            StyleMaximumSize::Percentage(Percentage::from_percent(75.0).expect("max-width"))
        );
    }

    #[test]
    fn style_size_inputs_reject_invalid_non_negative_sizing_lengths() {
        let style = ComputedStyle::initial()
            .with_property(
                PropertyId::Width,
                computed_length_percentage_or_auto_px(-1.0),
            )
            .expect("width");

        assert_eq!(
            StyleSizeInputs::from_computed_style(&style),
            Err(StyleSizeInputError::InvalidNonNegativeLength {
                property: StyleSizeInputProperty::Width,
                value: -1.0,
            })
        );
    }

    #[test]
    fn size_resolution_input_preserves_constraint_style_and_intrinsic_inputs() {
        let constraint_space = ConstraintSpace::new(
            None,
            AvailableSize::definite(640.0).expect("finite inline"),
            AvailableSize::Indefinite,
        );
        let style = StyleSizeInputs::from_computed_style(&ComputedStyle::initial())
            .expect("initial style inputs");
        let intrinsic = IntrinsicSizes::new(
            CssPx::new(20.0).expect("finite min-content"),
            CssPx::new(80.0).expect("finite max-content"),
            Some(CssPx::new(60.0).expect("finite preferred width")),
            None,
            None,
        )
        .expect("valid intrinsic sizes");

        let input = SizeResolutionInput::new(constraint_space, style, intrinsic);

        assert_eq!(input.constraint_space(), constraint_space);
        assert_eq!(input.style(), style);
        assert_eq!(input.intrinsic(), intrinsic);
    }

    #[test]
    fn normal_flow_auto_inline_size_fills_available_border_box_after_padding() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::PaddingLeft,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-left")
                .with_property(
                    PropertyId::PaddingRight,
                    ComputedValue::Length(Length::Px(15.0)),
                )
                .expect("padding-right"),
            200.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(
            resolved.content().value(),
            CssPx::new(175.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::AutoStretchToContainingBlock
        );
        assert_eq!(resolved.border(), CssPx::new(200.0).expect("border"));
    }

    #[test]
    fn normal_flow_explicit_width_is_content_box_and_padding_expands_border_box() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(100.0),
                )
                .expect("width")
                .with_property(
                    PropertyId::PaddingLeft,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-left")
                .with_property(
                    PropertyId::PaddingRight,
                    ComputedValue::Length(Length::Px(15.0)),
                )
                .expect("padding-right"),
            500.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(
            resolved.content().value(),
            CssPx::new(100.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::DefiniteLength
        );
        assert_eq!(resolved.border(), CssPx::new(125.0).expect("border"));
    }

    #[test]
    fn normal_flow_percentage_width_resolves_against_containing_size_not_available_space() {
        let input = size_input_with_containing_and_available_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
                        css::Percentage::from_percent(50.0).expect("finite width percentage"),
                    ))),
                )
                .expect("width")
                .with_property(
                    PropertyId::PaddingLeft,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-left")
                .with_property(
                    PropertyId::PaddingRight,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-right"),
            AvailableSize::definite(400.0).expect("containing inline"),
            AvailableSize::definite(300.0).expect("available inline"),
            AvailableSize::Indefinite,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(
            resolved.content().value(),
            CssPx::new(200.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::PercentageOfDefiniteContainingBlock
        );
        assert_eq!(resolved.border(), CssPx::new(220.0).expect("border"));
    }

    #[test]
    fn normal_flow_percentage_width_defers_against_indefinite_containing_size() {
        let input = size_input_with_available_inline_size(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
                        css::Percentage::from_percent(50.0).expect("finite width percentage"),
                    ))),
                )
                .expect("width"),
            AvailableSize::Indefinite,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(resolved.content().value(), CssPx::ZERO);
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::DeferredIndefinitePercentage
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::None
        );
    }

    #[test]
    fn normal_flow_min_max_constraints_apply_to_content_box_before_padding() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(200.0),
                )
                .expect("width")
                .with_property(
                    PropertyId::MaxWidth,
                    computed_length_percentage_or_none_px(120.0),
                )
                .expect("max-width")
                .with_property(
                    PropertyId::PaddingLeft,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-left")
                .with_property(
                    PropertyId::PaddingRight,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-right"),
            500.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(200.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(120.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Max
        );
        assert_eq!(resolved.border(), CssPx::new(140.0).expect("border"));
    }

    #[test]
    fn normal_flow_percentage_width_constraints_apply_after_explicit_width() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(300.0),
                )
                .expect("width")
                .with_property(
                    PropertyId::MaxWidth,
                    ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Percentage(
                        css::Percentage::from_percent(50.0).expect("finite max-width percentage"),
                    ))),
                )
                .expect("max-width"),
            400.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(300.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(200.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Max
        );
    }

    #[test]
    fn normal_flow_min_width_applies_to_content_box_before_padding() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(80.0),
                )
                .expect("width")
                .with_property(
                    PropertyId::MinWidth,
                    computed_length_percentage_or_auto_px(120.0),
                )
                .expect("min-width")
                .with_property(
                    PropertyId::PaddingLeft,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-left")
                .with_property(
                    PropertyId::PaddingRight,
                    ComputedValue::Length(Length::Px(10.0)),
                )
                .expect("padding-right"),
            500.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(80.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(120.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Min
        );
        assert_eq!(resolved.border(), CssPx::new(140.0).expect("border"));
    }

    #[test]
    fn normal_flow_crossed_min_max_width_constraints_resolve_with_minimum_winning() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(150.0),
                )
                .expect("width")
                .with_property(
                    PropertyId::MinWidth,
                    computed_length_percentage_or_auto_px(200.0),
                )
                .expect("min-width")
                .with_property(
                    PropertyId::MaxWidth,
                    computed_length_percentage_or_none_px(100.0),
                )
                .expect("max-width"),
            500.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::BlockLevel);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(150.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(200.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Min
        );
    }

    #[test]
    fn atomic_inline_available_space_clamp_is_not_reported_as_style_max() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(200.0),
                )
                .expect("width"),
            100.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(200.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(100.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::AvailableSpaceClamp
        );
        assert_eq!(resolved.border(), CssPx::new(100.0).expect("border"));
    }

    #[test]
    fn atomic_inline_min_width_wins_over_available_space_clamp() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(200.0),
                )
                .expect("width")
                .with_property(
                    PropertyId::MinWidth,
                    computed_length_percentage_or_auto_px(150.0),
                )
                .expect("min-width"),
            100.0,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(200.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(150.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Min
        );
    }

    #[test]
    fn atomic_inline_width_is_not_clamped_when_available_inline_size_is_indefinite() {
        let input = size_input_with_available_inline_size(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(100.0),
                )
                .expect("width"),
            AvailableSize::Indefinite,
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().value(),
            CssPx::new(100.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::None
        );
        assert_eq!(resolved.border(), CssPx::new(100.0).expect("border"));
    }

    #[test]
    fn atomic_inline_auto_width_uses_intrinsic_max_content_when_available_is_larger() {
        let input = size_input_with_intrinsic(
            ComputedStyle::initial(),
            500.0,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(120.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().value(),
            CssPx::new(120.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::ShrinkToFit
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::None
        );
        assert_eq!(resolved.border(), CssPx::new(120.0).expect("border"));
    }

    #[test]
    fn atomic_inline_auto_width_shrink_to_fit_uses_available_between_min_and_max_content() {
        let input = size_input_with_intrinsic(
            ComputedStyle::initial(),
            80.0,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(120.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(80.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::ShrinkToFit
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::None
        );
        assert_eq!(resolved.border(), CssPx::new(80.0).expect("border"));
    }

    #[test]
    fn atomic_inline_auto_width_shrink_to_fit_uses_min_content_when_available_is_smaller() {
        let input = size_input_with_intrinsic(
            ComputedStyle::initial(),
            20.0,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(120.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(40.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::ShrinkToFit
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::None
        );
        assert_eq!(resolved.border(), CssPx::new(40.0).expect("border"));
    }

    #[test]
    fn atomic_inline_auto_width_applies_min_width_after_intrinsic_shrink_to_fit() {
        let input = size_input_with_intrinsic(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::MinWidth,
                    computed_length_percentage_or_auto_px(100.0),
                )
                .expect("min-width"),
            80.0,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(120.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(80.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(100.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Min
        );
    }

    #[test]
    fn atomic_inline_auto_width_percentage_max_uses_containing_size_not_available_space() {
        let input = size_input_with_containing_available_intrinsic_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::MaxWidth,
                    ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Percentage(
                        css::Percentage::from_percent(25.0).expect("finite max-width percentage"),
                    ))),
                )
                .expect("max-width"),
            AvailableSize::definite(400.0).expect("containing inline"),
            AvailableSize::definite(80.0).expect("available inline"),
            AvailableSize::Indefinite,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(180.0).expect("max-content"),
                Some(CssPx::new(180.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(80.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::ShrinkToFit
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(80.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::None
        );
    }

    #[test]
    fn atomic_inline_auto_width_percentage_max_constrains_after_shrink_to_fit() {
        let input = size_input_with_containing_available_intrinsic_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::MaxWidth,
                    ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Percentage(
                        css::Percentage::from_percent(25.0).expect("finite max-width percentage"),
                    ))),
                )
                .expect("max-width"),
            AvailableSize::definite(400.0).expect("containing inline"),
            AvailableSize::definite(160.0).expect("available inline"),
            AvailableSize::Indefinite,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(220.0).expect("max-content"),
                Some(CssPx::new(220.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(160.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::ShrinkToFit
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(100.0).expect("percentage max")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Max
        );
    }

    #[test]
    fn atomic_inline_auto_width_applies_max_width_after_intrinsic_preferred_size() {
        let input = size_input_with_intrinsic(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::MaxWidth,
                    computed_length_percentage_or_none_px(90.0),
                )
                .expect("max-width"),
            500.0,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(120.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().preferred_value(),
            CssPx::new(120.0).expect("preferred")
        );
        assert_eq!(
            resolved.content().value(),
            CssPx::new(90.0).expect("content")
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::Max
        );
    }

    #[test]
    fn atomic_inline_auto_width_uses_intrinsic_preferred_when_available_is_indefinite() {
        let input = size_input_with_available_inline_and_intrinsic(
            ComputedStyle::initial(),
            AvailableSize::Indefinite,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(120.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().value(),
            CssPx::new(120.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::IntrinsicPreferredSize
        );
        assert_eq!(
            resolved.content().applied_constraint(),
            AppliedSizeConstraint::None
        );
        assert_eq!(resolved.border(), CssPx::new(120.0).expect("border"));
    }

    #[test]
    fn atomic_inline_explicit_width_still_overrides_intrinsic_width() {
        let input = size_input_with_intrinsic(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    computed_length_percentage_or_auto_px(90.0),
                )
                .expect("width"),
            500.0,
            IntrinsicSizes::new(
                CssPx::new(40.0).expect("min-content"),
                CssPx::new(120.0).expect("max-content"),
                Some(CssPx::new(120.0).expect("preferred")),
                None,
                None,
            )
            .expect("intrinsic"),
        );

        let resolved = resolve_normal_flow_inline_size(input, NormalFlowSizingMode::AtomicInline);

        assert_eq!(
            resolved.content().value(),
            CssPx::new(90.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::DefiniteLength
        );
    }

    #[test]
    fn normal_flow_explicit_height_is_content_box_and_padding_expands_border_box() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Height,
                    computed_length_percentage_or_auto_px(40.0),
                )
                .expect("height")
                .with_property(
                    PropertyId::PaddingTop,
                    ComputedValue::Length(Length::Px(5.0)),
                )
                .expect("padding-top")
                .with_property(
                    PropertyId::PaddingBottom,
                    ComputedValue::Length(Length::Px(7.0)),
                )
                .expect("padding-bottom"),
            500.0,
        );

        let resolved = resolve_normal_flow_block_size(
            input,
            NormalFlowSizingMode::BlockLevel,
            CssPx::new(120.0).expect("auto content"),
        );

        assert_eq!(
            resolved.content().value(),
            CssPx::new(40.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::DefiniteLength
        );
        assert_eq!(resolved.border(), CssPx::new(52.0).expect("border"));
    }

    #[test]
    fn normal_flow_percentage_height_resolves_against_definite_containing_block_size() {
        let input = size_input_with_containing_and_available_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Height,
                    ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
                        css::Percentage::from_percent(50.0).expect("finite height percentage"),
                    ))),
                )
                .expect("height")
                .with_property(
                    PropertyId::PaddingTop,
                    ComputedValue::Length(Length::Px(5.0)),
                )
                .expect("padding-top")
                .with_property(
                    PropertyId::PaddingBottom,
                    ComputedValue::Length(Length::Px(5.0)),
                )
                .expect("padding-bottom"),
            AvailableSize::definite(500.0).expect("containing inline"),
            AvailableSize::definite(500.0).expect("available inline"),
            AvailableSize::definite(300.0).expect("containing block"),
        );

        let resolved = resolve_normal_flow_block_size(
            input,
            NormalFlowSizingMode::BlockLevel,
            CssPx::new(40.0).expect("auto content"),
        );

        assert_eq!(
            resolved.content().value(),
            CssPx::new(150.0).expect("content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::PercentageOfDefiniteContainingBlock
        );
        assert_eq!(resolved.border(), CssPx::new(160.0).expect("border"));
    }

    #[test]
    fn normal_flow_percentage_height_defers_against_indefinite_containing_block_size() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Height,
                    ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
                        css::Percentage::from_percent(50.0).expect("finite height percentage"),
                    ))),
                )
                .expect("height"),
            500.0,
        );

        let resolved = resolve_normal_flow_block_size(
            input,
            NormalFlowSizingMode::BlockLevel,
            CssPx::new(40.0).expect("auto content"),
        );

        assert_eq!(
            resolved.content().value(),
            CssPx::new(40.0).expect("auto content")
        );
        assert_eq!(
            resolved.content().preferred_reason(),
            SizeResolutionReason::DeferredIndefinitePercentage
        );
    }

    #[test]
    fn normal_flow_block_axis_constraints_apply_to_content_box_before_padding() {
        let axis = AxisStyleSizeInput::new(
            StylePreferredSize::Auto,
            StyleMinimumSize::Auto,
            StyleMaximumSize::None,
        );
        let block = AxisStyleSizeInput::new(
            StylePreferredSize::Auto,
            StyleMinimumSize::Length(CssPx::new(100.0).expect("min block")),
            StyleMaximumSize::Length(CssPx::new(150.0).expect("max block")),
        );
        let style = StyleSizeInputs::new(axis, block, StyleBoxMetrics::zero());
        let input = size_input_with_style_inputs(style, 500.0);

        let min_resolved = resolve_normal_flow_block_size(
            input,
            NormalFlowSizingMode::BlockLevel,
            CssPx::new(80.0).expect("auto content"),
        );
        assert_eq!(
            min_resolved.content().value(),
            CssPx::new(100.0).expect("min content")
        );
        assert_eq!(
            min_resolved.content().applied_constraint(),
            AppliedSizeConstraint::Min
        );

        let max_resolved = resolve_normal_flow_block_size(
            input,
            NormalFlowSizingMode::BlockLevel,
            CssPx::new(180.0).expect("auto content"),
        );
        assert_eq!(
            max_resolved.content().value(),
            CssPx::new(150.0).expect("max content")
        );
        assert_eq!(
            max_resolved.content().applied_constraint(),
            AppliedSizeConstraint::Max
        );
    }

    fn size_input_with_style(
        style: ComputedStyle,
        available_inline_size: f32,
    ) -> SizeResolutionInput {
        size_input_with_intrinsic(style, available_inline_size, IntrinsicSizes::zero())
    }

    fn size_input_with_intrinsic(
        style: ComputedStyle,
        available_inline_size: f32,
        intrinsic: IntrinsicSizes,
    ) -> SizeResolutionInput {
        size_input_with_available_inline_and_intrinsic(
            style,
            AvailableSize::definite(available_inline_size).expect("available inline size"),
            intrinsic,
        )
    }

    fn size_input_with_available_inline_size(
        style: ComputedStyle,
        available_inline_size: AvailableSize,
    ) -> SizeResolutionInput {
        size_input_with_available_inline_and_intrinsic(
            style,
            available_inline_size,
            IntrinsicSizes::zero(),
        )
    }

    fn size_input_with_available_inline_and_intrinsic(
        style: ComputedStyle,
        available_inline_size: AvailableSize,
        intrinsic: IntrinsicSizes,
    ) -> SizeResolutionInput {
        let constraint_space =
            ConstraintSpace::new(None, available_inline_size, AvailableSize::Indefinite);
        let style = StyleSizeInputs::from_computed_style(&style).expect("style inputs");
        SizeResolutionInput::new(constraint_space, style, intrinsic)
    }

    fn size_input_with_containing_and_available_style(
        style: ComputedStyle,
        containing_inline_size: AvailableSize,
        available_inline_size: AvailableSize,
        containing_block_size: AvailableSize,
    ) -> SizeResolutionInput {
        size_input_with_containing_available_intrinsic_style(
            style,
            containing_inline_size,
            available_inline_size,
            containing_block_size,
            IntrinsicSizes::zero(),
        )
    }

    fn size_input_with_containing_available_intrinsic_style(
        style: ComputedStyle,
        containing_inline_size: AvailableSize,
        available_inline_size: AvailableSize,
        containing_block_size: AvailableSize,
        intrinsic: IntrinsicSizes,
    ) -> SizeResolutionInput {
        let containing_size =
            ContainingSize::new(None, containing_inline_size, containing_block_size);
        let available_space = AvailableSpace::new(available_inline_size, containing_block_size);
        let constraint_space = ConstraintSpace::from_containing_size(containing_size)
            .with_available_space(available_space);
        let style = StyleSizeInputs::from_computed_style(&style).expect("style inputs");
        SizeResolutionInput::new(constraint_space, style, intrinsic)
    }

    fn size_input_with_style_inputs(
        style: StyleSizeInputs,
        available_inline_size: f32,
    ) -> SizeResolutionInput {
        let constraint_space = ConstraintSpace::new(
            None,
            AvailableSize::definite(available_inline_size).expect("available inline size"),
            AvailableSize::Indefinite,
        );
        SizeResolutionInput::new(constraint_space, style, IntrinsicSizes::zero())
    }

    fn computed_length_percentage_or_auto_px(value: f32) -> ComputedValue {
        ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Length(Length::Px(value))))
    }

    fn computed_length_percentage_or_none_px(value: f32) -> ComputedValue {
        ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Length(Length::Px(value))))
    }
}
