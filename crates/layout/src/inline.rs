use css::{
    ComputedStyle,
    Length,
    Display,
};
use html::Node;
use html::dom_utils::is_non_rendering_element;

use crate::{
    Rectangle,
    TextMeasurer,
    LayoutBox,
    BoxKind,
    ReplacedKind,
    content_x_and_width,
    content_y,
};

const INLINE_PADDING: f32 = 4.0;

// Inline layout pipeline
//
// DOM → StyledNode → LayoutBox
//       ↓ (for inline blocks)
//   collect_inline_tokens_for_block_layout[ _for_paint ]
//       ↓
//   layout_tokens → Vec<LineBox<'a>> (with InlineFragment::Text / Box)
//
// Height uses:    collect_inline_tokens_for_block_layout
// Painting uses:  collect_inline_tokens_for_block_layout_for_paint


/// The logical content carried by a line fragment.
/// - `Text` is inline text
/// - `Box` is inline-level replaced/box content (e.g., inline-block)
pub enum InlineFragment<'a> {
    Text {
        text: String,
        style: &'a ComputedStyle,
    },
    Box {
        /// Style of the inline box (for color, etc.).
        style: &'a ComputedStyle,
        /// Layout box for this inline-level box, if we have one.
        /// - `Some(..)` in the painting path
        /// - `None` in the height computation path
        layout: Option<&'a LayoutBox<'a>>,
    },
    Replaced {
        style: &'a ComputedStyle,
        kind: ReplacedKind,
        layout: Option<&'a LayoutBox<'a>>, // usually None; future-proof (e.g. <button>)
    },
}

// One fragment of text within a line (later this can be per <span>, <a>, etc.)
pub struct LineFragment<'a> {
    pub kind: InlineFragment<'a>,
    pub rect: Rectangle,
}

// One line box: a horizontal slice of inline content.
pub struct LineBox<'a> {
    pub fragments: Vec<LineFragment<'a>>,
    pub rect: Rectangle,
}

// Internal token representation after whitespace processing.
// Not exported yet; only used inside this module.
enum InlineToken<'a> {
    Word { text: String, style: &'a ComputedStyle },
    Space { style: &'a ComputedStyle }, // a single collapsible space
    /// A "box token" representing an inline-level box (e.g. inline-block, image).
    /// Width/height are the box size in px.
    Box {
        width: f32,
        height: f32,
        style: &'a ComputedStyle,
        /// Layout box, if we want to paint its subtree later.
        layout: Option<&'a LayoutBox<'a>>,
    },
    Replaced {
        width: f32,
        height: f32,
        style: &'a ComputedStyle,
        kind: ReplacedKind,
        layout: Option<&'a LayoutBox<'a>>,
    },
}

fn push_text_as_tokens<'a>(
    text: &str,
    style: &'a ComputedStyle,
    tokens: &mut Vec<InlineToken<'a>>,
    pending_space: &mut bool,
) {
    let mut current_word = String::new();

    for ch in text.chars() {
        if ch.is_whitespace() {
            // End any current word.
            if !current_word.is_empty() {
                tokens.push(InlineToken::Word {
                    text: current_word.clone(),
                    style,
                });
                current_word.clear();
            }
            // Remember that we've seen whitespace; may become a Space later.
            *pending_space = true;
        } else {
            // We’re about to start/continue a word.
            // If there was whitespace *and* we already have some tokens,
            // emit a single Space before this new word.
            if *pending_space && !tokens.is_empty() {
                tokens.push(InlineToken::Space { style });
            }
            *pending_space = false;

            current_word.push(ch);
        }
    }

    // Flush last word in this text fragment.
    if !current_word.is_empty() {
        tokens.push(InlineToken::Word {
            text: current_word,
            style,
        });
    }

    // At the end we deliberately ignore pending_space:
    // trailing whitespace collapses away completely.
}

fn collect_inline_tokens_for_block_layout<'a>(
    block: &'a LayoutBox<'a>,
) -> Vec<InlineToken<'a>> {
    let mut tokens: Vec<InlineToken<'a>> = Vec::new();
    let mut pending_space = false;

    for child in &block.children {
        collect_inline_tokens_from_layout_box(child, &mut tokens, &mut pending_space);
    }

    tokens
}

