use crate::{
    LayoutBox,
    Rectangle,
    TextMeasurer,
    BoxKind,
    ReplacedKind,
    content_x_and_width,
    content_y,
    content_height,
};
use css::Display;
use html::{
    Node,
    Id,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Text,
    Link,
    Input,
    Image,
    InlineBlockBox,
    BlockBox,
}

#[derive(Clone, Copy, Debug)]
pub struct HitResult {
    pub node_id: Id,              // action target id (link/input/img/etc.)
    pub kind: HitKind,
    pub fragment_rect: Rectangle, // in layout coords
    pub local_pos: (f32, f32),    // point - fragment_rect.min
}

fn point_in_rect(p: (f32, f32), r: Rectangle) -> bool {
    let (x, y) = p;
    x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height
}

/// Engine hit-test in *layout coordinates* (same coordinate system as LayoutBox::rect)
pub fn hit_test<'a>(
    root: &'a LayoutBox<'a>,
    point: (f32, f32),
    measurer: &dyn TextMeasurer,
) -> Option<HitResult> {
    hit_test_box(root, point, measurer)
}

fn hit_test_box<'a>(
    node: &'a LayoutBox<'a>,
    point: (f32, f32),
    measurer: &dyn TextMeasurer,
) -> Option<HitResult> {
    if !point_in_rect(point, node.rect) {
        return None;
    }

    // 1) inline fragments first (text/link/replaced/inline-block)
    if let Some(hit) = hit_test_inline_fragments(node, point, measurer) {
        return Some(hit);
    }

    // 2) then recurse into children (reverse order = later painted on top)
    for child in node.children.iter().rev() {
        if matches!(child.kind, BoxKind::Inline | BoxKind::InlineBlock | BoxKind::ReplacedInline) {
            continue; // these are handled by inline fragments
        }
        if let Some(hit) = hit_test_box(child, point, measurer) {
            return Some(hit);
        }
    }

    // 3) fallback: this block
    Some(HitResult {
        node_id: node.node_id(),
        kind: HitKind::BlockBox,
        fragment_rect: node.rect,
        local_pos: (point.0 - node.rect.x, point.1 - node.rect.y),
    })
}

fn hit_test_inline_fragments<'a>(
    layout: &'a LayoutBox<'a>,
    point: (f32, f32),
    measurer: &dyn TextMeasurer,
) -> Option<HitResult> {
    match layout.node.node {
        Node::Element { .. } => {
            if matches!(layout.style.display, Display::Inline) {
                return None;
            }
        }
        Node::Document { .. } => return None,
        _ => return None,
    }

    let (content_x, content_width) =
        content_x_and_width(layout.style, layout.rect.x, layout.rect.width);
    let content_top = content_y(layout.style, layout.rect.y);
    let content_h = content_height(layout.style, layout.rect.height);

    let block_rect = Rectangle {
        x: content_x,
        y: content_top,
        width: content_width,
        height: content_h,
    };

    if !point_in_rect(point, block_rect) {
        return None;
    }

    let lines = crate::inline::layout_inline_for_paint(measurer, block_rect, layout);

    for line in &lines {
        for frag in &line.fragments {
            if !point_in_rect(point, frag.rect) {
                continue;
            }

            let local_pos = (point.0 - frag.rect.x, point.1 - frag.rect.y);

            match &frag.kind {
                crate::inline::InlineFragment::Replaced { kind, layout: frag_layout, .. } => {
                    let id = frag_layout.map(|lb| lb.node_id()).unwrap_or(layout.node_id());
                    let hit_kind = match kind {
                        ReplacedKind::Img => HitKind::Image,
                        ReplacedKind::InputText => HitKind::Input,
                        ReplacedKind::Button => HitKind::BlockBox, // (buttons get their own issue soon)
                    };

                    return Some(HitResult { node_id: id, kind: hit_kind, fragment_rect: frag.rect, local_pos });
                }

                crate::inline::InlineFragment::Box { layout: frag_layout, .. } => {
                    let id = frag_layout.map(|lb| lb.node_id()).unwrap_or(layout.node_id());
                    return Some(HitResult { node_id: id, kind: HitKind::InlineBlockBox, fragment_rect: frag.rect, local_pos });
                }

                crate::inline::InlineFragment::Text { action, .. } => {
                    if let Some((id, crate::inline::InlineActionKind::Link, _href)) = action {
                        return Some(HitResult {
                            node_id: *id,
                            kind: HitKind::Link,
                            fragment_rect: frag.rect,
                            local_pos,
                        });
                    }

                    return Some(HitResult {
                        node_id: layout.node_id(),
                        kind: HitKind::Text,
                        fragment_rect: frag.rect,
                        local_pos,
                    });
                }
            }
        }
    }

    None
}

