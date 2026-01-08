use css::{ComputedStyle, Display, Length};
use html::Node;
use html::dom_utils::is_non_rendering_element;

use crate::{BoxKind, LayoutBox, Rectangle, TextMeasurer, content_x_and_width, content_y};

use super::layout_tokens;
use super::options::INLINE_PADDING;
use super::replaced::size_replaced_inline_children;
use super::tokens::collect_inline_tokens_for_block_layout;

pub fn refine_layout_with_inline<'a>(measurer: &dyn TextMeasurer, layout_root: &mut LayoutBox<'a>) {
    let x = layout_root.rect.x;
    let y = layout_root.rect.y;
    let width = layout_root.rect.width;

    let new_height = recompute_block_heights(measurer, layout_root, x, y, width);
    layout_root.rect.height = new_height;
}

fn recompute_block_heights<'a>(
    measurer: &dyn TextMeasurer,
    node: &mut LayoutBox<'a>,
    x: f32,
    y: f32,
    available_width: f32,
) -> f32 {
    // Position & width are authoritative here
    node.rect.x = x;
    node.rect.y = y;

    let used_width =
        resolve_used_width_for_block(node.style, node.node.node, node.kind, available_width);
    node.rect.width = used_width;

    // Non-rendering elements: pure containers (but children still have margins)
    if is_non_rendering_element(node.node.node) {
        let mut cursor_y = y;

        let parent_x = x;
        let parent_width = used_width;

        for child in &mut node.children {
            let bm = child.style.box_metrics;

            // Space before child
            cursor_y += bm.margin_top;

            let child_x = parent_x + bm.margin_left;
            let child_width = (parent_width - bm.margin_left - bm.margin_right).max(0.0);

            let h = recompute_block_heights(measurer, child, child_x, cursor_y, child_width);

            // Move cursor past the child box
            cursor_y += h + bm.margin_bottom;
        }

        let height = cursor_y - y;
        node.rect.height = height;
        return height;
    }

    match node.node.node {
        Node::Document { .. } => {
            let mut cursor_y = y;

            let parent_x = x;
            let parent_width = used_width;

            for child in &mut node.children {
                let bm = child.style.box_metrics;

                cursor_y += bm.margin_top;

                let child_x = parent_x + bm.margin_left;
                let child_width = (parent_width - bm.margin_left - bm.margin_right).max(0.0);

                let h = recompute_block_heights(measurer, child, child_x, cursor_y, child_width);
                cursor_y += h + bm.margin_bottom;
            }

            let height = cursor_y - y;
            node.rect.height = height;
            height
        }

        Node::Element { name, .. } => {
            // <html> acts as pure container (no own row)
            if name.eq_ignore_ascii_case("html") {
                let mut cursor_y = y;

                let parent_x = x;
                let parent_width = used_width;

                // Inline elements: height is 0 at block level.
                if matches!(node.style.display, Display::Inline) {
                    let (content_x, content_width) = content_x_and_width(node.style, x, used_width);
                    let content_top = content_y(node.style, y);

                    size_replaced_inline_children(
                        measurer,
                        node,
                        content_x,
                        content_top,
                        content_width,
                    );

                    node.rect.height = 0.0;
                    return 0.0;
                }

                for child in &mut node.children {
                    let bm = child.style.box_metrics;

                    cursor_y += bm.margin_top;

                    let child_x = parent_x + bm.margin_left;
                    let child_width = (parent_width - bm.margin_left - bm.margin_right).max(0.0);

                    let h =
                        recompute_block_heights(measurer, child, child_x, cursor_y, child_width);
                    cursor_y += h + bm.margin_bottom;
                }

                let height = cursor_y - y;
                node.rect.height = height;
                return height;
            }

            // --- Block-level element: inline content + block children + padding ---

            let bm = node.style.box_metrics;

            // Content box horizontally: inside padding-left/right
            let (content_x, content_width) = content_x_and_width(node.style, x, used_width);

            // Content box top (used as the baseline for inline layout)
            let content_top = content_y(node.style, y);

            // 1) Layout inline-block children so we know their sizes.
            size_replaced_inline_children(measurer, node, content_x, content_top, content_width);

            {
                for child in &mut node.children {
                    if matches!(child.kind, BoxKind::InlineBlock) {
                        let cbm = child.style.box_metrics;

                        // Horizontal position as if it lived in the content box.
                        let child_x = content_x + cbm.margin_left;
                        let child_width =
                            (content_width - cbm.margin_left - cbm.margin_right).max(0.0);

                        // Vertically, for now we place them starting at content_top;
                        // the inline engine will decide their final visual y position.
                        let child_y = content_top + cbm.margin_top;

                        let _ =
                            recompute_block_heights(measurer, child, child_x, child_y, child_width);
                    }
                }
            }

            // 2) Inline content (text + inline-block boxes) via the inline engine,
            //    using layout-based inline token enumeration in DOM order.
            let mut inline_height = 0.0;

            {
                // Collect inline tokens directly from the layout tree, in DOM order.
                let tokens = collect_inline_tokens_for_block_layout(node);

                if !tokens.is_empty() {
                    // Give the inline layout a "tall enough" rectangle; it will
                    // early-out if we run out of vertical space.
                    let huge_height = 1_000_000.0;

                    // Inline content lives entirely inside the content box.
                    let block_rect = Rectangle {
                        x: content_x,
                        y: content_top,
                        width: content_width,
                        height: huge_height,
                    };

                    let lines = layout_tokens(measurer, block_rect, node.style, tokens);

                    if let Some(last) = lines.last() {
                        let last_bottom = last.rect.y + last.rect.height;
                        // height of all lines, measured from the top of our content box.
                        inline_height = (last_bottom - content_top) + INLINE_PADDING;
                    }
                }
            }

            // Fallback: at least one line-height even if no inline content at all
            if inline_height <= 0.0 {
                inline_height = measurer.line_height(node.style);
            }

            // 3) Block children start below content_top + inline content
            let content_start_y = content_top + inline_height;
            let mut cursor_y = content_start_y;

            for child in &mut node.children {
                // Skip inline, inline-block & replaced-inline children here; we already
                // accounted for them in the inline formatting context.
                if matches!(
                    child.kind,
                    BoxKind::Inline | BoxKind::InlineBlock | BoxKind::ReplacedInline
                ) {
                    continue;
                }

                let cbm = child.style.box_metrics;

                // Child's margin-top
                cursor_y += cbm.margin_top;

                let child_x = content_x + cbm.margin_left;
                let child_width = (content_width - cbm.margin_left - cbm.margin_right).max(0.0);

                let h = recompute_block_heights(measurer, child, child_x, cursor_y, child_width);

                // Move down by child's height + margin-bottom
                cursor_y += h + cbm.margin_bottom;
            }

            let children_height = cursor_y - content_start_y;

            // 4) Total height = padding-top + inline + children + padding-bottom
            let total_height = bm.padding_top + inline_height + children_height + bm.padding_bottom;

            node.rect.height = total_height;
            total_height
        }

        // Text / Comment nodes: no own block height
        Node::Text { .. } | Node::Comment { .. } => {
            node.rect.height = 0.0;
            0.0
        }
    }
}

