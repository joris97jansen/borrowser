use css::{Display, Length};
use egui::{Align2, Color32, FontId, Pos2, Rect, Vec2};
use html::Node;
use layout::{
    LayoutBox, LineBox, Rectangle, content_height, content_x_and_width, content_y,
    inline::{InlineFragment, layout_inline_for_paint},
};

use super::context::PaintCtx;

pub(super) fn paint_inline_content<'a>(layout: &LayoutBox<'a>, ctx: PaintCtx<'_>) {
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

fn paint_line_boxes<'a>(lines: &[LineBox<'a>], ctx: PaintCtx<'_>) {
    let painter = ctx.painter;
    let origin = ctx.origin;

    let fragment_rects = ctx.fragment_rects;

    for line in lines {
        for frag in &line.fragments {
            match &frag.kind {
                InlineFragment::Text { text, style, .. } => {
                    let (cr, cg, cb, ca) = style.color;
                    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

                    let Length::Px(font_px) = style.font_size;
                    let font_id = FontId::proportional(font_px);

                    let paint_rect = frag.paint_rect.rect();
                    let pos = Pos2 {
                        x: origin.x + paint_rect.x,
                        y: origin.y + paint_rect.y,
                    };

                    painter.text(pos, Align2::LEFT_TOP, text, font_id, text_color);
                }

                InlineFragment::Box { style, layout, .. } => {
                    let paint_rect = frag.paint_rect.rect();

                    if let Some(cache) = fragment_rects
                        && let Some(lb) = layout
                        && lb.replaced.is_some()
                    {
                        cache
                            .borrow_mut()
                            .insert(lb.node_id(), frag.paint_rect.rect());
                    }

                    let rect = Rect::from_min_size(
                        Pos2 {
                            x: origin.x + paint_rect.x,
                            y: origin.y + paint_rect.y,
                        },
                        Vec2::new(paint_rect.width, paint_rect.height),
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
                        super::paint_layout_box(
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
                    let paint_rect = frag.paint_rect.rect();
                    let rect = Rect::from_min_size(
                        Pos2 {
                            x: origin.x + paint_rect.x,
                            y: origin.y + paint_rect.y,
                        },
                        Vec2::new(paint_rect.width, paint_rect.height),
                    );

                    if let Some(cache) = fragment_rects
                        && let Some(lb) = layout
                    {
                        cache
                            .borrow_mut()
                            .insert(lb.node_id(), frag.paint_rect.rect());
                    }

                    super::replaced::paint_replaced_fragment(rect, style, *kind, *layout, ctx);
                }
            }
        }
    }
}