fn collect_inline_tokens_from_layout_box<'a>(
    layout: &'a LayoutBox<'a>,
    tokens: &mut Vec<InlineToken<'a>>,
    pending_space: &mut bool,
) {
    match layout.node.node {
        Node::Text { text, .. } => {
            if text.is_empty() {
                return;
            }
            // Treat the text content as part of the current inline
            // formatting context using the same whitespace behavior
            // as tokenize_runs.
            push_text_as_tokens(text, layout.style, tokens, pending_space);
        }

        Node::Element { .. } | Node::Document { .. } | Node::Comment { .. } => {
            match layout.kind {
                BoxKind::Inline => {
                    // Inline container: recurse into children, they
                    // participate in the same inline formatting context.
                    for child in &layout.children {
                        collect_inline_tokens_from_layout_box(child, tokens, pending_space);
                    }
                }

                BoxKind::InlineBlock => {
                    // Inline-block: a single inline box. We do not
                    // descend into its children here.
                    //
                    // If there was pending whitespace, flush it as a
                    // single Space token before the box (like a word).
                    if *pending_space && !tokens.is_empty() {
                        let style = layout.style;
                        tokens.push(InlineToken::Space { style });
                        *pending_space = false;
                    }

                    let style = layout.style;
                    let cbm = layout.style.box_metrics;

                    let width = layout.rect.width;
                    let height = layout.rect.height + cbm.margin_top + cbm.margin_bottom;

                    tokens.push(InlineToken::Box {
                        width,
                        height,
                        style,
                        layout: None, // height computation path: no layout ref
                    });
                }

                BoxKind::Block => {
                    // Block descendants are separate block formatting
                    // contexts. They do not contribute to this block's
                    // inline content. We *do* treat any text nodes as
                    // inline content (handled above by Node::Text),
                    // but for Element/Document/Comment with Block kind
                    // we stop here.
                }

                BoxKind::ReplacedInline => {
                    if *pending_space && !tokens.is_empty() {
                        let style = layout.style;
                        tokens.push(InlineToken::Space { style });
                        *pending_space = false;
                    }

                    let style = layout.style;
                    let cbm = style.box_metrics;

                    let width  = layout.rect.width;
                    let height = layout.rect.height + cbm.margin_top + cbm.margin_bottom;

                    let kind = layout.replaced.expect("ReplacedInline must have replaced kind");

                    tokens.push(InlineToken::Replaced {
                        width,
                        height,
                        style,
                        kind,
                        layout: None, // height computation path: no layout ref
                    });
                }
            }
        }
    }
}

fn collect_inline_tokens_for_block_layout_for_paint<'a>(
    block: &'a LayoutBox<'a>,
) -> Vec<InlineToken<'a>> {
    let mut tokens: Vec<InlineToken<'a>> = Vec::new();
    let mut pending_space = false;

    for child in &block.children {
        collect_inline_tokens_from_layout_box_for_paint(child, &mut tokens, &mut pending_space);
    }

    tokens
}

fn collect_inline_tokens_from_layout_box_for_paint<'a>(
    layout: &'a LayoutBox<'a>,
    tokens: &mut Vec<InlineToken<'a>>,
    pending_space: &mut bool,
) {
    match layout.node.node {
        Node::Text { text, .. } => {
            if text.is_empty() {
                return;
            }
            // Same whitespace behavior as tokenize_runs / height path.
            push_text_as_tokens(text, layout.style, tokens, pending_space);
        }

        Node::Element { .. } | Node::Document { .. } | Node::Comment { .. } => {
            match layout.kind {
                BoxKind::Inline => {
                    // Inline container: recurse into children, they
                    // participate in the same inline formatting context.
                    for child in &layout.children {
                        collect_inline_tokens_from_layout_box_for_paint(
                            child,
                            tokens,
                            pending_space,
                        );
                    }
                }

                BoxKind::InlineBlock => {
                    // Inline-block: single inline box; do not descend.
                    //
                    // If there was pending whitespace, flush it as a Space
                    // token before the box (like a word).
                    if *pending_space && !tokens.is_empty() {
                        let style = layout.style;
                        tokens.push(InlineToken::Space { style });
                        *pending_space = false;
                    }

                    let style = layout.style;
                    let cbm = layout.style.box_metrics;

                    let width = layout.rect.width;
                    let height = layout.rect.height + cbm.margin_top + cbm.margin_bottom;

                    tokens.push(InlineToken::Box {
                        width,
                        height,
                        style,
                        layout: Some(layout), // painting path: keep a layout ref
                    });
                }

                BoxKind::Block => {
                    // Block descendants are separate block formatting contexts.
                    // They do not contribute to this block's inline content.
                }

                BoxKind::ReplacedInline => {
                    if *pending_space && !tokens.is_empty() {
                        let style = layout.style;
                        tokens.push(InlineToken::Space { style });
                        *pending_space = false;
                    }

                    let style = layout.style;
                    let cbm = style.box_metrics;

                    let width  = layout.rect.width;
                    let height = layout.rect.height + cbm.margin_top + cbm.margin_bottom;

                    let kind = layout.replaced.expect("ReplacedInline must have replaced kind");

                    tokens.push(InlineToken::Replaced {
                        width,
                        height,
                        style,
                        kind,
                        layout: Some(layout),
                    });
                }
            }
        }
    }
}

