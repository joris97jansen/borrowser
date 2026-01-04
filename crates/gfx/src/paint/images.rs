use crate::dom::{get_attr, resolve_relative_url};
use css::{ComputedStyle, Length};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use layout::{LayoutBox, TextMeasurer};

use super::context::PaintCtx;

#[derive(Clone, Debug)]
pub enum ImageState {
    Missing,
    Loading,
    Decoding,
    Ready {
        texture_id: egui::TextureId,
        size_px: [usize; 2],
    },
    Error {
        error: String,
    },
}

pub trait ImageProvider {
    fn image_state_by_url(&self, url: &str) -> ImageState;
    fn image_intrinsic_size_px(&self, url: &str) -> Option<(u32, u32)>;
}

pub(super) fn paint_img_fragment<'a>(
    rect: Rect,
    style: &ComputedStyle,
    layout: Option<&LayoutBox<'a>>,
    ctx: PaintCtx<'_>,
) {
    let painter = ctx.painter;
    let base_url = ctx.base_url;
    let resources = ctx.resources;
    let measurer = ctx.measurer;

    let alt = layout
        .and_then(|lb| get_attr(lb.node.node, "alt"))
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let img_url = layout
        .and_then(|lb| get_attr(lb.node.node, "src"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|src| resolve_relative_url(base_url, src));

    let state = img_url
        .as_deref()
        .map(|url| resources.image_state_by_url(url))
        .unwrap_or(ImageState::Missing);

    match state {
        ImageState::Ready { texture_id, .. } => {
            let uv = Rect::from_min_max(Pos2 { x: 0.0, y: 0.0 }, Pos2 { x: 1.0, y: 1.0 });
            painter.image(texture_id, rect, uv, Color32::WHITE);
        }
        ImageState::Loading | ImageState::Decoding => {
            paint_img_fallback_placeholder(
                painter,
                rect,
                style,
                measurer,
                ImgFallbackState::Loading,
                alt,
            );
        }
        ImageState::Error { .. } => {
            paint_img_fallback_placeholder(
                painter,
                rect,
                style,
                measurer,
                ImgFallbackState::Error,
                alt,
            );
        }
        ImageState::Missing => {
            paint_img_fallback_placeholder(
                painter,
                rect,
                style,
                measurer,
                ImgFallbackState::Missing,
                alt,
            );
        }
    }
}

pub(super) fn truncate_to_fit(
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
    text: &str,
    max_w: f32,
) -> String {
    if text.is_empty() || max_w <= 0.0 {
        return String::new();
    }
    if measurer.measure(text, style) <= max_w {
        return text.to_string();
    }

    // Simple ellipsis truncation.
    let ell = "…";
    let ell_w = measurer.measure(ell, style);
    if ell_w > max_w {
        return String::new();
    }

    // Binary search cut point.
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();

    while lo < hi {
        let mid = (lo + hi) / 2;
        let candidate: String = chars[..mid].iter().collect();
        let w = measurer.measure(&(candidate.clone() + ell), style);
        if w <= max_w {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    let cut = lo.saturating_sub(1);
    let mut s: String = chars[..cut].iter().collect();
    s.push_str(ell);
    s
}

enum ImgFallbackState {
    Missing,
    Loading,
    Error,
}

fn paint_img_fallback_placeholder(
    painter: &Painter,
    rect: Rect,
    style: &ComputedStyle,
    measurer: &dyn TextMeasurer,
    state: ImgFallbackState,
    alt: Option<&str>,
) {
    // Placeholder box
    let (r, g, b, a) = style.background_color;
    let fill = if a > 0 {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    } else {
        Color32::from_rgba_unmultiplied(220, 220, 220, 255)
    };

    painter.rect_filled(rect, 2.0, fill);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_rgb(120, 120, 120)),
        StrokeKind::Outside,
    );

    // Error decoration (subtle "broken" cross)
    if matches!(state, ImgFallbackState::Error) {
        let inset = 5.0;
        if rect.width() > inset * 2.0 && rect.height() > inset * 2.0 {
            let a = rect.min + Vec2::new(inset, inset);
            let b = rect.max - Vec2::new(inset, inset);
            let c = Pos2 { x: a.x, y: b.y };
            let d = Pos2 { x: b.x, y: a.y };

            let stroke = Stroke::new(1.5, Color32::from_rgba_unmultiplied(220, 80, 80, 140));
            painter.line_segment([a, b], stroke);
            painter.line_segment([c, d], stroke);
        }
    }

    // Text content (status + alt)
    let padding = 6.0;
    let inner = rect.shrink(padding);
    if inner.width() <= 1.0 || inner.height() <= 1.0 {
        return;
    }

    let clip_painter = painter.with_clip_rect(rect);

    let (cr, cg, cb, ca) = style.color;
    let base_text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

    let status = match state {
        ImgFallbackState::Loading => Some("Loading…"),
        ImgFallbackState::Error => Some("Failed to load image"),
        ImgFallbackState::Missing => None,
    };

    let main_text = match (state, alt) {
        (ImgFallbackState::Error, Some(alt)) => Some(alt),
        (ImgFallbackState::Error, None) => Some("Broken image"),
        (ImgFallbackState::Loading, Some(alt)) => Some(alt),
        (ImgFallbackState::Loading, None) => None,
        (ImgFallbackState::Missing, Some(alt)) => Some(alt),
        (ImgFallbackState::Missing, None) => Some("IMG"),
    };

    let mut y = inner.min.y;
    let mut remaining_h = inner.height();

    if let Some(status) = status {
        let mut status_style = *style;
        let Length::Px(font_px) = style.font_size;
        status_style.font_size = Length::Px((font_px * 0.85).clamp(10.0, 12.0));

        let status_color = base_text_color.gamma_multiply(0.65);
        let font_id = match status_style.font_size {
            Length::Px(px) => FontId::proportional(px),
        };
        clip_painter.text(
            Pos2 { x: inner.min.x, y },
            Align2::LEFT_TOP,
            status,
            font_id,
            status_color,
        );

        let status_h = measurer.line_height(&status_style);
        y += status_h;
        remaining_h = (remaining_h - status_h).max(0.0);
    }

    if let Some(text) = main_text
        && remaining_h > 1.0
    {
        paint_wrapped_text(
            &clip_painter,
            Rect::from_min_size(
                Pos2 { x: inner.min.x, y },
                Vec2::new(inner.width(), remaining_h),
            ),
            style,
            measurer,
            text,
            base_text_color,
        );
    }
}

fn paint_wrapped_text(
    painter: &Painter,
    rect: Rect,
    style: &ComputedStyle,
    measurer: &dyn TextMeasurer,
    text: &str,
    color: Color32,
) {
    let max_w = rect.width().max(0.0);
    let max_h = rect.height().max(0.0);
    if max_w <= 1.0 || max_h <= 1.0 {
        return;
    }

    let line_h = measurer.line_height(style).max(1.0);
    let max_lines = (max_h / line_h).floor().max(0.0) as usize;
    if max_lines == 0 {
        return;
    }

    let mut lines = wrap_text_to_width(text, max_w, measurer, style);
    if lines.is_empty() {
        return;
    }

    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            if !last.ends_with('…') {
                last.push('…');
            }
            *last = ellipsize_to_width(last, max_w, measurer, style);
        }
    }

    let font_id = match style.font_size {
        Length::Px(px) => FontId::proportional(px),
    };

    for (i, line) in lines.iter().enumerate() {
        let y = rect.min.y + (i as f32) * line_h;
        if y > rect.max.y {
            break;
        }
        painter.text(
            Pos2 { x: rect.min.x, y },
            Align2::LEFT_TOP,
            line,
            font_id.clone(),
            color,
        );
    }
}

