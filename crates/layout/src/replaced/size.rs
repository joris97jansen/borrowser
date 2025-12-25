use super::intrinsic::IntrinsicSize;
use super::length::px_opt;
use css::ComputedStyle;

fn clamp(v: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let mut out = v;
    if let Some(mn) = min {
        out = out.max(mn);
    }
    if let Some(mx) = max {
        out = out.min(mx);
    }
    out
}

fn default_fallback_intrinsic() -> IntrinsicSize {
    // classic HTML fallback (2:1)
    IntrinsicSize::from_w_h(Some(300.0), Some(150.0))
}

/// width/height sizing for replaced elements, with ratio support.
/// Returns (used_width, used_height) in px.
///
/// Current engine constraints:
/// - px-only width/height/min/max-width
/// - no min/max-height yet
/// - no CSS aspect-ratio field yet
pub fn compute_replaced_size(
    style: &ComputedStyle,
    intrinsic: IntrinsicSize,
    available_inline_w: Option<f32>, // Some for inline; None if not applicable
) -> (f32, f32) {
    let intrinsic =
        if intrinsic.width.is_some() || intrinsic.height.is_some() || intrinsic.ratio.is_some() {
            intrinsic
        } else {
            default_fallback_intrinsic()
        };

    // CSS specified sizes (px-only)
    let w_spec = px_opt(style.width);
    let h_spec = px_opt(style.height);

    // width constraints (px-only)
    let min_w = px_opt(style.min_width);
    let max_w = px_opt(style.max_width);

    // Ratio: intrinsic ratio if present else fallback 2:1
    let ratio = intrinsic.ratio.unwrap_or(2.0).max(0.0001);

    // Height is "auto" when CSS height is not specified.
    // For images, this means height should track width changes to preserve aspect ratio.
    let height_is_auto = h_spec.is_none();

    // Base size
    let (mut w, mut h) = match (w_spec, h_spec) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w / ratio),
        (None, Some(h)) => (h * ratio, h),
        (None, None) => {
            let w0 = intrinsic.width.unwrap_or(300.0);
            let h0 = intrinsic.height.unwrap_or_else(|| w0 / ratio);
            (w0, h0)
        }
    };

    // Apply width constraints first
    let w_before = w;
    w = clamp(w, min_w, max_w);

    // If width changed and height is auto, preserve ratio.
    if (w - w_before).abs() > f32::EPSILON && height_is_auto {
        h = w / ratio;
    }

    // Inline clamp (shrink-to-fit-ish)
    if let Some(avail) = available_inline_w {
        if avail.is_finite() && avail > 0.0 && w > avail {
            w = avail;
            if height_is_auto {
                h = w / ratio;
            }
        }
    }

    (w.max(1.0), h.max(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::Length;

    fn assert_close(a: f32, b: f32) {
        let eps = 0.001;
        assert!((a - b).abs() <= eps, "expected {a} ≈ {b}");
    }

    #[test]
    fn intrinsic_size_clamps_preserving_ratio() {
        let style = ComputedStyle::initial();
        let intrinsic = IntrinsicSize::from_w_h(Some(200.0), Some(100.0)); // 2:1

        let (w, h) = compute_replaced_size(&style, intrinsic, Some(100.0));
        assert_close(w, 100.0);
        assert_close(h, 50.0);
    }

    #[test]
    fn width_only_computes_height_from_ratio() {
        let mut style = ComputedStyle::initial();
        style.width = Some(Length::Px(120.0));

        let intrinsic = IntrinsicSize::from_w_h(Some(200.0), Some(100.0)); // 2:1
        let (w, h) = compute_replaced_size(&style, intrinsic, None);

        assert_close(w, 120.0);
        assert_close(h, 60.0);
    }

    #[test]
    fn height_only_computes_width_from_ratio() {
        let mut style = ComputedStyle::initial();
        style.height = Some(Length::Px(25.0));

        let intrinsic = IntrinsicSize::from_w_h(Some(200.0), Some(100.0)); // 2:1
        let (w, h) = compute_replaced_size(&style, intrinsic, None);

        assert_close(w, 50.0);
        assert_close(h, 25.0);
    }

    #[test]
    fn max_width_clamps_and_scales_auto_height() {
        let mut style = ComputedStyle::initial();
        style.max_width = Some(Length::Px(80.0));

        let intrinsic = IntrinsicSize::from_w_h(Some(200.0), Some(100.0)); // 2:1
        let (w, h) = compute_replaced_size(&style, intrinsic, None);

        assert_close(w, 80.0);
        assert_close(h, 40.0);
    }
}
