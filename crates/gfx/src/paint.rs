use crate::EguiTextMeasurer;
use crate::dom::{get_attr, resolve_relative_url};
use crate::input::{ActiveTarget, InputValueStore, SelectionRange, TextareaCachedLine};
use crate::text_control::*;
use css::{ComputedStyle, Display, Length};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use html::{Id, Node, dom_utils::is_non_rendering_element};
use layout::{
    BoxKind, HitKind, LayoutBox, LineBox, ListMarker, Rectangle, ReplacedKind, TextMeasurer,
    content_height, content_x_and_width, content_y,
    inline::{InlineFragment, button_label_from_layout, layout_inline_for_paint},
};
use std::cell::RefCell;
use std::collections::HashMap;

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

#[derive(Clone, Copy)]
pub(crate) struct PaintCtx<'a> {
    pub(crate) painter: &'a Painter,
    pub(crate) origin: Pos2,
    pub(crate) measurer: &'a EguiTextMeasurer,
    pub(crate) base_url: Option<&'a str>,
    pub(crate) resources: &'a dyn ImageProvider,
    pub(crate) input_values: &'a InputValueStore,
    pub(crate) focused: Option<Id>,
    pub(crate) focused_textarea_lines: Option<&'a [TextareaCachedLine]>,
    pub(crate) active: Option<ActiveTarget>,
    pub(crate) selection_bg_fill: Color32,
    pub(crate) selection_stroke: Stroke,
    pub(crate) fragment_rects: Option<&'a RefCell<HashMap<Id, Rectangle>>>,
}

impl<'a> PaintCtx<'a> {
    fn with_origin(self, origin: Pos2) -> Self {
        Self { origin, ..self }
    }
}

pub(crate) fn paint_page<'a>(layout_root: &LayoutBox<'a>, ctx: PaintCtx<'_>, paint_root_bg: bool) {
    paint_layout_box(layout_root, ctx, paint_root_bg);
}

