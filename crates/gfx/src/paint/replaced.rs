use css::{ComputedStyle, Length};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};
use layout::inline::button_label_from_layout;
use layout::{HitKind, LayoutBox, ReplacedKind};

use super::context::PaintCtx;

pub(super) fn paint_replaced_fragment<'a>(
    rect: Rect,
    style: &ComputedStyle,
    kind: ReplacedKind,
    layout: Option<&LayoutBox<'a>>,
    ctx: PaintCtx<'_>,
) {
    let painter = ctx.painter;

    match kind {
        ReplacedKind::Button => {
            let font_id = font_id_from_style(style);
            let text_color = text_color_from_style(style);

            let id = layout.map(|lb| lb.node_id());
            let is_pressed = id.is_some_and(|id| {
                ctx.active
                    .is_some_and(|a| a.id == id && matches!(a.kind, HitKind::Button))
            });

            let base_fill = background_color_from_style(style);
            let fill = if is_pressed {
                base_fill.gamma_multiply(0.9)
            } else {
                base_fill
            };

            painter.rect_filled(rect, 6.0, fill);

            let mut border_color = blend_colors(fill, text_color, 0.5);
            if is_pressed {
                border_color = border_color.gamma_multiply(0.9);
            }
            let border_color = force_opaque(border_color);
            let border_width = if is_pressed { 2.0 } else { 1.0 };
            let stroke = Stroke::new(border_width, border_color);
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

            let label_color = if is_pressed {
                text_color.gamma_multiply(0.9)
            } else {
                text_color
            };
            painter.text(
                rect.center() + offset,
                Align2::CENTER_CENTER,
                label,
                font_id,
                label_color,
            );
        }

        ReplacedKind::InputCheckbox | ReplacedKind::InputRadio => {
            let text_color = text_color_from_style(style);

            let id = layout.map(|lb| lb.node_id());
            let is_checked = id.is_some_and(|id| ctx.input_values.is_checked(id));
            let is_focused = id.is_some_and(|id| ctx.focused == Some(id));

            let is_pressed = id.is_some_and(|id| {
                ctx.active.is_some_and(|a| {
                    a.id == id && matches!(a.kind, HitKind::Checkbox | HitKind::Radio)
                })
            });

            let side = rect.width().min(rect.height()).max(0.0);
            if side <= 0.0 {
                return;
            }

            let control_rect = Rect::from_center_size(rect.center(), Vec2::splat(side));

            let base_fill = background_color_from_style(style);
            let fill = if is_pressed {
                base_fill.gamma_multiply(0.9)
            } else {
                base_fill
            };

            let glyph_color = control_glyph_color(text_color, fill);

            let border = if is_focused {
                ctx.selection_stroke
            } else {
                Stroke::new(1.0, force_opaque(blend_colors(fill, text_color, 0.5)))
            };
            let corner = (side * 0.2).min(4.0);

            match kind {
                ReplacedKind::InputCheckbox => {
                    painter.rect_filled(control_rect, corner, fill);
                    painter.rect_stroke(control_rect, corner, border, StrokeKind::Outside);

                    if is_checked {
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

                        let stroke = Stroke::new(thickness, glyph_color);
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
                        painter.circle_filled(center, r * 0.45, glyph_color);
                    }
                }

                _ => unreachable!("handled by match guard"),
            }
        }

        ReplacedKind::Img => super::images::paint_img_fragment(rect, style, layout, ctx),

        ReplacedKind::InputText => super::text_control::paint_input_text(rect, style, layout, ctx),

        ReplacedKind::TextArea => super::text_control::paint_textarea(rect, style, layout, ctx),
    }

    // Default centered label for other replaced elements.
    // (Currently exhaustive, but this is useful if new kinds are added.)
    if !matches!(
        kind,
        ReplacedKind::Button
            | ReplacedKind::InputText
            | ReplacedKind::TextArea
            | ReplacedKind::InputCheckbox
            | ReplacedKind::InputRadio
            | ReplacedKind::Img
    ) {
        let font_id = font_id_from_style(style);
        let text_color = text_color_from_style(style);

        let label = format!("{kind:?}").to_uppercase();

        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font_id,
            text_color,
        );
    }
}

fn font_id_from_style(style: &ComputedStyle) -> FontId {
    match style.font_size {
        Length::Px(px) => FontId::proportional(px),
    }
}

fn text_color_from_style(style: &ComputedStyle) -> Color32 {
    let (r, g, b, a) = style.color;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn background_color_from_style(style: &ComputedStyle) -> Color32 {
    let (r, g, b, a) = style.background_color;
    if a > 0 {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    } else {
        default_control_background()
    }
}

fn default_control_background() -> Color32 {
    Color32::from_rgba_unmultiplied(230, 230, 230, 255)
}

fn control_glyph_color(text_color: Color32, fill: Color32) -> Color32 {
    if text_color.a() > 0 {
        text_color
    } else {
        default_control_glyph_color(fill)
    }
}

fn default_control_glyph_color(fill: Color32) -> Color32 {
    let fill = force_opaque(fill);
    let luma = (fill.r() as u32 * 299 + fill.g() as u32 * 587 + fill.b() as u32 * 114) / 1000;

    if luma >= 128 {
        blend_rgb(fill, Color32::BLACK, 0.7)
    } else {
        blend_rgb(fill, Color32::WHITE, 0.8)
    }
}

fn blend_rgb(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| -> u8 {
        ((x as f32) + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };

    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        255,
    )
}

fn blend_colors(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| -> u8 {
        ((x as f32) + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };

    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

fn force_opaque(color: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 255)
}
