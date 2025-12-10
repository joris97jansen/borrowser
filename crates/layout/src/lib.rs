mod text;
pub use text::TextMeasurer;

pub mod inline;
pub use inline::{
        LineBox,
        refine_layout_with_inline,
        layout_inline_for_paint,
};

use css::{
    ComputedStyle,
    StyledNode,
    Display,
};
use html::dom_utils::is_non_rendering_element;


/// A rectangle in CSS px units (we'll treat everything as px for now).
#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// What kind of layout box this is. For now: only block.
#[derive(Clone, Copy, Debug)]
pub enum BoxKind {
    Block,
    Inline,
    InlineBlock,
    // Future: AnonymousBlock, ListItem, etc.
}

/// What kind of list marker this block has, if any.
#[derive(Clone, Copy, Debug)]
pub enum ListMarker {
    /// Bullet for unordered lists (<ul><li>).
    Unordered,
    /// Numbered marker for ordered lists (<ol><li>), 1-based.
    Ordered(u32),
}

/// A node in the layout tree:
/// - points to a styled node
/// - has a geometry rect
/// - has child layout boxes
pub struct LayoutBox<'a> {
    pub kind: BoxKind,
    pub style: &'a ComputedStyle,
    pub node: &'a StyledNode<'a>,
    pub rect: Rectangle,
    pub children: Vec<LayoutBox<'a>>,
    pub list_marker: Option<ListMarker>,
}

/// Compute block layout for a style tree.
/// - `root` is the style-tree root (usually the document node)
/// - `page_width` is the available content width in px
pub fn layout_block_tree<'a>(
    root: &'a StyledNode<'a>,
    page_width: f32,
) -> LayoutBox<'a> {
    let (root_box, _next_y) = layout_block_subtree(root, 0.0, 0.0, page_width);
    root_box
}

/// Internal recursive function:
/// - `x`, `y` = top-left of this box
/// - `width`  = available width
/// Returns: (this LayoutBox, next_y_cursor)
fn layout_block_subtree<'a>(
    styled: &'a StyledNode<'a>,
    x: f32,
    y: f32,
    width: f32,
) -> (LayoutBox<'a>, f32) {
    use html::Node;

    // 0) Non-rendering elements: act like transparent containers.
    // They do NOT get their own "row"; only their children do.
    if is_non_rendering_element(styled.node) {
        let mut children_boxes = Vec::new();
        let mut cursor_y = y;

        for child in &styled.children {
            let (child_box, new_y) = layout_block_subtree(child, x, cursor_y, width);
            cursor_y = new_y;
            children_boxes.push(child_box);
        }

        let height = if children_boxes.is_empty() {
            0.0
        } else {
            cursor_y - y
        };

        let rect = Rectangle { x, y, width, height };

        let layout_box = LayoutBox {
            kind: BoxKind::Block,
            style: &styled.style,
            node: styled,
            rect,
            children: children_boxes,
            list_marker: None,
        };

        let next_y = y + height;
        return (layout_box, next_y);
    }

    // 1) Normal block layout logic
    let style = &styled.style;
    let mut children_boxes = Vec::new();

    // Where children start and how tall *we* are by default.
    let (content_start_y, mut cursor_y, base_height) = match styled.node {
        // Document: no own “row”, just a container for children.
        Node::Document { .. } => {
            let content_y = y;
            (content_y, content_y, 0.0)
        }

        // Special-case <html>: also no base row; it’s just the top container.
        Node::Element { name, .. } if name.eq_ignore_ascii_case("html") => {
            let content_y = y;
            (content_y, content_y, 0.0)
        }

        // Normal elements: base height = CSS line-height derived from font-size.
        Node::Element { .. } => {
            let base = line_height_from(style);
            let content_y = y + base;
            (content_y, content_y, base)
        }

        // Text / Comment: inline content → no own block height.
        _ => {
            let base = 0.0;
            let content_y = y;
            (content_y, content_y, base)
        }
    };

    // Lay out children vertically
    if matches!(styled.node, Node::Document { .. } | Node::Element { .. }) {
        // Detect whether this element is a <ul> or <ol>.
        let (is_ul, is_ol) = match styled.node {
            html::Node::Element { name, .. } => {
                let is_ul = name.eq_ignore_ascii_case("ul");
                let is_ol = name.eq_ignore_ascii_case("ol");
                (is_ul, is_ol)
            }
            _ => (false, false),
        };

        // For <ol>, we number <li> children starting at 1.
        let mut next_ol_index: u32 = 1;

        for child in &styled.children {
            let (mut child_box, new_cursor) = layout_block_subtree(child, x, cursor_y, width);

            // If this is a list container (<ul> / <ol>) and the child is a list-item,
            // assign a marker.
            if let html::Node::Element { .. } = child.node {
                if child_box.style.display == Display::ListItem {
                    if is_ul {
                        // <ul><li> → bullet
                        child_box.list_marker = Some(ListMarker::Unordered);
                    } else if is_ol {
                        // <ol><li> → numbered, 1-based
                        child_box.list_marker = Some(ListMarker::Ordered(next_ol_index));
                        next_ol_index += 1;
                    }
                }
            }

            cursor_y = new_cursor;
            children_boxes.push(child_box);
        }
    }

    let children_height = if children_boxes.is_empty() {
        0.0
    } else {
        cursor_y - content_start_y
    };

    let mut height = base_height + children_height;

    // If this is an Element and everything somehow ended up as 0,
    // fall back to its line-height-based base (defensive).
    if height <= 0.0 {
        if matches!(styled.node, Node::Element { .. }) {
            height = line_height_from(style);
        } else {
            height = 0.0;
        }
    }

    let rect = Rectangle { x, y, width, height };

    // Decide box kind based on computed display.
    // For now, we only distinguish Block vs Inline.
    let kind = match styled.node {
        // Document and <html> act as block-level containers.
        html::Node::Document { .. } => BoxKind::Block,
        html::Node::Element { name, .. } if name.eq_ignore_ascii_case("html") => BoxKind::Block,

        // Text / Comment nodes already get 0 height; keep them as Block for now.
        html::Node::Text { .. } | html::Node::Comment { .. } => BoxKind::Block,

        // All other elements: look at display.
        _ => {
            match style.display {
                Display::Inline => BoxKind::Inline,
                Display::InlineBlock => BoxKind::InlineBlock,
                _ => BoxKind::Block,
            }
        }
    };

    let layout_box = LayoutBox {
        kind,
        style,
        node: styled,
        rect,
        children: children_boxes,
        list_marker: None,
    };

    let next_y = y + height;
    (layout_box, next_y)
}

fn line_height_from(style: &ComputedStyle) -> f32 {
    match style.font_size {
        css::Length::Px(px) => px * 1.2, // same factor as inline layout
    }
}