fn paint_line_boxes<'a>(lines: &[LineBox<'a>], ctx: PaintCtx<'_>) {
    let painter = ctx.painter;
    let origin = ctx.origin;
    let measurer = ctx.measurer;
    let base_url = ctx.base_url;
    let resources = ctx.resources;
    let input_values = ctx.input_values;
    let focused = ctx.focused;
    let focused_textarea_lines = ctx.focused_textarea_lines;
    let active = ctx.active;
    let selection_bg_fill = ctx.selection_bg_fill;
    let selection_stroke = ctx.selection_stroke;
    let fragment_rects = ctx.fragment_rects;

    for line in lines {
        for frag in &line.fragments {
            match &frag.kind {
                InlineFragment::Text { text, style, .. } => {
                    let (cr, cg, cb, ca) = style.color;
                    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

                    let Length::Px(font_px) = style.font_size;
                    let font_id = FontId::proportional(font_px);

                    let pos = Pos2 {
                        x: origin.x + frag.rect.x,
                        y: origin.y + frag.rect.y,
                    };

                    painter.text(pos, Align2::LEFT_TOP, text, font_id, text_color);
                }

                InlineFragment::Box { style, layout, .. } => {
                    if let Some(cache) = fragment_rects
                        && let Some(lb) = layout
                        && lb.replaced.is_some()
                    {
                        cache.borrow_mut().insert(lb.node_id(), frag.rect);
                    }

                    let rect = Rect::from_min_size(
                        Pos2 {
                            x: origin.x + frag.rect.x,
                            y: origin.y + frag.rect.y,
                        },
                        Vec2::new(frag.rect.width, frag.rect.height),
                    );

                    if let Some(child_box) = layout {
                        // Paint the inline-block's full content at this inline position.
                        // Compute an origin such that child's rect's top-left lands at `rect.min`.
                        let translated_origin = Pos2 {
                            x: rect.min.x - child_box.rect.x,
                            y: rect.min.y - child_box.rect.y,
                        };

                        // Paint the entire subtree of this inline-block here,
                        // including its background/border and its children.
                        paint_layout_box(
                            child_box,
                            ctx.with_origin(translated_origin),
                            false, // do NOT skip inline-block children inside this subtree
                        );
                    } else {
                        // Fallback: simple placeholder rectangle using the box style.
                        let (r, g, b, a) = style.background_color;
                        let color = if a > 0 {
                            Color32::from_rgba_unmultiplied(r, g, b, a)
                        } else {
                            Color32::from_rgba_unmultiplied(180, 180, 180, 255)
                        };

                        painter.rect_filled(rect, 0.0, color);
                    }
                }

                InlineFragment::Replaced {
                    style,
                    kind,
                    layout,
                    ..
                } => {
                    let rect = Rect::from_min_size(
                        Pos2 {
                            x: origin.x + frag.rect.x,
                            y: origin.y + frag.rect.y,
                        },
                        Vec2::new(frag.rect.width, frag.rect.height),
                    );

                    if let Some(cache) = fragment_rects
                        && let Some(lb) = layout
                    {
                        cache.borrow_mut().insert(lb.node_id(), frag.rect);
                    }

                    // --- BUTTON: pressed visual state (uses `active`) ---
                    if matches!(kind, ReplacedKind::Button) {
                        let id = layout.map(|lb| lb.node_id());
                        let is_pressed = id.is_some_and(|id| {
                            active.is_some_and(|a| a.id == id && matches!(a.kind, HitKind::Button))
                        });

                        let fill = if is_pressed {
                            Color32::from_rgb(200, 200, 200)
                        } else {
                            Color32::from_rgb(230, 230, 230)
                        };

                        painter.rect_filled(rect, 6.0, fill);

                        let stroke = if is_pressed {
                            Stroke::new(2.0, Color32::from_rgb(110, 110, 110))
                        } else {
                            Stroke::new(1.0, Color32::from_rgb(140, 140, 140))
                        };
                        painter.rect_stroke(rect, 6.0, stroke, StrokeKind::Outside);

                        let mut label = "Button".to_string();
                        if let Some(lb) = layout {
                            label = button_label_from_layout(lb);
                        }

                        let offset = if is_pressed {
                            Vec2::new(1.0, 1.0)
                        } else {
                            Vec2::ZERO
                        };

                        painter.text(
                            rect.center() + offset,
                            Align2::CENTER_CENTER,
                            label,
                            FontId::proportional(12.0),
                            Color32::from_rgb(60, 60, 60),
                        );

                        continue; // IMPORTANT: don't fall through to generic replaced painting
                    }

                    // --- INPUT CHECKBOX / RADIO ---
                    if matches!(kind, ReplacedKind::InputCheckbox | ReplacedKind::InputRadio) {
                        let id = layout.map(|lb| lb.node_id());
                        let is_checked = id.is_some_and(|id| input_values.is_checked(id));
                        let is_focused = id.is_some_and(|id| focused == Some(id));

                        let is_pressed = id.is_some_and(|id| {
                            active.is_some_and(|a| {
                                a.id == id && matches!(a.kind, HitKind::Checkbox | HitKind::Radio)
                            })
                        });

                        let side = rect.width().min(rect.height()).max(0.0);
                        if side > 0.0 {
                            let control_rect =
                                Rect::from_center_size(rect.center(), Vec2::splat(side));

                            let (br, bg, bb, ba) = style.background_color;
                            let base_fill = if ba > 0 {
                                Color32::from_rgba_unmultiplied(br, bg, bb, ba)
                            } else {
                                Color32::WHITE
                            };
                            let fill = if is_pressed {
                                base_fill.gamma_multiply(0.9)
                            } else {
                                base_fill
                            };

                            let border = if is_focused {
                                selection_stroke
                            } else {
                                Stroke::new(1.0, Color32::from_rgb(120, 120, 120))
                            };
                            let corner = (side * 0.2).min(4.0);

                            match kind {
                                ReplacedKind::InputCheckbox => {
                                    painter.rect_filled(control_rect, corner, fill);
                                    painter.rect_stroke(
                                        control_rect,
                                        corner,
                                        border,
                                        StrokeKind::Outside,
                                    );

                                    if is_checked {
                                        let (cr, cg, cb, ca) = style.color;
                                        let check_color =
                                            Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
                                        let thickness = (side * 0.12).max(1.5);

                                        let a = Pos2 {
                                            x: control_rect.min.x + side * 0.25,
                                            y: control_rect.min.y + side * 0.55,
                                        };
                                        let b = Pos2 {
                                            x: control_rect.min.x + side * 0.45,
                                            y: control_rect.min.y + side * 0.75,
                                        };
                                        let c = Pos2 {
                                            x: control_rect.min.x + side * 0.80,
                                            y: control_rect.min.y + side * 0.30,
                                        };

                                        let stroke = Stroke::new(thickness, check_color);
                                        painter.line_segment([a, b], stroke);
                                        painter.line_segment([b, c], stroke);
                                    }
                                }

                                ReplacedKind::InputRadio => {
                                    let center = control_rect.center();
                                    let r = side * 0.5;
                                    painter.circle_filled(center, r, fill);
                                    painter.circle_stroke(center, r, border);

                                    if is_checked {
                                        let (cr, cg, cb, ca) = style.color;
                                        let dot = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
                                        painter.circle_filled(center, r * 0.45, dot);
                                    }
                                }

                                _ => unreachable!("handled by match guard"),
                            }
                        }

                        continue; // don't fall through to generic replaced painting
                    }

                    // --- IMG: decoded texture (if ready) or accessibility-ish fallback ---
                    if matches!(kind, ReplacedKind::Img) {
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
                                let uv = Rect::from_min_max(
                                    Pos2 { x: 0.0, y: 0.0 },
                                    Pos2 { x: 1.0, y: 1.0 },
                                );
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

                        continue;
                    }

                    let is_focused_text_control =
                        matches!(kind, ReplacedKind::InputText | ReplacedKind::TextArea)
                            && layout.is_some_and(|lb| focused == Some(lb.node_id()));

                    // Fill + stroke (placeholder look)
                    let (r, g, b, a) = style.background_color;
                    let fill = if a > 0 {
                        Color32::from_rgba_unmultiplied(r, g, b, a)
                    } else {
                        Color32::from_rgba_unmultiplied(220, 220, 220, 255)
                    };

                    painter.rect_filled(rect, 2.0, fill);
                    let stroke = if is_focused_text_control {
                        selection_stroke
                    } else {
                        Stroke::new(1.0, Color32::from_rgb(120, 120, 120))
                    };

                    painter.rect_stroke(rect, 2.0, stroke, StrokeKind::Outside);

                    // Special case: <input type="text"> draws its value/placeholder inside the box
                    if matches!(kind, ReplacedKind::InputText) {
                        // Determine shown text: value first, else placeholder
                        let mut value = String::new();
                        let mut placeholder: Option<String> = None;
                        let mut caret: usize = 0;
                        let mut selection: Option<SelectionRange> = None;
                        let mut scroll_x: f32 = 0.0;

                        if let Some(lb) = layout {
                            let id = lb.node_id();
                            if let Some((v, c, sel, sx, _sy)) = input_values.get_state(id) {
                                value = v.to_string();
                                caret = c;
                                selection = sel;
                                scroll_x = sx;
                            }

                            placeholder = if value.is_empty() {
                                get_attr(lb.node.node, "placeholder")
                                    .map(str::trim)
                                    .filter(|ph| !ph.is_empty())
                                    .map(|ph| ph.to_string())
                            } else {
                                None
                            };
                        }

                        // Inner text area from padding (with sane minimums)
                        let (pad_l, pad_r, pad_t, pad_b) = input_text_padding(style);

                        let available_text_w = (rect.width() - pad_l - pad_r).max(0.0);

                        let line_h = measurer.line_height(style);
                        let inner_h = (rect.height() - pad_t - pad_b).max(0.0);
                        let caret_h = line_h.min(inner_h).max(1.0);
                        let extra_y = (inner_h - caret_h).max(0.0) * 0.5;
                        let text_y = rect.min.y + pad_t + extra_y;

                        // Paint in style color (placeholder uses a lighter tint).
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

                        if is_focused_text_control {
                            // Focused input: render the full value, clipped to the inner rect,
                            // with a caret and optional selection highlight.
                            let clip_painter = painter.with_clip_rect(inner_rect);

                            let caret = clamp_caret_to_boundary(&value, caret);

                            // Scroll horizontally to keep the caret visible.
                            let text_w = if is_placeholder {
                                0.0
                            } else {
                                measurer.measure(&value, style)
                            };
                            let caret_w = if is_placeholder {
                                0.0
                            } else {
                                measurer.measure(&value[..caret], style)
                            };

                            // `scroll_x` is persistent state in the store; clamp it to current bounds.
                            let scroll_max = if !is_placeholder && available_text_w > 0.0 {
                                (text_w - available_text_w).max(0.0)
                            } else {
                                0.0
                            };
                            scroll_x = scroll_x.clamp(0.0, scroll_max);

                            let text_x = inner_rect.min.x - scroll_x;

                            // Selection highlight (single-line).
                            if let (false, Some(sel)) =
                                (is_placeholder, selection.filter(|s| s.start < s.end))
                            {
                                let sel_start = sel.start.min(value.len());
                                let sel_end = sel.end.min(value.len());

                                if value.is_char_boundary(sel_start)
                                    && value.is_char_boundary(sel_end)
                                {
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

                                    clip_painter.rect_filled(sel_rect, 0.0, selection_bg_fill);
                                }
                            }

                            // Text
                            let paint_text = if is_placeholder {
                                placeholder.as_deref().unwrap_or_default()
                            } else {
                                value.as_str()
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

                            // Caret: 1px vertical line.
                            let caret_x = if is_placeholder {
                                inner_rect.min.x
                            } else {
                                inner_rect.min.x + caret_w - scroll_x
                            };
                            let caret_max_x =
                                (inner_rect.min.x + available_text_w - 1.0).max(inner_rect.min.x);
                            let caret_x = caret_x.clamp(inner_rect.min.x, caret_max_x).round();
                            let caret_rect = Rect::from_min_size(
                                Pos2 {
                                    x: caret_x,
                                    y: text_y,
                                },
                                Vec2 { x: 1.0, y: caret_h },
                            );
                            // Caret uses the actual text color, not placeholder styling.
                            clip_painter.rect_filled(caret_rect, 0.0, value_color);
                        } else {
                            // Unfocused input: show a simple truncated preview (no caret/selection).
                            let painted = if !is_placeholder {
                                truncate_to_fit(measurer, style, &value, available_text_w)
                            } else {
                                let ph = placeholder.as_deref().unwrap_or_default();
                                truncate_to_fit(measurer, style, ph, available_text_w)
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

                        continue; // skip default label painting below
                    }

                    // Special case: <textarea> draws its multi-line value with wrapping.
                    if matches!(kind, ReplacedKind::TextArea) {
                        let mut value: &str = "";
                        let mut placeholder: Option<&str> = None;
                        let mut caret: usize = 0;
                        let mut selection: Option<SelectionRange> = None;
                        let mut scroll_y: f32 = 0.0;

                        if let Some(lb) = layout {
                            let id = lb.node_id();
                            if let Some((v, c, sel, _sx, sy)) = input_values.get_state(id) {
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

                        // Inner text area from padding (with sane minimums)
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

                        // Paint in style color (placeholder uses a lighter tint).
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
                                    layout_textarea_cached_lines(
                                        measurer,
                                        style,
                                        available_text_w,
                                        paint_text,
                                        false,
                                    )
                                })
                                .as_slice()
                        } else if is_focused_text_control {
                            if let Some(cached) = focused_textarea_lines {
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
                                    layout_textarea_cached_lines(
                                        measurer,
                                        style,
                                        available_text_w,
                                        paint_text,
                                        false,
                                    )
                                })
                                .as_slice()
                        };

                        // Clamp scroll to the current text bounds.
                        let text_h = textarea_text_height(lines, measurer.line_height(style));
                        let scroll_max = if available_text_h > 0.0 {
                            (text_h - available_text_h).max(0.0)
                        } else {
                            0.0
                        };
                        scroll_y = scroll_y.clamp(0.0, scroll_max);

                        let clip_painter = painter.with_clip_rect(inner_rect);

                        // Multi-line selection highlight.
                        if is_focused_text_control
                            && let (false, Some(sel)) =
                                (is_placeholder, selection.filter(|s| s.start < s.end))
                        {
                            paint_textarea_selection(
                                &clip_painter,
                                lines,
                                value,
                                sel,
                                TextAreaSelectionPaintParams {
                                    inner_origin: inner_rect.min,
                                    scroll_y,
                                    measurer,
                                    style,
                                    selection_bg_fill,
                                },
                            );
                        }

                        // Text fragments
                        for line in lines {
                            for tfrag in &line.fragments {
                                let Some((start, end)) = tfrag.source_range else {
                                    continue;
                                };
                                if start > end || end > paint_text.len() {
                                    continue;
                                }
                                if !(paint_text.is_char_boundary(start)
                                    && paint_text.is_char_boundary(end))
                                {
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

                        // Caret: 1px vertical line.
                        if is_focused_text_control {
                            if is_placeholder {
                                let caret_h =
                                    measurer.line_height(style).min(available_text_h).max(1.0);
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
                                let (cx, cy, ch) =
                                    textarea_caret_geometry(lines, value, caret, measurer, style);
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

                        continue; // skip default label painting below
                    }

                    // Default centered label for other replaced elements
                    let mut label = match kind {
                        ReplacedKind::Img => "IMG".to_string(),
                        ReplacedKind::Button => "BUTTON".to_string(),
                        ReplacedKind::InputText => unreachable!("handled above"),
                        ReplacedKind::TextArea => unreachable!("handled above"),
                        ReplacedKind::InputCheckbox => "CHECKBOX".to_string(),
                        ReplacedKind::InputRadio => "RADIO".to_string(),
                    };

                    // If <img alt="...">, show alt text instead
                    if let (ReplacedKind::Img, Some(alt)) =
                        (kind, layout.and_then(|lb| get_attr(lb.node.node, "alt")))
                    {
                        let alt = alt.trim();
                        if !alt.is_empty() {
                            label = alt.to_string();
                        }
                    }

                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(12.0),
                        Color32::from_rgb(60, 60, 60),
                    );
                }
            }
        }
    }
}

fn paint_layout_box<'a>(
    layout: &LayoutBox<'a>,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    let painter = ctx.painter;
    let origin = ctx.origin;
    let measurer = ctx.measurer;

    // 0) Do not paint non-rendering elements (head, style, script, etc.)
    if is_non_rendering_element(layout.node.node) {
        for child in &layout.children {
            paint_layout_box(child, ctx, skip_inline_block_children);
        }
        return;
    }

    let rect = Rect::from_min_size(
        Pos2 {
            x: origin.x + layout.rect.x,
            y: origin.y + layout.rect.y,
        },
        Vec2 {
            x: layout.rect.width,
            y: layout.rect.height,
        },
    );

    // background
    let (r, g, b, a) = layout.style.background_color;
    if a > 0 {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(r, g, b, a));
    }

    // 1) List marker (for display:list-item), if any.
    //    This does not affect layout; it's purely visual.
    if matches!(layout.style.display, Display::ListItem) {
        paint_list_marker(layout, painter, origin, measurer);
    }

    // 2) Inline content
    paint_inline_content(layout, ctx);

    // 3) Recurse into children
    for child in &layout.children {
        // ✅ Inline engine already painted inline-blocks AND replaced elements via fragments.
        if skip_inline_block_children
            && (matches!(child.kind, BoxKind::InlineBlock) || child.replaced.is_some())
        {
            continue;
        }

        paint_layout_box(child, ctx, skip_inline_block_children);
    }
}

fn paint_list_marker<'a>(
    layout: &LayoutBox<'a>,
    painter: &Painter,
    origin: Pos2,
    measurer: &dyn TextMeasurer,
) {
    let marker = match layout.list_marker {
        Some(m) => m,
        None => return, // nothing to paint
    };

    // Choose marker text: bullet or number.
    let marker_text = match marker {
        ListMarker::Unordered => "•".to_string(),
        ListMarker::Ordered(index) => format!("{index}."),
    };

    // Use the list item's text style for the marker.
    let style = layout.style;
    let (cr, cg, cb, ca) = style.color;
    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

    let Length::Px(font_px) = style.font_size;
    let font_id = FontId::proportional(font_px);

    // Position: slightly to the left of the content box (padding-left),
    // aligned with the top of the content. This doesn't change layout height.
    let bm = layout.style.box_metrics;

    // Content box x/y in layout coordinates (same as inline content start).
    let content_x = layout.rect.x + bm.padding_left;
    let content_y = layout.rect.y + bm.padding_top;

    // Measure marker width so we can place it just to the left of the content.
    let marker_width = measurer.measure(&marker_text, style);

    // How much gap between marker and content.
    let gap = 4.0;

    let marker_pos = Pos2 {
        x: origin.x + content_x - marker_width - gap,
        y: origin.y + content_y,
    };

    painter.text(
        marker_pos,
        Align2::LEFT_TOP,
        marker_text,
        font_id,
        text_color,
    );
}

// Paint a sequence of LineBox/LineFragment produced by the inline engine.
// Text fragments are painted directly; Box fragments (inline-blocks) are
// painted by translating the associated LayoutBox subtree into the fragment
// rect position.
fn paint_inline_content<'a>(layout: &LayoutBox<'a>, ctx: PaintCtx<'_>) {
    // ✅ Replaced elements (<textarea>, <input>, <img>, <button>) do NOT paint their DOM children.
    // They are painted by InlineFragment::Replaced in paint_line_boxes.
    if layout.replaced.is_some() {
        return;
    }

    let measurer = ctx.measurer;

    // Only block-like elements host their own inline formatting context.
    match layout.node.node {
        Node::Element { .. } => {
            // Inline elements do NOT establish their own block-level
            // inline formatting context; their text is handled by the
            // nearest block ancestor.
            if matches!(layout.style.display, Display::Inline) {
                return;
            }
        }
        // The Document node itself also does not host inline content;
        // its block children (html/body/etc.) will do that.
        Node::Document { .. } => return,
        _ => return,
    }

    // Compute the content box consistently with the layout engine.
    let (content_x, content_width) =
        content_x_and_width(layout.style, layout.rect.x, layout.rect.width);
    let content_y = content_y(layout.style, layout.rect.y);
    let content_height = content_height(layout.style, layout.rect.height);

    let block_rect = Rectangle {
        x: content_x,
        y: content_y,
        width: content_width,
        height: content_height,
    };

    // Use the painting-aware inline layout: text + inline-block boxes,
    // enumerated from the layout tree in DOM order. LineBox/LineFragment are
    // the source of truth for inline geometry here.
    let lines = layout_inline_for_paint(measurer, block_rect, layout);

    if lines.is_empty() {
        return;
    }

    paint_line_boxes(&lines, ctx);
}

fn truncate_to_fit(
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
