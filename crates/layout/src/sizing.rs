//! Sizing contract types for Milestone X.
//!
//! This module defines the shared vocabulary future sizing resolvers must use.
//! It deliberately does not replace the current geometry pass yet; X1 is the
//! architecture boundary that later X issues will implement against.

use crate::ContainingBlockId;

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

/// The sizing environment for a box before used-size resolution.
///
/// Available sizes are content-box bases from the containing formatting
/// context. They are the basis for percentage resolution and line-width
/// selection; a box's own margins, borders, and padding are handled by the
/// used-size resolver when converting style and box metrics into content-box
/// sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstraintSpace {
    containing_block: Option<ContainingBlockId>,
    available_inline_size: AvailableSize,
    available_block_size: AvailableSize,
}

impl ConstraintSpace {
    pub fn new(
        containing_block: Option<ContainingBlockId>,
        available_inline_size: AvailableSize,
        available_block_size: AvailableSize,
    ) -> Self {
        Self {
            containing_block,
            available_inline_size,
            available_block_size,
        }
    }

    pub fn containing_block(self) -> Option<ContainingBlockId> {
        self.containing_block
    }

    pub fn available_inline_size(self) -> AvailableSize {
        self.available_inline_size
    }

    pub fn available_block_size(self) -> AvailableSize {
        self.available_block_size
    }

    pub fn available_size(self, axis: SizeAxis) -> AvailableSize {
        match axis {
            SizeAxis::Inline => self.available_inline_size,
            SizeAxis::Block => self.available_block_size,
        }
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

/// Why a used size took its final value.
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

/// Min/max constraint applied after preferred-size resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppliedSizeConstraint {
    None,
    Min,
    Max,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
