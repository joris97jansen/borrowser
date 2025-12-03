use css::{
    StyledNode,
};
use html::Node;
use html::dom_utils::is_non_rendering_element;

use crate::{
    Rect,
    TextMeasurer,
    LayoutBox,
};

const INLINE_PADDING: f32 = 4.0;


pub fn is_inline_element_name(name: &str) -> bool {
    // This is a *starting* set; we can expand as needed.
    name.eq_ignore_ascii_case("span")
        || name.eq_ignore_ascii_case("a")
        || name.eq_ignore_ascii_case("em")
        || name.eq_ignore_ascii_case("strong")
        || name.eq_ignore_ascii_case("b")
        || name.eq_ignore_ascii_case("i")
        || name.eq_ignore_ascii_case("u")
        || name.eq_ignore_ascii_case("small")
        || name.eq_ignore_ascii_case("big")
        || name.eq_ignore_ascii_case("code")
}

// One fragment of text within a line (later this can be per <span>, <a>, etc.)
pub struct LineFragment<'a> {
    pub text: String,
    pub style: &'a css::ComputedStyle,
    pub rect: Rect,
}

// A logical piece of inline content with a single style
pub struct InlineRun<'a> {
    pub text: String,
    style: &'a css::ComputedStyle,
}

// One line box: a horizontal slice of inline content.
pub struct LineBox<'a> {
    pub fragments: Vec<LineFragment<'a>>,
    pub rect: Rect,
}

pub fn collect_inline_runs_for_block<'a>(block: &'a StyledNode<'a>) -> Vec<InlineRun<'a>> {
    let mut runs = Vec::new();

    match block.node {
        Node::Element { .. } | Node::Document { .. } => {
            for child in &block.children {
                collect_inline_runs_desc(child, &mut runs);
            }
        }
        _ => {
            // For text/comment root we do nothing; blocks are Elements/Document.
        }
    }

    runs
}

fn collect_inline_runs_desc<'a>(styled: &'a StyledNode<'a>, out: &mut Vec<InlineRun<'a>>) {
    match styled.node {
        Node::Text { text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push(InlineRun {
                    // Keep original contents; we’ll handle spaces in layout
                    text: text.clone(),
                    style: &styled.style,
                });
            }
        }

        Node::Element { name, .. } => {
            if is_inline_element_name(name) {
                // Inline element → dive into children; they inherit/override style.
                for child in &styled.children {
                    collect_inline_runs_desc(child, out);
                }
            } else {
                // Block element → DO NOT descend.
                // This subtree will be handled by its own LayoutBox in a separate paint pass.
            }
        }

        Node::Document { .. } | Node::Comment { .. } => {
            for child in &styled.children {
                collect_inline_runs_desc(child, out);
            }
        }
    }
}

