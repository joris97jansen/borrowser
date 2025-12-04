use css::{
    StyledNode,
    ComputedStyle,
    Length,
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
    pub style: &'a ComputedStyle,
    pub rect: Rect,
}

// A logical piece of inline content with a single style
pub struct InlineRun<'a> {
    pub text: String,
    style: &'a ComputedStyle,
}

// One line box: a horizontal slice of inline content.
pub struct LineBox<'a> {
    pub fragments: Vec<LineFragment<'a>>,
    pub rect: Rect,
}

// Internal token representation after whitespace processing.
// Not exported yet; only used inside this module.
enum InlineToken<'a> {
    Word { text: String, style: &'a ComputedStyle },
    Space { style: &'a ComputedStyle }, // a single collapsible space
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

fn tokenize_runs<'a>(runs: &[InlineRun<'a>]) -> Vec<InlineToken<'a>> {
    let mut tokens: Vec<InlineToken<'a>> = Vec::new();

    // This flag tracks "we saw one or more whitespace chars since the last token".
    // We *only* turn it into a real Space token when we see the next Word.
    let mut pending_space = false;

    for run in runs {
        let style = run.style;
        let mut current_word = String::new();

        for ch in run.text.chars() {
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
                pending_space = true;
            } else {
                // We’re about to start/continue a word.
                // If there was whitespace *and* we already have some tokens,
                // emit a single Space before this new word.
                if pending_space && !tokens.is_empty() {
                    tokens.push(InlineToken::Space { style });
                }
                pending_space = false;

                current_word.push(ch);
            }
        }

        // Flush last word in this run
        if !current_word.is_empty() {
            tokens.push(InlineToken::Word {
                text: current_word,
                style,
            });
            // note: pending_space remains as-is (if trailing spaces followed,
            // they would have set it inside the loop)
        }
    }

    // At the end we deliberately ignore pending_space:
    // trailing whitespace collapses away completely.
    tokens
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
    block_style: &'a ComputedStyle,
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
        let fake_style = ComputedStyle {
            font_size: Length::Px(font_px),
            ..*block_style
        };
        line_height = measurer.line_height(&fake_style);
    }

    let tokens = tokenize_runs(runs);

    let mut lines: Vec<LineBox<'a>> = Vec::new();
    let mut line_fragments: Vec<LineFragment<'a>> = Vec::new();

    let line_start_x = rect.x + padding;
    let mut cursor_x = line_start_x;
    let mut cursor_y = rect.y + padding;

    let max_x = rect.x + rect.width - padding;
    let bottom_limit = rect.y + padding + available_height;

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
                        return lines;
                    }

                    cursor_x = line_start_x;
                    is_first_in_line = true;
                    continue;
                }

                // Space fits on this line → add it as its own fragment
                let frag_rect = Rect {
                    x: cursor_x,
                    y: cursor_y,
                    width: space_width,
                    height: line_height,
                };

                line_fragments.push(LineFragment {
                    text: space_text.to_string(),
                    style,
                    rect: frag_rect,
                });

                cursor_x += space_width;
            }

            InlineToken::Word { text, style } => {
                let word_width = measurer.measure(&text, style);

                let fits = cursor_x + word_width <= max_x;

                if !fits && !is_first_in_line {
                    // Close current line and move word to next line
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
                        return lines;
                    }

                    cursor_x = line_start_x;
                }

                // Place the word (even if it is longer than the full line; we just let it overflow).
                let frag_rect = Rect {
                    x: cursor_x,
                    y: cursor_y,
                    width: word_width,
                    height: line_height,
                };

                line_fragments.push(LineFragment {
                    text,
                    style,
                    rect: frag_rect,
                });

                cursor_x += word_width;
                is_first_in_line = false;
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

    // Non-rendering elements: pure containers (but children still have margins)
    if is_non_rendering_element(node.node.node) {
        let mut cursor_y = y;

        for child in &mut node.children {
            let bm = child.style.box_metrics;

            // Space before child
            cursor_y += bm.margin_top;

            let h = recompute_block_heights(measurer, child, x, cursor_y, width);

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

            for child in &mut node.children {
                let bm = child.style.box_metrics;

                cursor_y += bm.margin_top;
                let h = recompute_block_heights(measurer, child, x, cursor_y, width);
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

                for child in &mut node.children {
                    let bm = child.style.box_metrics;

                    cursor_y += bm.margin_top;
                    let h = recompute_block_heights(measurer, child, x, cursor_y, width);
                    cursor_y += h + bm.margin_bottom;
                }

                let height = cursor_y - y;
                node.rect.height = height;
                return height;
            }

            // Inline elements: height is 0 at block level.
            if is_inline_element_name(name) {
                node.rect.height = 0.0;
                return 0.0;
            }

            // --- Block-level element: inline content + block children + padding ---
            //
            // We treat:
            //   border box: node.rect (x, y, width, height)
            //   content box: inside padding_*
            //   margins: handled by the parent when placing this node.

            let bm = node.style.box_metrics;

            // 1) Inline height from line boxes (inside the padding-top area)
            let mut inline_height = 0.0;
            let runs = collect_inline_runs_for_block(node.node);

            if !runs.is_empty() {
                let huge_height = 1_000_000.0;

                // Inline content lives inside padding-top (and later, padding-left/right).
                let block_rect = Rect {
                    x,                            // horizontal paddings are not applied yet
                    y: y + bm.padding_top,
                    width,
                    height: huge_height,
                };

                let lines = layout_inline_runs(measurer, block_rect, node.style, &runs);
                if let Some(last) =  lines.last() {
                    let last_bottom = last.rect.y + last.rect.height;
                    // height of all lines, measured from the top of our padding area
                    inline_height = (last_bottom - (y + bm.padding_top)) + INLINE_PADDING;
                }
            }

            // Fallback: at least one line-height even if no text
            if inline_height <= 0.0 {
                inline_height = measurer.line_height(node.style);
            }

            // 2) Block children start below padding-top + inline content
            let content_start_y = y + bm.padding_top + inline_height;
            let mut cursor_y = content_start_y;

            for child in &mut node.children {
                let cbm = child.style.box_metrics;

                // Child's margin-top
                cursor_y += cbm.margin_top;

                let h = recompute_block_heights(measurer, child, x, cursor_y, width);

                // Move down by child's height + margin-bottom
                cursor_y += h + cbm.margin_bottom;
            }

            let children_height = cursor_y - content_start_y;

            // 3) Total height = padding-top + inline + children + padding-bottom
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