fn wrap_text_to_width(
    text: &str,
    max_width: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            let w = measurer.measure(word, style);
            if w <= max_width {
                current.push_str(word);
            } else {
                lines.push(ellipsize_to_width(word, max_width, measurer, style));
            }
            continue;
        }

        let candidate = format!("{current} {word}");
        if measurer.measure(&candidate, style) <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));

            let w = measurer.measure(word, style);
            if w <= max_width {
                current.push_str(word);
            } else {
                lines.push(ellipsize_to_width(word, max_width, measurer, style));
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn ellipsize_to_width(
    text: &str,
    max_width: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    if !(max_width.is_finite() && max_width > 0.0) {
        return String::new();
    }

    if measurer.measure(text, style) <= max_width {
        return text.to_string();
    }

    let ellipsis = "…";
    if measurer.measure(ellipsis, style) > max_width {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut lo: usize = 0;
    let mut hi: usize = chars.len();

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let mut candidate: String = chars[..mid].iter().collect();
        candidate.push_str(ellipsis);

        if measurer.measure(&candidate, style) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let mut out: String = chars[..lo].iter().collect();
    out.push_str(ellipsis);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct FixedMeasurer;

    impl TextMeasurer for FixedMeasurer {
        fn measure(&self, text: &str, _style: &ComputedStyle) -> f32 {
            text.chars().count() as f32
        }

        fn line_height(&self, _style: &ComputedStyle) -> f32 {
            10.0
        }
    }

    #[test]
    fn ellipsize_to_width_never_exceeds_limit() {
        let measurer = FixedMeasurer;
        let style = ComputedStyle::initial();

        let s = ellipsize_to_width("hello world", 5.0, &measurer, &style);
        assert!(measurer.measure(&s, &style) <= 5.0);
        assert!(s.ends_with('…') || s.is_empty());
    }

    #[test]
    fn wrap_text_to_width_respects_width_per_line() {
        let measurer = FixedMeasurer;
        let style = ComputedStyle::initial();

        let lines = wrap_text_to_width("a bb ccc dddd", 3.0, &measurer, &style);
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(measurer.measure(line, &style) <= 3.0);
        }
    }
}
