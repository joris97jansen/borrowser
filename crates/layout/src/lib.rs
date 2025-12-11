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

/// The inner "content box" of a layout box: border box minus padding.
/// We expose it via small helpers so that all code computes content
/// geometry in a single, consistent way.
pub fn content_x_and_width(
    style: &ComputedStyle,
    border_x: f32,
    border_width: f32,
) -> (f32, f32) {
    let bm = style.box_metrics;

    let content_x = border_x + bm.padding_left;
    let content_width = (border_width - bm.padding_left - bm.padding_right).max(0.0);

    debug_assert!(
        content_width >= 0.0,
        "content_x_and_width produced negative width: border_width={border_width}, paddings=({}, {})",
        bm.padding_left,
        bm.padding_right,
    );

    (content_x, content_width)
}

/// Vertical position of the content box top (border box top + padding-top).
pub fn content_y(style: &ComputedStyle, border_y: f32) -> f32 {
    let bm = style.box_metrics;
    border_y + bm.padding_top
}

/// Height of the content box (border box height minus vertical padding).
pub fn content_height(style: &ComputedStyle, border_height: f32) -> f32 {
    let bm = style.box_metrics;
    let content_height = (border_height - bm.padding_top - bm.padding_bottom).max(0.0);

    debug_assert!(
        content_height >= 0.0,
        "content_height produced negative height: border_height={border_height}, paddings=({}, {})",
        bm.padding_top,
        bm.padding_bottom,
    );

    content_height
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
/// - `measurer` is used to measure text during inline layout
pub fn layout_block_tree<'a>(
    root: &'a StyledNode<'a>,
    page_width: f32,
    measurer: &dyn TextMeasurer,
) -> LayoutBox<'a> {
    // 1) Build the layout tree structure (no real geometry yet).
    let mut root_box = layout_block_subtree(root, 0.0, 0.0, page_width);

    // 2) Single authoritative geometry pass: inline + block layout.
    //
    //    This computes x/y/width/height for *all* LayoutBoxes,
    //    using the same inline token / LineBox pipeline that painting uses.
    crate::inline::refine_layout_with_inline(measurer, &mut root_box);

    root_box
}


/// Internal recursive function:
/// - `x`, `y` = top-left of this box
/// - `width`  = available width
/// Returns (layout box, next y position after this box).
fn layout_block_subtree<'a>(
    styled: &'a StyledNode<'a>,
    x: f32,
    y: f32,
    width: f32,
) -> LayoutBox<'a> {
    use html::Node;

    // 0) Non-rendering elements: transparent containers.
    //    They still get a LayoutBox so the tree shape matches the DOM,
    //    but we don't attempt to compute geometry here.
    if is_non_rendering_element(styled.node) {
        let mut children_boxes = Vec::new();

        for child in &styled.children {
            let child_box = layout_block_subtree(child, x, y, width);
            children_boxes.push(child_box);
        }

        let rect = Rectangle {
            x,
            y,
            width,
            height: 0.0, // real height will be computed later
        };

        return LayoutBox {
            kind: BoxKind::Block,
            style: &styled.style,
            node: styled,
            rect,
            children: children_boxes,
            list_marker: None,
        };
    }

    // 1) Build children recursively (no vertical layout here).
    let mut children_boxes = Vec::new();

    if matches!(styled.node, Node::Document { .. } | Node::Element { .. }) {
        // Detect whether this element is a <ul> or <ol> so we can assign
        // list markers to its <li> children.
        let (is_ul, is_ol) = match styled.node {
            Node::Element { name, .. } => {
                let is_ul = name.eq_ignore_ascii_case("ul");
                let is_ol = name.eq_ignore_ascii_case("ol");
                (is_ul, is_ol)
            }
            _ => (false, false),
        };

        // For <ol>, number <li> children starting at 1.
        let mut next_ol_index: u32 = 1;

        for child in &styled.children {
            let mut child_box = layout_block_subtree(child, x, y, width);

            // If this is a list container (<ul>/<ol>) and the child is a list-item,
            // assign a marker.
            if let Node::Element { .. } = child.node {
                if child_box.style.display == Display::ListItem {
                    if is_ul {
                        child_box.list_marker = Some(ListMarker::Unordered);
                    } else if is_ol {
                        child_box.list_marker = Some(ListMarker::Ordered(next_ol_index));
                        next_ol_index += 1;
                    }
                }
            }

            children_boxes.push(child_box);
        }
    }

    // 2) Decide box kind based on node type + computed display.
    let style = &styled.style;

    let kind = match styled.node {
        // Document and <html> act as block-level containers.
        Node::Document { .. } => BoxKind::Block,
        Node::Element { name, .. } if name.eq_ignore_ascii_case("html") => BoxKind::Block,

        // Text / Comment nodes are inline content but we
        // represent them as Block here; the inline engine
        // will interpret them correctly via BoxKind and display.
        Node::Text { .. } | Node::Comment { .. } => BoxKind::Block,

        // All other elements: look at display.
        _ => match style.display {
            Display::Inline => BoxKind::Inline,
            Display::InlineBlock => BoxKind::InlineBlock,
            _ => BoxKind::Block,
        },
    };

    // 3) Border-box rect: x/y/width are authoritative here.
    //    Height is always 0.0 in this phase; it will be computed by
    //    the inline-aware layout pass (recompute_block_heights).
    let rect = Rectangle {
        x,
        y,
        width,
        height: 0.0,
    };

    LayoutBox {
        kind,
        style,
        node: styled,
        rect,
        children: children_boxes,
        list_marker: None,
    }
}