pub fn layout_inline_for_paint<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block: &'a LayoutBox<'a>,
) -> Vec<LineBox<'a>> {
    // Unified layout-based token enumeration (DOM order):
    // text + inline-block boxes from the layout tree.
    let tokens = collect_inline_tokens_for_block_layout_for_paint(block);

    if tokens.is_empty() {
        return Vec::new();
    }

    // Lay them out with the same generic token engine as the height path.
    layout_tokens(measurer, rect, block.style, tokens)
}

fn layout_tokens<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block_style: &'a ComputedStyle,
    tokens: Vec<InlineToken<'a>>,
) -> Vec<LineBox<'a>> {
    let padding = INLINE_PADDING;
    let available_height = rect.height - 2.0 * padding;

    // Base line height derived from the block's style.
    let mut base_line_height = measurer.line_height(block_style);

    // Simple clamp for extreme cases (same as before).
    if base_line_height > available_height && available_height > 0.0 {
        let font_px = (available_height / 1.2).max(8.0);
        let fake_style = ComputedStyle {
            font_size: Length::Px(font_px),
            ..*block_style
        };
        base_line_height = measurer.line_height(&fake_style);
    }

    let mut lines: Vec<LineBox<'a>> = Vec::new();
    let mut line_fragments: Vec<LineFragment<'a>> = Vec::new();

    let line_start_x = rect.x + padding;
    let mut cursor_x = line_start_x;
    let mut cursor_y = rect.y + padding;

    let max_x = rect.x + rect.width - padding;
    let bottom_limit = rect.y + padding + available_height;

    // Current line height; can grow if we see tall box fragments.
    let mut line_height = base_line_height;

    let mut is_first_in_line = true;

    for token in tokens {
        match token {
            InlineToken::Space { style } => {
                // Never show a space at the beginning of a line.
                if is_first_in_line {
                    continue;
                }

                let space_text = " ";
                let space_width = measurer.measure(space_text, style);

                // If the space itself doesn't fit, we break the line
                // and *drop* the space (no leading spaces).
                if cursor_x + space_width > max_x {
                    if !line_fragments.is_empty() {
                        let line_width = cursor_x - line_start_x;
                        let line_rect = Rectangle {
                            x: line_start_x,
                            y: cursor_y,
                            width: line_width,
                            height: line_height,
                        };

                        lines.push(LineBox {
                            rect: line_rect,
                            fragments: std::mem::take(&mut line_fragments),
                        });
                    }

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_height = base_line_height;
                    is_first_in_line = true;
                    continue;
                }

                let frag_rect = Rectangle {
                    x: cursor_x,
                    y: cursor_y,
                    width: space_width,
                    height: line_height,
                };

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Text {
                        text: space_text.to_string(),
                        style,
                    },
                    rect: frag_rect,
                });

                cursor_x += space_width;
            }

            InlineToken::Word { text, style } => {
                let word_width = measurer.measure(&text, style);

                let fits = cursor_x + word_width <= max_x;

                if !fits && !is_first_in_line {
                    if !line_fragments.is_empty() {
                        let line_width = cursor_x - line_start_x;
                        let line_rect = Rectangle {
                            x: line_start_x,
                            y: cursor_y,
                            width: line_width,
                            height: line_height,
                        };

                            lines.push(LineBox {
                            rect: line_rect,
                            fragments: std::mem::take(&mut line_fragments),
                        });
                    }

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_height = base_line_height;
                    is_first_in_line = true;
                }

                let frag_rect = Rectangle {
                    x: cursor_x,
                    y: cursor_y,
                    width: word_width,
                    height: line_height,
                };

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Text { text, style },
                    rect: frag_rect,
                });

                cursor_x += word_width;
                is_first_in_line = false;
            }

            InlineToken::Box { width: box_width, height: box_height, style, layout } => {
                let fits = cursor_x + box_width <= max_x;

                if !fits && !is_first_in_line {
                    if !line_fragments.is_empty() {
                        let line_width = cursor_x - line_start_x;
                        let line_rect = Rectangle {
                            x: line_start_x,
                            y: cursor_y,
                            width: line_width,
                            height: line_height,
                        };

                        lines.push(LineBox {
                            rect: line_rect,
                            fragments: std::mem::take(&mut line_fragments),
                        });
                    }

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_height = base_line_height;
                }

                // Box behaves like a big glyph: it can grow the line height.
                if box_height > line_height {
                    line_height = box_height;
                }

                let frag_rect = Rectangle {
                    x: cursor_x,
                    y: cursor_y,
                    width: box_width,
                    height: box_height,
                };

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Box { style, layout },
                    rect: frag_rect,
                });

                cursor_x += box_width;
                is_first_in_line = false;
            }

            InlineToken::Replaced { width, height, style, kind, layout } => {
                let fits = cursor_x + width <= max_x;

                if !fits && !is_first_in_line {
                    if !line_fragments.is_empty() {
                        let line_width = cursor_x - line_start_x;
                        let line_rect = Rectangle {
                            x: line_start_x,
                            y: cursor_y,
                            width: line_width,
                            height: line_height,
                        };

                        lines.push(LineBox {
                            rect: line_rect,
                            fragments: std::mem::take(&mut line_fragments),
                        });
                    }

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_height = base_line_height;
                }

                if height > line_height {
                    line_height = height;
                }

                let frag_rect = Rectangle { x: cursor_x, y: cursor_y, width, height };

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Replaced { style, kind, layout },
                    rect: frag_rect,
                });

                cursor_x += width;
                is_first_in_line = false;
            }
        }
    }

    // Flush the last line
    if !line_fragments.is_empty() && cursor_y + line_height <= bottom_limit {
        let line_width = cursor_x - line_start_x;
        let line_rect = Rectangle {
            x: line_start_x,
            y: cursor_y,
            width: line_width,
            height: line_height,
        };

        lines.push(LineBox {
            rect: line_rect,
            fragments: line_fragments,
        });
    }

    lines
}

