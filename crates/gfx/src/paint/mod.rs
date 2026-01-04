mod context;
mod images;
mod inline;
mod replaced;
mod text_control;

pub(crate) use context::PaintCtx;
pub use images::{ImageProvider, ImageState};

use crate::EguiTextMeasurer;
use crate::input::{ActiveTarget, InputValueStore, TextareaCachedLine};
use css::{Display, Length};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use html::{Id, dom_utils::is_non_rendering_element};
use layout::{BoxKind, LayoutBox, ListMarker, Rectangle, TextMeasurer};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct PaintArgs<'a> {
    pub painter: &'a Painter,
    pub origin: Pos2,
    pub measurer: &'a EguiTextMeasurer,
    pub base_url: Option<&'a str>,
    pub resources: &'a dyn ImageProvider,
    pub input_values: &'a InputValueStore,
    pub focused: Option<Id>,
    pub focused_textarea_lines: Option<&'a [TextareaCachedLine]>,
    pub active: Option<ActiveTarget>,
    pub selection_bg_fill: Color32,
    pub selection_stroke: Stroke,
    pub fragment_rects: Option<&'a RefCell<HashMap<Id, Rectangle>>>,
}

fn paint_layout_box<'a>(
    layout: &LayoutBox<'a>,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    let painter = ctx.painter;
    let origin = ctx.origin;
    let measurer = ctx.measurer;

    // Non-rendering elements (e.g. <head>, <style>, <script>) suppress painting for the entire subtree.
    // Layout/style should prevent paintable boxes from existing here.
    if is_non_rendering_element(layout.node.node) {
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
    inline::paint_inline_content(layout, ctx);

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

pub fn paint_page<'a>(layout_root: &LayoutBox<'a>, args: PaintArgs<'_>) {
    let ctx = PaintCtx {
        painter: args.painter,
        origin: args.origin,
        measurer: args.measurer,
        base_url: args.base_url,
        resources: args.resources,
        input_values: args.input_values,
        focused: args.focused,
        focused_textarea_lines: args.focused_textarea_lines,
        active: args.active,
        selection_bg_fill: args.selection_bg_fill,
        selection_stroke: args.selection_stroke,
        fragment_rects: args.fragment_rects,
    };

    paint_layout_box(layout_root, ctx, true);
}
