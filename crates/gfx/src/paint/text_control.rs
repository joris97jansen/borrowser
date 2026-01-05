use crate::dom::get_attr;
use crate::input::SelectionRange;
use crate::textarea::{
    TextareaCachedLine, TextareaSelectionPaintParams, layout_textarea_cached_lines,
    paint_textarea_selection, textarea_caret_geometry, textarea_text_height,
};
use crate::text_control::{clamp_caret_to_boundary, input_text_padding};
use css::{ComputedStyle, Length};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use layout::{LayoutBox, TextMeasurer};

use super::context::PaintCtx;

pub(super) fn paint_input_text<'a>(
    rect: Rect,
    style: &ComputedStyle,
    layout: Option<&LayoutBox<'a>>,
    ctx: PaintCtx<'_>,
) {
    let painter = ctx.painter;
    let measurer = ctx.measurer;

    let is_focused = layout.is_some_and(|lb| ctx.focused == Some(lb.node_id()));

    paint_text_control_container(painter, rect, style, is_focused, ctx.selection_stroke);

    let mut value: &str = "";
    let mut placeholder: Option<&str> = None;
    let mut caret: usize = 0;
    let mut selection: Option<SelectionRange> = None;
    let mut scroll_x: f32 = 0.0;

    if let Some(lb) = layout {
        let id = lb.node_id();
        if let Some((v, c, sel, sx, _sy)) = ctx.input_values.get_state(id) {
            value = v;
            caret = c;
            selection = sel;
            scroll_x = sx;
        }

        placeholder = if value.is_empty() {
            get_attr(lb.node.node, "placeholder")
                .map(str::trim)
                .filter(|ph| !ph.is_empty())
        } else {
            None
        };
    }

    let (pad_l, pad_r, pad_t, pad_b) = input_text_padding(style);
    let available_text_w = (rect.width() - pad_l - pad_r).max(0.0);

    let line_h = measurer.line_height(style);
    let inner_h = (rect.height() - pad_t - pad_b).max(0.0);
    let caret_h = line_h.min(inner_h).max(1.0);
    let extra_y = (inner_h - caret_h).max(0.0) * 0.5;
    let text_y = rect.min.y + pad_t + extra_y;

    let (cr, cg, cb, ca) = style.color;
    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
    let value_color = text_color;
    let placeholder_color = text_color.gamma_multiply(0.6);
    let Length::Px(font_px) = style.font_size;
    let font_id = FontId::proportional(font_px);

    let is_placeholder = value.is_empty();
    let paint_color = if is_placeholder {
        placeholder_color
    } else {
        value_color
    };

    let inner_min_x = rect.min.x + pad_l;
    let inner_max_x = (rect.max.x - pad_r).max(inner_min_x);
    let inner_min_y = rect.min.y + pad_t;
    let inner_max_y = (rect.max.y - pad_b).max(inner_min_y);
    let inner_rect = Rect::from_min_max(
        Pos2 {
            x: inner_min_x,
            y: inner_min_y,
        },
        Pos2 {
            x: inner_max_x,
            y: inner_max_y,
        },
    );

    if is_focused {
        let clip_painter = painter.with_clip_rect(inner_rect);

        let caret = clamp_caret_to_boundary(value, caret);

        let text_w = if is_placeholder {
            0.0
        } else {
            measurer.measure(value, style)
        };
        let caret_w = if is_placeholder {
            0.0
        } else if value.is_char_boundary(caret) {
            measurer.measure(&value[..caret], style)
        } else {
            0.0
        };

        let scroll_max = if !is_placeholder && available_text_w > 0.0 {
            (text_w - available_text_w).max(0.0)
        } else {
            0.0
        };
        scroll_x = scroll_x.clamp(0.0, scroll_max);

        let text_x = inner_rect.min.x - scroll_x;

        if let (false, Some(sel)) = (is_placeholder, selection.filter(|s| s.start < s.end)) {
            let sel_start = sel.start.min(value.len());
            let sel_end = sel.end.min(value.len());

            if value.is_char_boundary(sel_start) && value.is_char_boundary(sel_end) {
                let x0 = measurer.measure(&value[..sel_start], style);
                let x1 = measurer.measure(&value[..sel_end], style);
                let sel_rect = Rect::from_min_max(
                    Pos2 {
                        x: text_x + x0,
                        y: text_y,
                    },
                    Pos2 {
                        x: text_x + x1,
                        y: text_y + caret_h,
                    },
                );

                clip_painter.rect_filled(sel_rect, 0.0, ctx.selection_bg_fill);
            }
        }

        let paint_text = if is_placeholder {
            placeholder.unwrap_or_default()
        } else {
            value
        };
        clip_painter.text(
            Pos2 {
                x: text_x,
                y: text_y,
            },
            Align2::LEFT_TOP,
            paint_text,
            font_id,
            paint_color,
        );

        let caret_x = if is_placeholder {
            inner_rect.min.x
        } else {
            inner_rect.min.x + caret_w - scroll_x
        };
        let caret_max_x = (inner_rect.min.x + available_text_w - 1.0).max(inner_rect.min.x);
        let caret_x = caret_x.clamp(inner_rect.min.x, caret_max_x).round();
        let caret_rect = Rect::from_min_size(
            Pos2 {
                x: caret_x,
                y: text_y,
            },
            Vec2 { x: 1.0, y: caret_h },
        );
        clip_painter.rect_filled(caret_rect, 0.0, value_color);
    } else {
        let painted = if !is_placeholder {
            super::images::truncate_to_fit(measurer, style, value, available_text_w)
        } else {
            let ph = placeholder.unwrap_or_default();
            super::images::truncate_to_fit(measurer, style, ph, available_text_w)
        };

        painter.text(
            Pos2 {
                x: inner_rect.min.x,
                y: text_y,
            },
            Align2::LEFT_TOP,
            &painted,
            font_id,
            paint_color,
        );
    }
}