pub fn refine_layout_with_inline<'a>(
    measurer: &dyn TextMeasurer,
    layout_root: &mut LayoutBox<'a>,
) {
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

    let used_width = resolve_used_width_for_block(node.style, &node.node.node, node.kind, available_width);
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

                    size_replaced_inline_children(measurer, node, content_x, content_top, content_width);

                    node.rect.height = 0.0;
                    return 0.0;
                }

                for child in &mut node.children {
                    let bm = child.style.box_metrics;

                    cursor_y += bm.margin_top;

                    let child_x = parent_x + bm.margin_left;
                    let child_width =
                        (parent_width - bm.margin_left - bm.margin_right).max(0.0);

                    let h = recompute_block_heights(measurer, child, child_x, cursor_y, child_width);
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

                        let _ = recompute_block_heights(
                            measurer,
                            child,
                            child_x,
                            child_y,
                            child_width,
                        );
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
                if matches!(child.kind, BoxKind::Inline | BoxKind::InlineBlock | BoxKind::ReplacedInline) {
                    continue;
                }

                let cbm = child.style.box_metrics;

                // Child's margin-top
                cursor_y += cbm.margin_top;

                let child_x = content_x + cbm.margin_left;
                let child_width =
                    (content_width - cbm.margin_left - cbm.margin_right).max(0.0);

                let h = recompute_block_heights(measurer, child, child_x, cursor_y, child_width);

                // Move down by child's height + margin-bottom
                cursor_y += h + cbm.margin_bottom;
            }

            let children_height = cursor_y - content_start_y;

            // 4) Total height = padding-top + inline + children + padding-bottom
            let total_height =
                bm.padding_top + inline_height + children_height + bm.padding_bottom;

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
        if !matches!(style.display, Display::Inline) {
            if let Some(Length::Px(px)) = style.width {
                if px >= 0.0 {
                    w = px;
                }
            }
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
    if let Some(Length::Px(min_px)) = style.min_width {
        if min_px >= 0.0 {
            w = w.max(min_px);
        }
    }

    if let Some(Length::Px(max_px)) = style.max_width {
        if max_px >= 0.0 {
            w = w.min(max_px);
        }
    }

    // 4) FINAL clamp for inline-block (naïve shrink-to-fit)
    if matches!(kind, BoxKind::InlineBlock) {
        w = w.min(available_width.max(0.0));
    }

    // Final safety: never negative.
    w.max(0.0)
}