pub fn layout_inline_runs<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rect,
    block_style: &'a css::ComputedStyle,
    runs: &[InlineRun<'a>],
) -> Vec<LineBox<'a>> {
    let padding = INLINE_PADDING;
    let available_height = rect.height - 2.0 * padding;

    let mut line_height = measurer.line_height(block_style);

    if line_height > available_height && available_height > 0.0 {
        // Simple clamp: keep at least 8px, but don’t overflow insanely.
        let font_px = (available_height / 1.2).max(8.0);
        // Recompute line height from the adjusted font size
        // (reuse the same rule the measurer uses)
        let fake_style = css::ComputedStyle {
            font_size: css::Length::Px(font_px),
            ..*block_style
        };
        line_height = measurer.line_height(&fake_style);
    }

    let mut lines: Vec<LineBox<'a>> = Vec::new();
    let mut line_fragments: Vec<LineFragment<'a>> = Vec::new();

    let line_start_x = rect.x + padding;
    let mut cursor_x = line_start_x;
    let mut cursor_y = rect.y + padding;

    let max_x = rect.x + rect.width - padding;
    let bottom_limit = rect.y + padding + available_height;

    for run in runs {
        for word in run.text.split_whitespace() {
            // Are we at the start of the current line?
            let line_empty = line_fragments.is_empty();

            let text_piece = if line_empty {
                word.to_string()
            } else {
                format!(" {}", word)
            };

            let word_width = measurer.measure(&text_piece, run.style);

            let fits = cursor_x + word_width <= max_x;

            if !fits && !line_empty {
                // --- Wrap: close current line, move to next ---

                if !line_fragments.is_empty() {
                    let line_width = cursor_x - line_start_x;
                    let line_rect = Rect {
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
                if cursor_y + line_height > bottom_limit {
                    // No more vertical space in this block
                    return lines;
                }

                cursor_x = line_start_x;

                // Place the same word at the start of the new line (no leading space)
                let text_piece = word.to_string();
                let word_width = measurer.measure(&text_piece, run.style);

                let frag_rect = Rect {
                    x: cursor_x,
                    y: cursor_y,
                    width: word_width,
                    height: line_height,
                };

                line_fragments.push(LineFragment {
                    text: text_piece,
                    style: run.style,
                    rect: frag_rect,
                });

                cursor_x += word_width;
            } else {
                // --- Fits (or it's the very first word on an empty line) ---

                let frag_rect = Rect {
                    x: cursor_x,
                    y: cursor_y,
                    width: word_width,
                    height: line_height,
                };

                line_fragments.push(LineFragment {
                    text: text_piece,
                    style: run.style,
                    rect: frag_rect,
                });

                cursor_x += word_width;
            }
        }
    }

    // Flush the last line
    if !line_fragments.is_empty() && cursor_y + line_height <= bottom_limit {
        let line_width = cursor_x - line_start_x;
        let line_rect = Rect {
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
    width: f32,
) -> f32 {
    // Position & width are authoritative here
    node.rect.x = x;
    node.rect.y = y;
    node.rect.width = width;

    // Non-rendering elements: pure containers
    if is_non_rendering_element(node.node.node) {
        let mut cursor_y = y;
        for child in &mut node.children {
            let h = recompute_block_heights(measurer, child, x, cursor_y, width);
            cursor_y += h;
        }
        let height = cursor_y - y;
        node.rect.height = height;
        return height;
    }

    match node.node.node {
        Node::Document { .. } => {
            let mut cursor_y = y;
            for child in &mut node.children {
                let h = recompute_block_heights(measurer, child, x, cursor_y, width);
                cursor_y += h;
            }
            let height = cursor_y - y;
            node.rect.height = height;
            height
        }

        Node::Element { name, .. } => {
            // <html> acts as pure container (no own row)
            if name.eq_ignore_ascii_case("html") {
                let mut cursor_y = y;
                for child in &mut node.children {
                    let h = recompute_block_heights(measurer, child, x, cursor_y, width);
                    cursor_y += h;
                }
                let height = cursor_y - y;
                node.rect.height = height;
                return height;
            }

            // Inline elements do not generate a separate block height here.
            // Their text is handled by the nearest block ancestor via inline layout.
            if is_inline_element_name(name) {
                node.rect.height = 0.0;
                return 0.0;
            }

            // --- Block-level element: inline content + block children ---

            // 1) Compute inline height using a measurement pass for this block
            let runs = collect_inline_runs_for_block(node.node);
            let mut inline_height = 0.0;

            if !runs.is_empty() {
                let huge_height = 1_000_000.0;
                let block_rect = Rect {
                    x,
                    y,
                    width,
                    height: huge_height,
                };

                let lines = layout_inline_runs(measurer, block_rect, node.style, &runs);
                if let Some(last) = lines.last() {
                    let last_bottom = last.rect.y + last.rect.height;
                    inline_height = (last_bottom - y) + INLINE_PADDING;
                }
            }

            // Fallback: at least one line-height even if no text
            if inline_height <= 0.0 {
                inline_height = measurer.line_height(node.style);
            }

            // 2) Block children start below the inline content
            let mut cursor_y = y + inline_height;
            for child in &mut node.children {
                let h = recompute_block_heights(measurer, child, x, cursor_y, width);
                cursor_y += h;
            }

            let children_height = cursor_y - (y + inline_height);
            let total_height = inline_height + children_height;

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