use crate::dom::get_attr;
use css::ComputedStyle;
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
            let id = layout.map(|lb| lb.node_id());
            let is_pressed = id.is_some_and(|id| {
                ctx.active
                    .is_some_and(|a| a.id == id && matches!(a.kind, HitKind::Button))
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
        }

        ReplacedKind::InputCheckbox | ReplacedKind::InputRadio => {
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
                ctx.selection_stroke
            } else {
                Stroke::new(1.0, Color32::from_rgb(120, 120, 120))
            };
            let corner = (side * 0.2).min(4.0);

            match kind {
                ReplacedKind::InputCheckbox => {
                    painter.rect_filled(control_rect, corner, fill);
                    painter.rect_stroke(control_rect, corner, border, StrokeKind::Outside);

                    if is_checked {
                        let (cr, cg, cb, ca) = style.color;
                        let check_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);
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
        let mut label = format!("{kind:?}").to_uppercase();

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
