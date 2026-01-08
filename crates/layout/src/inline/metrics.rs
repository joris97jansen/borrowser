use css::{ComputedStyle, Length};

use crate::TextMeasurer;

#[derive(Clone, Copy, Debug)]
pub(super) struct FragmentMetrics {
    pub(super) ascent: f32,
    pub(super) descent: f32,
}

impl FragmentMetrics {
    pub(super) fn height(self) -> f32 {
        self.ascent + self.descent
    }
}

fn resolve_font_size_px(font_size: Length) -> f32 {
    // Today we only have `px`, but keep the unit conversion decision centralized.
    // When `Length` grows new variants, we can choose whether to resolve here or
    // earlier in style computation.
    match font_size {
        Length::Px(px) => px,
    }
}

fn compute_font_metrics_from(font_px: f32, line_height: f32) -> FragmentMetrics {
    // We don't have real font metrics yet. Approximate a typical ascent/descent split and
    // distribute line-height "leading" equally above and below the em box (like browsers do).
    //
    // This is used both for per-fragment text metrics and for the line "strut" metrics.
    let font_px = font_px.max(0.0);
    let line_height = line_height.max(0.0);

    let font_ascent = font_px * 0.8;
    let font_descent = font_px - font_ascent;

    let em_height = font_ascent + font_descent;
    let leading = (line_height - em_height).max(0.0);
    let half_leading = leading * 0.5;

    let mut ascent = half_leading + font_ascent;
    if ascent > line_height {
        ascent = line_height;
    }

    let descent = (line_height - ascent).max(0.0);
    FragmentMetrics { ascent, descent }
}

pub(super) fn compute_text_metrics(
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> FragmentMetrics {
    let line_height = measurer.line_height(style);
    let font_px = resolve_font_size_px(style.font_size);
    compute_font_metrics_from(font_px, line_height)
}

pub(super) fn compute_strut_metrics(
    measurer: &dyn TextMeasurer,
    block_style: &ComputedStyle,
    available_height: f32,
) -> (f32, FragmentMetrics) {
    // Each line box has a minimum height derived from the block's font metrics,
    // even if the line contains only replaced content. This matches how browser
    // engines use a "strut" for line box construction.
    let mut strut_font_px = resolve_font_size_px(block_style.font_size);
    let mut base_line_height = measurer.line_height(block_style);

    // Clamp for extreme cases: keep at least one line visible.
    if base_line_height > available_height && available_height > 0.0 {
        strut_font_px = (available_height / 1.2).max(8.0);
        let fake_style = ComputedStyle {
            font_size: Length::Px(strut_font_px),
            ..*block_style
        };
        base_line_height = measurer.line_height(&fake_style);
    }

    let metrics = compute_font_metrics_from(strut_font_px, base_line_height);
    (base_line_height, metrics)
}

pub(super) fn replaced_baseline_metrics_bottom_edge(height: f32) -> FragmentMetrics {
    // For replaced elements, CSS defines the baseline as the bottom margin edge.
    // With only box sizes available today, we treat that as the bottom edge.
    let height = height.max(0.0);
    FragmentMetrics {
        ascent: height,
        descent: 0.0,
    }
}

pub(super) fn inline_block_baseline_metrics_placeholder_bottom_edge(
    height: f32,
) -> FragmentMetrics {
    // CSS2.1: inline-block baseline is the baseline of its last in-flow line box;
    // if it has no in-flow line boxes, it's the bottom margin edge.
    //
    // TODO: Once inline-blocks build line boxes for their contents, compute the
    // actual last-line baseline here (instead of using the bottom edge).
    let height = height.max(0.0);
    FragmentMetrics {
        ascent: height,
        descent: 0.0,
    }
}