fn resolve_used_width_for_block(
    style: &ComputedStyle,
    node: &html::Node,
    kind: BoxKind,
    available_width: f32,
) -> f32 {
    // 1) Start from available width.
    let mut w = available_width.max(0.0);

    // 2) Apply explicit width for non-inline elements.
    if let html::Node::Element { .. } = node {
        if let (false, Some(Length::Px(px))) = (
            matches!(style.display, Display::Inline),
            style
                .width
                .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0)),
        ) {
            w = px;
        }

        // Naïve shrink-to-fit **only** for inline-block:
        //
        // - If width was specified, we keep it but clamp to available_width.
        // - If width was not specified, we just keep the "fill available" default.
        // - In both cases we cap at available_width to avoid horizontal overflow.
        if matches!(kind, BoxKind::InlineBlock) {
            w = w.min(available_width.max(0.0));
        }
    }

    // 3) Apply min-width / max-width (px-only).
    if let Some(Length::Px(min_px)) = style
        .min_width
        .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
    {
        w = w.max(min_px);
    }

    if let Some(Length::Px(max_px)) = style
        .max_width
        .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
    {
        w = w.min(max_px);
    }

    // 4) FINAL clamp for inline-block (naïve shrink-to-fit)
    if matches!(kind, BoxKind::InlineBlock) {
        w = w.min(available_width.max(0.0));
    }

    // Final safety: never negative.
    w.max(0.0)
}
