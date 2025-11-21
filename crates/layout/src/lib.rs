use css::{ComputedStyle, StyledNode};
use html::Node;

const DEFAULT_BLOCK_HEIGHT: f32 = 24.0; // temporary until we have text metrics


/// A rectangle in CSS px units (we'll treat everything as px for now).
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// What kind of layout box this is. For now: only block.
#[derive(Clone, Copy, Debug)]
pub enum BoxKind {
    Block,
    // Future: Inline, AnonymousBlock, InlineBlock, etc.
}

/// A node in the layout tree:
/// - points to a styled node
/// - has a geometry rect
/// - has child layout boxes
pub struct LayoutBox<'a> {
    pub kind: BoxKind,
    pub style: &'a ComputedStyle,
    pub node: &'a StyledNode<'a>,
    pub rect: Rect,
    pub children: Vec<LayoutBox<'a>>,
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

        // Normal elements: get a base “row” of DEFAULT_BLOCK_HEIGHT.
        Node::Element { .. } => {
            let base = DEFAULT_BLOCK_HEIGHT;
            let content_y = y + base;
            (content_y, content_y, base)
        }

        // Text / Comment: treat as leaf with a base height.
        _ => {
            let base = DEFAULT_BLOCK_HEIGHT;
            let content_y = y + base;
            (content_y, content_y, base)
        }
    };

    // Lay out children vertically inside our content area
    if matches!(styled.node, Node::Document { .. } | Node::Element { .. }) {
        for child in &styled.children {
            let (child_box, new_cursor) = layout_block_subtree(child, x, cursor_y, width);
            cursor_y = new_cursor;
            children_boxes.push(child_box);
        }
    }

    // Height contributed by children
    let children_height = if children_boxes.is_empty() {
        0.0
    } else {
        cursor_y - content_start_y
    };

    // Our total height = "own row" + children.
    let mut height = base_height + children_height;
    if height <= 0.0 {
        height = DEFAULT_BLOCK_HEIGHT;
    }

    let rect = Rect { x, y, width, height };

    let layout_box = LayoutBox {
        kind: BoxKind::Block,
        style,
        node: styled,
        rect,
        children: children_boxes,
    };

    // Next sibling starts below us.
    let next_y = y + height;

    (layout_box, next_y)
}