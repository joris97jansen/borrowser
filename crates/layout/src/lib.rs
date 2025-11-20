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
/// - `viewport_width` is the available content width in px
pub fn layout_block_tree<'a>(
    root: &'a StyledNode<'a>,
    viewport_width: f32,
) -> LayoutBox<'a> {
    // We treat the root as a block box at (0,0).
    // Children will be stacked vertically inside it.
    layout_block_subtree(root, 0.0, 0.0, viewport_width).0
}

/// Internal recursive function:
/// - `x`, `y` = top-left of this box
/// - `width` = available width
/// Returns: (this LayoutBox, next_y_cursor)
fn layout_block_subtree<'a>(
    styled: &'a StyledNode<'a>,
    x: f32,
    y: f32,
    width: f32,
) -> (LayoutBox<'a>, f32) {
    let style = &styled.style;
    let mut children_boxes = Vec::new();
    let mut cursor_y = y;

    // Only Document + Element can have element children in style tree.
    match styled.node {
        Node::Document { .. } | Node::Element { .. } => {
            // Children are all StyledNode<'a> already filtered to elements/doc.
            for child in &styled.children {
                // Each child gets its own block subtree stacked vertically.
                let (child_box, new_cursor) =
                    layout_block_subtree(child, x, cursor_y, width);
                cursor_y = new_cursor;
                children_boxes.push(child_box);
            }
        }
        _ => {
            // Shouldn't happen normally because style tree filters this,
            // but we don't panic—just treat as leaf.
        }
    }

    let height = if children_boxes.is_empty() {
        // Leaf block: until we can measure text, give it a fixed height.
        DEFAULT_BLOCK_HEIGHT
    } else {
        // Height is from our top `y` to the cursor after last child.
        cursor_y - y
    };

    let rect = Rect { x, y, width, height };

    let layout_box = LayoutBox {
        kind: BoxKind::Block,
        style,
        node: styled,
        rect,
        children: children_boxes,
    };

    // The next sibling should start below this box.
    let next_y = y + height;

    (layout_box, next_y)
}