pub(super) fn paint_textarea<'a>(
    rect: Rect,
    style: &ComputedStyle,
    layout: Option<&LayoutBox<'a>>,
    ctx: PaintCtx<'_>,
) {
    let painter = ctx.painter;
    let measurer = ctx.measurer;

    let is_focused = layout.is_some_and(|lb| ctx.focused == Some(lb.node_id()));

    paint_text_control_container(painter, rect, style, is_focused, ctx.selection_stroke);

    let mut value: &str = "";
    let mut placeholder: Option<&str> = None;
    let mut caret: usize = 0;
    let mut selection: Option<SelectionRange> = None;
    let mut scroll_y: f32 = 0.0;

    if let Some(lb) = layout {
        let id = lb.node_id();
        if let Some((v, c, sel, _sx, sy)) = ctx.input_values.get_state(id) {
            value = v;
            caret = c;
            selection = sel;
            scroll_y = sy;
        }

        placeholder = if value.is_empty() {
            get_attr(lb.node.node, "placeholder")
                .map(str::trim)
                .filter(|ph| !ph.is_empty())
        } else {
            None
        };
    }

    let (pad_l, pad_r, pad_t, pad_b) = input_text_padding(style);

    let inner_min_x = rect.min.x + pad_l;
    let inner_max_x = (rect.max.x - pad_r).max(inner_min_x);
    let inner_min_y = rect.min.y + pad_t;
    let inner_max_y = (rect.max.y - pad_b).max(inner_min_y);
    let inner_rect = Rect::from_min_max(
        Pos2 {
            x: inner_min_x,
            y: inner_min_y,
        },
        Pos2 {
            x: inner_max_x,
            y: inner_max_y,
        },
    );

    let available_text_w = inner_rect.width().max(0.0);
    let available_text_h = inner_rect.height().max(0.0);

    let (cr, cg, cb, ca) = style.color;
    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
    let value_color = text_color;
    let placeholder_color = text_color.gamma_multiply(0.6);
    let Length::Px(font_px) = style.font_size;
    let font_id = FontId::proportional(font_px);

    let is_placeholder = value.is_empty();
    let paint_color = if is_placeholder {
        placeholder_color
    } else {
        value_color
    };

    let paint_text = if is_placeholder {
        placeholder.unwrap_or_default()
    } else {
        value
    };

    let mut owned_lines: Option<Vec<TextareaCachedLine>> = None;
    let lines: &[TextareaCachedLine] = if is_placeholder {
        owned_lines
            .get_or_insert_with(|| {
                layout_textarea_cached_lines(measurer, style, available_text_w, paint_text, false)
            })
            .as_slice()
    } else if is_focused {
        if let Some(cached) = ctx.focused_textarea_lines {
            cached
        } else {
            owned_lines
                .get_or_insert_with(|| {
                    layout_textarea_cached_lines(
                        measurer,
                        style,
                        available_text_w,
                        paint_text,
                        false,
                    )
                })
                .as_slice()
        }
    } else {
        owned_lines
            .get_or_insert_with(|| {
                layout_textarea_cached_lines(measurer, style, available_text_w, paint_text, false)
            })
            .as_slice()
    };

    let text_h = textarea_text_height(lines, measurer.line_height(style));
    let scroll_max = if available_text_h > 0.0 {
        (text_h - available_text_h).max(0.0)
    } else {
        0.0
    };
    scroll_y = scroll_y.clamp(0.0, scroll_max);

    let clip_painter = painter.with_clip_rect(inner_rect);

    if is_focused
        && let (false, Some(sel)) = (is_placeholder, selection.filter(|s| s.start < s.end))
    {
        paint_textarea_selection(
            &clip_painter,
            lines,
            value,
            sel,
            TextareaSelectionPaintParams {
                inner_origin: inner_rect.min,
                scroll_y,
                measurer,
                style,
                selection_bg_fill: ctx.selection_bg_fill,
            },
        );
    }

    for line in lines {
        for tfrag in &line.fragments {
            let Some((start, end)) = tfrag.source_range else {
                continue;
            };
            if start > end || end > paint_text.len() {
                continue;
            }
            if !(paint_text.is_char_boundary(start) && paint_text.is_char_boundary(end)) {
                continue;
            }

            let mut s = &paint_text[start..end];
            if s == " " || s == "\t" {
                s = "\u{00A0}";
            }

            clip_painter.text(
                Pos2 {
                    x: inner_rect.min.x + tfrag.rect.x,
                    y: inner_rect.min.y + tfrag.rect.y - scroll_y,
                },
                Align2::LEFT_TOP,
                s,
                font_id.clone(),
                paint_color,
            );
        }
    }

    if is_focused {
        if is_placeholder {
            let caret_h = measurer.line_height(style).min(available_text_h).max(1.0);
            let caret_rect = Rect::from_min_size(
                Pos2 {
                    x: inner_rect.min.x.round(),
                    y: inner_rect.min.y.round(),
                },
                Vec2 { x: 1.0, y: caret_h },
            );
            clip_painter.rect_filled(caret_rect, 0.0, value_color);
        } else {
            let caret = clamp_caret_to_boundary(value, caret);
            let (cx, cy, ch) = textarea_caret_geometry(lines, value, caret, measurer, style);
            let caret_h = ch.min(available_text_h).max(1.0);
            let caret_rect = Rect::from_min_size(
                Pos2 {
                    x: (inner_rect.min.x + cx).round(),
                    y: (inner_rect.min.y + cy - scroll_y).round(),
                },
                Vec2 { x: 1.0, y: caret_h },
            );
            clip_painter.rect_filled(caret_rect, 0.0, value_color);
        }
    }
}

fn paint_text_control_container(
    painter: &Painter,
    rect: Rect,
    style: &ComputedStyle,
    is_focused: bool,
    focus_stroke: Stroke,
) {
    let (r, g, b, a) = style.background_color;
    let fill = if a > 0 {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    } else {
        Color32::from_rgba_unmultiplied(220, 220, 220, 255)
    };

    painter.rect_filled(rect, 2.0, fill);

    let stroke = if is_focused {
        focus_stroke
    } else {
        Stroke::new(1.0, Color32::from_rgb(120, 120, 120))
    };
    painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Outside);
}
