//! Sizing contract types for Milestone X.
//!
//! This module defines the shared vocabulary future sizing resolvers must use.
//! It deliberately does not replace the current geometry pass yet; X1 is the
//! architecture boundary that later X issues will implement against.

use crate::ContainingBlockId;
use css::{BoxMetrics, ComputedStyle, Length};

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
/// `1.0` is 100%, `0.5` is 50%. Percentages are not currently produced by the
/// CSS computed-value layer, but size input types can already represent them.
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
            preferred_size_from_length_or_auto(style.width(), StyleSizeInputProperty::Width)?,
            minimum_size_from_length_or_auto(style.min_width(), StyleSizeInputProperty::MinWidth)?,
            maximum_size_from_length_or_none(style.max_width(), StyleSizeInputProperty::MaxWidth)?,
        );
        let block = AxisStyleSizeInput::new(
            preferred_size_from_length_or_auto(style.height(), StyleSizeInputProperty::Height)?,
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
}

/// Supported normal-flow sizing behavior for the current layout subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalFlowSizingMode {
    Document,
    BlockLevel,
    InlineLevel,
    AtomicInline,
    Anonymous,
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
    let available_inline = input
        .constraint_space()
        .available_size(SizeAxis::Inline)
        .definite_value();
    let padding = style.box_metrics().padding();
    let padding_inline = css_px_sum(padding.left(), padding.right());
    let available_content =
        available_inline.map(|available_inline| subtract_css_px(available_inline, padding_inline));
    let basis = input
        .constraint_space()
        .containing_size_for_axis(SizeAxis::Inline);
    let axis = style.inline();

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
        (_, StylePreferredSize::Auto) => auto_stretch_preferred(available_content),
    };

    let constraints = inline_constraints(axis, basis);
    let (value, applied_constraint) =
        apply_inline_size_adjustments(preferred_value, constraints, mode, available_content);
    let used = used_axis_size(preferred_value, preferred_reason, value, applied_constraint);
    let border = css_px_sum(value, padding_inline);

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
                CssPx::ZERO,
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

fn apply_inline_size_adjustments(
    preferred_value: CssPx,
    constraints: AxisSizeConstraints,
    mode: NormalFlowSizingMode,
    available_content: Option<CssPx>,
) -> (CssPx, AppliedSizeConstraint) {
    let mut out = preferred_value;
    let mut applied = AppliedSizeConstraint::None;

    if let Some(max) = constraints.max().filter(|max| out > *max) {
        out = max;
        applied = AppliedSizeConstraint::Max;
    }
    if matches!(mode, NormalFlowSizingMode::AtomicInline) {
        if let Some(available_content) = available_content.filter(|available| out > *available) {
            out = available_content;
            applied = AppliedSizeConstraint::AvailableSpaceClamp;
        }
    }
    if let Some(min) = constraints.min().filter(|min| out < *min) {
        out = min;
        applied = AppliedSizeConstraint::Min;
    }

    (out, applied)
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
        let mut out = value;
        let mut applied = AppliedSizeConstraint::None;
        if let Some(max) = self.max.filter(|max| out > *max) {
            out = max;
            applied = AppliedSizeConstraint::Max;
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
    DeferredIndefinitePercentage,
    UnsupportedDeferred,
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

fn preferred_size_from_length_or_auto(
    value: Option<Length>,
    property: StyleSizeInputProperty,
) -> Result<StylePreferredSize, StyleSizeInputError> {
    match value {
        Some(length) => {
            non_negative_length_from_css(length, property).map(StylePreferredSize::Length)
        }
        None => Ok(StylePreferredSize::Auto),
    }
}

fn minimum_size_from_length_or_auto(
    value: Option<Length>,
    property: StyleSizeInputProperty,
) -> Result<StyleMinimumSize, StyleSizeInputError> {
    match value {
        Some(length) => {
            non_negative_length_from_css(length, property).map(StyleMinimumSize::Length)
        }
        None => Ok(StyleMinimumSize::Auto),
    }
}

fn maximum_size_from_length_or_none(
    value: Option<Length>,
    property: StyleSizeInputProperty,
) -> Result<StyleMaximumSize, StyleSizeInputError> {
    match value {
        Some(length) => {
            non_negative_length_from_css(length, property).map(StyleMaximumSize::Length)
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
                ComputedValue::LengthOrAuto(Some(Length::Px(120.0))),
            )
            .expect("width")
            .with_property(
                PropertyId::Height,
                ComputedValue::LengthOrAuto(Some(Length::Px(40.0))),
            )
            .expect("height")
            .with_property(
                PropertyId::MinWidth,
                ComputedValue::LengthOrAuto(Some(Length::Px(80.0))),
            )
            .expect("min-width")
            .with_property(
                PropertyId::MaxWidth,
                ComputedValue::LengthOrNone(Some(Length::Px(240.0))),
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
    fn style_size_inputs_reject_invalid_non_negative_sizing_lengths() {
        let style = ComputedStyle::initial()
            .with_property(
                PropertyId::Width,
                ComputedValue::LengthOrAuto(Some(Length::Px(-1.0))),
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
                    ComputedValue::LengthOrAuto(Some(Length::Px(100.0))),
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
    fn normal_flow_min_max_constraints_apply_to_content_box_before_padding() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    ComputedValue::LengthOrAuto(Some(Length::Px(200.0))),
                )
                .expect("width")
                .with_property(
                    PropertyId::MaxWidth,
                    ComputedValue::LengthOrNone(Some(Length::Px(120.0))),
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
    fn normal_flow_min_width_applies_to_content_box_before_padding() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    ComputedValue::LengthOrAuto(Some(Length::Px(80.0))),
                )
                .expect("width")
                .with_property(
                    PropertyId::MinWidth,
                    ComputedValue::LengthOrAuto(Some(Length::Px(120.0))),
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
    fn atomic_inline_available_space_clamp_is_not_reported_as_style_max() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    ComputedValue::LengthOrAuto(Some(Length::Px(200.0))),
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
    fn atomic_inline_width_is_not_clamped_when_available_inline_size_is_indefinite() {
        let input = size_input_with_available_inline_size(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Width,
                    ComputedValue::LengthOrAuto(Some(Length::Px(100.0))),
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
    fn normal_flow_explicit_height_is_content_box_and_padding_expands_border_box() {
        let input = size_input_with_style(
            ComputedStyle::initial()
                .with_property(
                    PropertyId::Height,
                    ComputedValue::LengthOrAuto(Some(Length::Px(40.0))),
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

    fn size_input_with_style(
        style: ComputedStyle,
        available_inline_size: f32,
    ) -> SizeResolutionInput {
        size_input_with_available_inline_size(
            style,
            AvailableSize::definite(available_inline_size).expect("available inline size"),
        )
    }

    fn size_input_with_available_inline_size(
        style: ComputedStyle,
        available_inline_size: AvailableSize,
    ) -> SizeResolutionInput {
        let constraint_space =
            ConstraintSpace::new(None, available_inline_size, AvailableSize::Indefinite);
        let style = StyleSizeInputs::from_computed_style(&style).expect("style inputs");
        SizeResolutionInput::new(constraint_space, style, IntrinsicSizes::zero())
    }
}
