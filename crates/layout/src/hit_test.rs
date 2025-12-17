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
use html::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Block,
    InlineBlock,
    Replaced(ReplacedKind),
    Text, // optional; we can return block id for text hits later
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitTestResult {
    pub node_id: html::Id,
    pub kind: HitKind,
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
) -> Option<HitTestResult> {
    hit_test_box(root, point, measurer)
}

fn hit_test_box<'a>(
    node: &'a LayoutBox<'a>,
    point: (f32, f32),
    measurer: &dyn TextMeasurer,
) -> Option<HitTestResult> {
    // Quick reject
    if !point_in_rect(point, node.rect) {
        return None;
    }

    // 1) First try inline fragments for block-like hosts (this is where <img>/<input> live)
    if let Some(hit) = hit_test_inline_fragments(node, point, measurer) {
        return Some(hit);
    }

    // 2) Then recurse into block children (skip inline/inline-block children like paint does)
    for child in &node.children {
        if matches!(child.kind, BoxKind::Inline | BoxKind::InlineBlock | BoxKind::ReplacedInline) {
            continue;
        }
        if let Some(hit) = hit_test_box(child, point, measurer) {
            return Some(hit);
        }
    }

    // 3) Fallback: we’re inside this block’s rect
    Some(HitTestResult {
        node_id: node.node_id(),
        kind: HitKind::Block,
    })
}

fn hit_test_inline_fragments<'a>(
    layout: &'a LayoutBox<'a>,
    point: (f32, f32),
    measurer: &dyn TextMeasurer,
) -> Option<HitTestResult> {
    // Only block-like elements host an inline formatting context (same rules as paint_inline_content)
    match layout.node.node {
        Node::Element { .. } => {
            if matches!(layout.style.display, Display::Inline) {
                return None;
            }
        }
        Node::Document { .. } => return None,
        _ => return None,
    }

    let (content_x, content_width) = content_x_and_width(layout.style, layout.rect.x, layout.rect.width);
    let content_y = content_y(layout.style, layout.rect.y);
    let content_h = content_height(layout.style, layout.rect.height);

    let block_rect = Rectangle {
        x: content_x,
        y: content_y,
        width: content_width,
        height: content_h,
    };

    // Reuse the SAME inline geometry used by painting
    let lines = crate::inline::layout_inline_for_paint(measurer, block_rect, layout);

    // Walk fragments in visual order. For “best match”, prefer replaced/inline-block hits first.
    for line in &lines {
        for frag in &line.fragments {
            if !point_in_rect(point, frag.rect) {
                continue;
            }

            match &frag.kind {
                crate::inline::InlineFragment::Replaced { kind, layout: frag_layout, .. } => {
                    // Should be Some(layout) for replaced items in paint path
                    if let Some(lb) = frag_layout {
                        return Some(HitTestResult {
                            node_id: lb.node_id(),
                            kind: HitKind::Replaced(*kind),
                        });
                    }
                    // Fallback to parent if somehow missing
                    return Some(HitTestResult {
                        node_id: layout.node_id(),
                        kind: HitKind::Replaced(*kind),
                    });
                }

                crate::inline::InlineFragment::Box { layout: frag_layout, .. } => {
                    if let Some(child_box) = frag_layout {
                        // Inline-block subtree is painted translated into frag rect.
                        // Translate the point back into the child’s layout coordinate space:
                        let frag_min_x = frag.rect.x;
                        let frag_min_y = frag.rect.y;

                        let child_point = (
                            (point.0 - frag_min_x) + child_box.rect.x,
                            (point.1 - frag_min_y) + child_box.rect.y,
                        );

                        // Prefer a deeper hit inside the subtree
                        if let Some(hit) = hit_test_box(child_box, child_point, measurer) {
                            return Some(hit);
                        }

                        return Some(HitTestResult {
                            node_id: child_box.node_id(),
                            kind: HitKind::InlineBlock,
                        });
                    }

                    return Some(HitTestResult {
                        node_id: layout.node_id(),
                        kind: HitKind::InlineBlock,
                    });
                }

                crate::inline::InlineFragment::Text { .. } => {
                    // Phase 1: we don’t have per-text-node IDs in fragments, so return block.
                    return Some(HitTestResult {
                        node_id: layout.node_id(),
                        kind: HitKind::Text,
                    });
                }
            }
        }
    }

    None
}