fn default_intrinsic_size(kind: ReplacedKind) -> (f32, f32) {
    match kind {
        ReplacedKind::Img => (300.0, 150.0),       // classic HTML fallback
        ReplacedKind::InputText => (160.0, 24.0),  // reasonable default
        ReplacedKind::Button => (80.0, 28.0),
    }
}

fn default_intrinsic_img_size() -> (f32, f32) {
    (300.0, 150.0)
}

fn resolve_replaced_width_px(
    style: &ComputedStyle,
    available_width: f32,
    intrinsic_width: f32,
) -> f32 {
    let mut w = intrinsic_width.max(0.0);

    // CSS width wins (px-only in Phase 1)
    if let Some(Length::Px(px)) = style.width {
        if px >= 0.0 {
            w = px;
        }
    }

    // Clamp with min/max-width (px-only)
    if let Some(Length::Px(min_px)) = style.min_width {
        if min_px >= 0.0 {
            w = w.max(min_px);
        }
    }
    if let Some(Length::Px(max_px)) = style.max_width {
        if max_px >= 0.0 {
            w = w.min(max_px);
        }
    }

    // Final clamp to available inline space
    w = w.min(available_width.max(0.0));

    w.max(0.0)
}

fn resolve_replaced_height_px(
    style: &ComputedStyle,
    intrinsic_height: f32,
) -> f32 {
    let mut h = intrinsic_height.max(0.0);

    if let Some(Length::Px(px)) = style.height {
        if px >= 0.0 {
            h = px;
        }
    }

    h.max(0.0)
}

fn get_attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match node {
        html::Node::Element { attributes, .. } => {
            for (k, v) in attributes {
                if k.eq_ignore_ascii_case(name) {
                    return v.as_deref();
                }
            }
            None
        }
        _ => None,
    }
}

fn size_replaced_inline_children<'a>(
    measurer: &dyn TextMeasurer,
    parent: &mut LayoutBox<'a>,
    content_x: f32,
    content_top: f32,
    content_width: f32,
) {
    for child in &mut parent.children {
        if !matches!(child.kind, BoxKind::ReplacedInline) {
            continue;
        }

        let cbm = child.style.box_metrics;
        let child_x = content_x + cbm.margin_left;
        let child_y = content_top + cbm.margin_top;

        child.rect.x = child_x;
        child.rect.y = child_y;

        let kind = child.replaced.expect("ReplacedInline must have kind");

        match kind {
            ReplacedKind::Img => {
                let (intr_w, intr_h) = default_intrinsic_img_size();

                let w = resolve_replaced_width_px(child.style, content_width, intr_w);
                let h = resolve_replaced_height_px(child.style, intr_h);

                child.rect.width = w;
                child.rect.height = h;
            }
            ReplacedKind::InputText => {
                // 1) Determine intrinsic base width from `size` attribute.
                //    HTML default size is commonly 20 for text inputs.
                let size_chars: u32 = get_attr(child.node.node, "size")
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .filter(|n| *n > 0)
                    .unwrap_or(20);

                // Average char width: measure "0" or "M" in current font.
                // "0" tends to be stable across fonts; "M" often overestimates.
                let avg_char_w = measurer.measure("0", child.style).max(4.0);

                // Add a little extra so text isn't flush even if padding is 0.
                let fudge = 8.0;

                let intrinsic_w = (size_chars as f32) * avg_char_w + fudge;

                // 2) Resolve width:
                //    CSS width wins; else intrinsic; then min/max; clamp to available.
                let w = resolve_replaced_width_px(child.style, content_width, intrinsic_w);

                // 3) Resolve height:
                //    derive from line height + vertical padding, with a sane minimum.
                let bm = child.style.box_metrics;
                let line_h = measurer.line_height(child.style);
                let pad_y = (bm.padding_top + bm.padding_bottom).max(4.0);

                let mut h = (line_h + pad_y).max(18.0);

                if let Some(Length::Px(px)) = child.style.height {
                    if px >= 0.0 {
                        h = px;
                    }
                }

                child.rect.width = w;
                child.rect.height = h;
            }
            _ => {
                // Phase 1: ignore other replaced kinds here
                // (they'll get their own sizing rules in their issues)
                let (intr_w, intr_h) = default_intrinsic_size(kind);
                let w = resolve_replaced_width_px(child.style, content_width, intr_w);
                let h = resolve_replaced_height_px(child.style, intr_h);

                child.rect.width = w;
                child.rect.height = h;
            }
        }
    }
}
