pub mod contracts;

mod context;
mod debug;
mod images;
mod inline;
mod primitives;
mod replaced;
mod text_control;

pub(crate) use context::PaintCtx;
pub use images::{ImageProvider, ImageState};
pub use primitives::{
    PaintBackground, PaintBorder, PaintBorderEdges, PaintBorderSide, PaintClip, PaintClipScope,
    PaintColor, PaintInlineBox, PaintInput, PaintListMarker, PaintListMarkerKind, PaintNode,
    PaintOutline, PaintPrimitive, PaintPrimitiveKind, PaintReplaced, PaintReplacedKind,
    PaintSource, PaintText, PaintTextDecoration, PaintTextDecorationLine, PaintTree,
};

use crate::EguiTextMeasurer;
use crate::input::{ActiveTarget, InputValueStore};
use crate::textarea::TextareaCachedLine;
use css::{Display, Length};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use html::{dom_utils::is_non_rendering_element, internal::Id};
use layout::{BoxKind, LayoutBox, LayoutPhaseOutput, ListMarker, Rectangle, TextMeasurer};
use std::cell::RefCell;
use std::collections::HashMap;

/// Structured layout-to-paint handoff.
#[derive(Clone, Copy)]
pub struct PaintPhaseInput<'layout, 'style_tree, 'dom> {
    layout: &'layout LayoutPhaseOutput<'style_tree, 'dom>,
}

impl<'layout, 'style_tree, 'dom> PaintPhaseInput<'layout, 'style_tree, 'dom> {
    pub fn new(layout: &'layout LayoutPhaseOutput<'style_tree, 'dom>) -> Self {
        Self { layout }
    }

    pub fn layout(&self) -> &'layout LayoutPhaseOutput<'style_tree, 'dom> {
        self.layout
    }

    pub fn layout_root(&self) -> &'layout LayoutBox<'style_tree, 'dom> {
        self.layout.root()
    }

    pub fn to_paint_input(
        &self,
        measurer: &dyn TextMeasurer,
    ) -> PaintInput<'layout, 'style_tree, 'dom> {
        PaintInput::from_phase_input(*self, measurer)
    }

    /// Stable debug snapshot for the semantic layout-to-paint handoff.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        out.push_str("version: 1\n");
        out.push_str("paint-phase-input\n");
        out.push_str(&format!(
            "layout-root-id: {}\n",
            self.layout_root().node_id().0
        ));
        out.push_str(&format!(
            "viewport-width: {:.2}\n",
            self.layout().viewport_width()
        ));
        out.push_str(&format!(
            "document-rect: x={:.2} y={:.2} w={:.2} h={:.2}\n",
            self.layout().document_rect().x,
            self.layout().document_rect().y,
            self.layout().document_rect().width,
            self.layout().document_rect().height
        ));
        for line in self.layout().to_debug_snapshot().lines().skip(4) {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}

/// Paint-runtime execution arguments. These are backend/runtime inputs, not the
/// semantic layout-to-paint handoff itself.
#[derive(Clone, Copy)]
pub struct PaintArgs<'a> {
    pub painter: &'a Painter,
    pub origin: Pos2,
    pub measurer: &'a EguiTextMeasurer,
    pub base_url: Option<&'a str>,
    pub resources: &'a dyn ImageProvider,
    pub input_values: &'a InputValueStore,
    pub focused: Option<Id>,
    pub focused_textarea_lines: Option<&'a [TextareaCachedLine]>,
    pub active: Option<ActiveTarget>,
    pub selection_bg_fill: Color32,
    pub selection_stroke: Stroke,
    pub fragment_rects: Option<&'a RefCell<HashMap<Id, Rectangle>>>,
}

fn paint_layout_box(
    layout: &LayoutBox<'_, '_>,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    let painter = ctx.painter;
    let origin = ctx.origin;
    let measurer = ctx.measurer;

    // Non-rendering elements (e.g. <head>, <style>, <script>) suppress painting for the entire subtree.
    // Layout/style should prevent paintable boxes from existing here.
    if is_non_rendering_element(layout.node.node) {
        return;
    }

    let rect = Rect::from_min_size(
        Pos2 {
            x: origin.x + layout.rect.x,
            y: origin.y + layout.rect.y,
        },
        Vec2 {
            x: layout.rect.width,
            y: layout.rect.height,
        },
    );

    // background
    let (r, g, b, a) = layout.style.background_color();
    if !layout.is_anonymous() && a > 0 {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(r, g, b, a));
    }

    if let Some(border) =
        primitives::border_primitive_from_layout(layout, PaintSource::from_layout(layout))
    {
        paint_border_primitive(border, painter, origin);
    }

    // 1) List marker (for display:list-item), if any.
    //    This does not affect layout; it's purely visual.
    if !layout.is_anonymous() && matches!(layout.style.display(), Display::ListItem) {
        paint_list_marker(layout, painter, origin, measurer);
    }

    paint_layout_box_contents_with_own_overflow_clip(layout, ctx, skip_inline_block_children);
    paint_outline_for_layout(layout, painter, origin);
}

fn paint_layout_box_contents_with_own_overflow_clip(
    layout: &LayoutBox<'_, '_>,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    let Some(clip) = layout.overflow_clip() else {
        paint_layout_box_contents(layout, ctx, skip_inline_block_children);
        return;
    };

    let clip_painter = ctx
        .painter
        .with_clip_rect(backend_rect_from_layout_rect(clip.rect(), ctx.origin));
    let clipped_ctx = PaintCtx {
        painter: &clip_painter,
        ..ctx
    };
    paint_layout_box_contents(layout, clipped_ctx, skip_inline_block_children);
}

fn backend_rect_from_layout_rect(rect: Rectangle, origin: Pos2) -> Rect {
    Rect::from_min_size(
        Pos2 {
            x: origin.x + rect.x,
            y: origin.y + rect.y,
        },
        Vec2 {
            x: rect.width,
            y: rect.height,
        },
    )
}

fn paint_outline_for_layout(layout: &LayoutBox<'_, '_>, painter: &Painter, origin: Pos2) {
    if let Some(outline) =
        primitives::outline_primitive_from_layout(layout, PaintSource::from_layout(layout))
    {
        paint_outline_primitive(outline, painter, origin);
    }
}

fn paint_border_primitive(border: PaintBorder, painter: &Painter, origin: Pos2) {
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: border.rect.x,
            y: border.rect.y,
            width: border.rect.width,
            height: border.edges.top.width,
        },
        border.edges.top,
    );
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: border.rect.x + (border.rect.width - border.edges.right.width).max(0.0),
            y: border.rect.y,
            width: border.edges.right.width,
            height: border.rect.height,
        },
        border.edges.right,
    );
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: border.rect.x,
            y: border.rect.y + (border.rect.height - border.edges.bottom.width).max(0.0),
            width: border.rect.width,
            height: border.edges.bottom.width,
        },
        border.edges.bottom,
    );
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: border.rect.x,
            y: border.rect.y,
            width: border.edges.left.width,
            height: border.rect.height,
        },
        border.edges.left,
    );
}

fn paint_outline_primitive(outline: PaintOutline, painter: &Painter, origin: Pos2) {
    let side = PaintBorderSide {
        width: outline.width,
        color: outline.color,
    };
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: outline.outer_rect.x,
            y: outline.outer_rect.y,
            width: outline.outer_rect.width,
            height: outline.width,
        },
        side,
    );
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: outline.border_rect.x + outline.border_rect.width,
            y: outline.border_rect.y,
            width: outline.width,
            height: outline.border_rect.height,
        },
        side,
    );
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: outline.outer_rect.x,
            y: outline.border_rect.y + outline.border_rect.height,
            width: outline.outer_rect.width,
            height: outline.width,
        },
        side,
    );
    paint_border_side_rect(
        painter,
        origin,
        Rectangle {
            x: outline.outer_rect.x,
            y: outline.border_rect.y,
            width: outline.width,
            height: outline.border_rect.height,
        },
        side,
    );
}

fn paint_border_side_rect(painter: &Painter, origin: Pos2, rect: Rectangle, side: PaintBorderSide) {
    if !side.is_visible() {
        return;
    }

    let rect = Rect::from_min_size(
        Pos2 {
            x: origin.x + rect.x,
            y: origin.y + rect.y,
        },
        Vec2 {
            x: rect.width.max(0.0),
            y: rect.height.max(0.0),
        },
    );
    painter.rect_filled(
        rect,
        0.0,
        Color32::from_rgba_unmultiplied(side.color.r, side.color.g, side.color.b, side.color.a),
    );
}

fn paint_layout_box_contents(
    layout: &LayoutBox<'_, '_>,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    // 2) Inline content
    inline::paint_inline_content(layout, ctx);

    // 3) Recurse into children
    for child in &layout.children {
        // ✅ Inline engine already painted inline-blocks AND replaced elements via fragments.
        if skip_inline_block_children
            && (matches!(child.kind, BoxKind::InlineBlock) || child.replaced.is_some())
        {
            continue;
        }

        paint_layout_box(child, ctx, skip_inline_block_children);
    }
}

fn paint_list_marker(
    layout: &LayoutBox<'_, '_>,
    painter: &Painter,
    origin: Pos2,
    measurer: &dyn TextMeasurer,
) {
    let marker = match layout.list_marker {
        Some(m) => m,
        None => return, // nothing to paint
    };

    // Choose marker text: bullet or number.
    let marker_text = match marker {
        ListMarker::Unordered => "•".to_string(),
        ListMarker::Ordered(index) => format!("{index}."),
    };

    // Use the list item's text style for the marker.
    let style = layout.style;
    let (cr, cg, cb, ca) = style.color();
    let text_color = Color32::from_rgba_unmultiplied(cr, cg, cb, ca);

    let Length::Px(font_px) = style.font_size();
    let font_id = FontId::proportional(font_px);

    // Position: slightly to the left of the content box (padding-left),
    // aligned with the top of the content. This doesn't change layout height.
    let bm = layout.box_metrics();

    // Content box x/y in layout coordinates (same as inline content start).
    let content_x = layout.rect.x + bm.padding_left;
    let content_y = layout.rect.y + bm.padding_top;

    // Measure marker width so we can place it just to the left of the content.
    let marker_width = measurer.measure(&marker_text, style);

    // How much gap between marker and content.
    let gap = 4.0;

    let marker_pos = Pos2 {
        x: origin.x + content_x - marker_width - gap,
        y: origin.y + content_y,
    };

    painter.text(
        marker_pos,
        Align2::LEFT_TOP,
        marker_text,
        font_id,
        text_color,
    );
}

pub fn paint_page(input: PaintPhaseInput<'_, '_, '_>, args: PaintArgs<'_>) {
    let layout_root = input.layout_root();
    let ctx = PaintCtx {
        painter: args.painter,
        origin: args.origin,
        measurer: args.measurer,
        base_url: args.base_url,
        resources: args.resources,
        input_values: args.input_values,
        focused: args.focused,
        focused_textarea_lines: args.focused_textarea_lines,
        active: args.active,
        selection_bg_fill: args.selection_bg_fill,
        selection_stroke: args.selection_stroke,
        fragment_rects: args.fragment_rects,
    };

    paint_layout_box(layout_root, ctx, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::{ComputedStyle, Length};
    use egui::{LayerId, Order, RawInput, Shape};
    use html::{Node, internal::Id};
    use layout::LayoutPhaseInput;
    use std::sync::Arc;

    struct TestMeasurer;

    impl layout::TextMeasurer for TestMeasurer {
        fn measure(&self, text: &str, _style: &ComputedStyle) -> f32 {
            text.chars().count() as f32 * 8.0
        }

        fn line_height(&self, style: &ComputedStyle) -> f32 {
            let Length::Px(px) = style.font_size();
            px * 1.2
        }
    }

    struct NoopImageProvider;

    impl ImageProvider for NoopImageProvider {
        fn image_state_by_url(&self, _url: &str) -> ImageState {
            ImageState::Missing
        }

        fn image_intrinsic_size_px(&self, _url: &str) -> Option<(u32, u32)> {
            None
        }
    }

    #[test]
    fn paint_phase_input_exposes_layout_owned_overflow_clip() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("width".to_string(), "100px".to_string()),
                    ("height".to_string(), "20px".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                ],
                children: Vec::new(),
            }],
        };
        let styled = css::build_style_tree(&dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        let snapshot = PaintPhaseInput::new(&layout).to_debug_snapshot();

        assert!(snapshot.contains(
            "overflow=policy=(inline=clip block=clip) clip=x=0.00 y=0.00 w=100.00 h=20.00"
        ));
    }

    #[test]
    fn immediate_paint_scopes_own_overflow_clip_to_contents_and_descendants() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "100px".to_string()),
                    ("height".to_string(), "50px".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                    ("background-color".to_string(), "#0a141e".to_string()),
                    ("border-top-width".to_string(), "2px".to_string()),
                    ("border-top-style".to_string(), "solid".to_string()),
                    ("border-top-color".to_string(), "#28323c".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "#46505a".to_string()),
                ],
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("div"),
                    attributes: Vec::new(),
                    style: vec![
                        ("display".to_string(), "block".to_string()),
                        ("width".to_string(), "140px".to_string()),
                        ("height".to_string(), "70px".to_string()),
                        ("background-color".to_string(), "#5a646e".to_string()),
                        ("outline-width".to_string(), "4px".to_string()),
                        ("outline-style".to_string(), "solid".to_string()),
                        ("outline-color".to_string(), "#78828c".to_string()),
                    ],
                    children: Vec::new(),
                }],
            }],
        };
        let styled = css::build_style_tree(&dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        let input_values = InputValueStore::new();
        let resources = NoopImageProvider;
        let ctx = egui::Context::default();
        let initial_clip = Rect::from_min_size(
            Pos2 {
                x: -100.0,
                y: -100.0,
            },
            Vec2 { x: 400.0, y: 400.0 },
        );
        let output = ctx.run(
            RawInput {
                screen_rect: Some(initial_clip),
                ..Default::default()
            },
            |ctx| {
                let painter = Painter::new(
                    ctx.clone(),
                    LayerId::new(Order::Foreground, egui::Id::new("page-paint")),
                    initial_clip,
                );
                let measurer = EguiTextMeasurer::new(ctx);
                paint_page(
                    PaintPhaseInput::new(&layout),
                    PaintArgs {
                        painter: &painter,
                        origin: Pos2 { x: 0.0, y: 0.0 },
                        measurer: &measurer,
                        base_url: None,
                        resources: &resources,
                        input_values: &input_values,
                        focused: None,
                        focused_textarea_lines: None,
                        active: None,
                        selection_bg_fill: Color32::TRANSPARENT,
                        selection_stroke: Stroke::NONE,
                        fragment_rects: None,
                    },
                );
            },
        );
        let section_clip = find_layout_by_direct_node_id(layout.root(), Id(2))
            .and_then(LayoutBox::overflow_clip)
            .map(|clip| backend_rect_from_layout_rect(clip.rect(), Pos2 { x: 0.0, y: 0.0 }))
            .expect("layout-owned section overflow clip");

        assert_eq!(
            clip_rects_for_fill(&output.shapes, Color32::from_rgb(0x0a, 0x14, 0x1e)),
            vec![initial_clip],
            "own background must not be clipped by the box's own overflow clip"
        );
        assert_eq!(
            clip_rects_for_fill(&output.shapes, Color32::from_rgb(0x28, 0x32, 0x3c)),
            vec![initial_clip],
            "own border must not be clipped by the box's own overflow clip"
        );
        assert_eq!(
            clip_rects_for_fill(&output.shapes, Color32::from_rgb(0x5a, 0x64, 0x6e)),
            vec![section_clip],
            "descendant background must be clipped by the ancestor overflow clip"
        );
        assert_eq!(
            clip_rects_for_fill(&output.shapes, Color32::from_rgb(0x46, 0x50, 0x5a)),
            vec![initial_clip; 4],
            "own outline must not be clipped by the box's own overflow clip"
        );
        assert_eq!(
            clip_rects_for_fill(&output.shapes, Color32::from_rgb(0x78, 0x82, 0x8c)),
            vec![section_clip; 4],
            "ancestor clips must apply to descendant outlines"
        );
    }

    #[test]
    fn immediate_paint_orders_parent_box_visuals_child_subtree_and_parent_outline() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "120px".to_string()),
                    ("height".to_string(), "60px".to_string()),
                    ("background-color".to_string(), "#102030".to_string()),
                    ("border-top-width".to_string(), "2px".to_string()),
                    ("border-top-style".to_string(), "solid".to_string()),
                    ("border-top-color".to_string(), "#405060".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "#a0b0c0".to_string()),
                ],
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("div"),
                    attributes: Vec::new(),
                    style: vec![
                        ("display".to_string(), "block".to_string()),
                        ("width".to_string(), "40px".to_string()),
                        ("height".to_string(), "20px".to_string()),
                        ("background-color".to_string(), "#708090".to_string()),
                    ],
                    children: Vec::new(),
                }],
            }],
        };
        let shapes = paint_shapes_for_dom(&dom);
        let fills = rect_fill_sequence(&shapes);

        let parent_background = position_of_fill(&fills, Color32::from_rgb(0x10, 0x20, 0x30))
            .expect("parent background fill");
        let parent_border = position_of_fill(&fills, Color32::from_rgb(0x40, 0x50, 0x60))
            .expect("parent border fill");
        let child_background = position_of_fill(&fills, Color32::from_rgb(0x70, 0x80, 0x90))
            .expect("child background fill");
        let parent_outline = position_of_fill(&fills, Color32::from_rgb(0xa0, 0xb0, 0xc0))
            .expect("parent outline fill");

        assert!(parent_background < parent_border);
        assert!(parent_border < child_background);
        assert!(child_background < parent_outline);
    }

    #[test]
    fn immediate_paint_preserves_layout_sibling_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "120px".to_string()),
                ],
                children: vec![
                    Node::Element {
                        id: Id(3),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![
                            ("display".to_string(), "block".to_string()),
                            ("width".to_string(), "40px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#112233".to_string()),
                        ],
                        children: Vec::new(),
                    },
                    Node::Element {
                        id: Id(4),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![
                            ("display".to_string(), "block".to_string()),
                            ("width".to_string(), "40px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#445566".to_string()),
                        ],
                        children: Vec::new(),
                    },
                ],
            }],
        };
        let shapes = paint_shapes_for_dom(&dom);
        let fills = rect_fill_sequence(&shapes);

        let first = position_of_fill(&fills, Color32::from_rgb(0x11, 0x22, 0x33))
            .expect("first child fill");
        let second = position_of_fill(&fills, Color32::from_rgb(0x44, 0x55, 0x66))
            .expect("second child fill");

        assert!(first < second);
    }

    #[test]
    fn immediate_paint_repeated_execution_has_stable_rect_fill_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "120px".to_string()),
                    ("height".to_string(), "60px".to_string()),
                    ("background-color".to_string(), "#102030".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                ],
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("div"),
                    attributes: Vec::new(),
                    style: vec![
                        ("display".to_string(), "block".to_string()),
                        ("width".to_string(), "40px".to_string()),
                        ("height".to_string(), "20px".to_string()),
                        ("background-color".to_string(), "#708090".to_string()),
                    ],
                    children: Vec::new(),
                }],
            }],
        };

        let first = rect_fill_signature(&paint_shapes_for_dom(&dom));
        let second = rect_fill_signature(&paint_shapes_for_dom(&dom));

        assert_eq!(first, second);
    }

    fn clip_rects_for_fill(shapes: &[egui::epaint::ClippedShape], fill: Color32) -> Vec<Rect> {
        shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Rect(rect) if rect.fill == fill => Some(shape.clip_rect),
                _ => None,
            })
            .collect()
    }

    fn find_layout_by_direct_node_id<'layout, 'style_tree, 'dom>(
        layout: &'layout LayoutBox<'style_tree, 'dom>,
        id: Id,
    ) -> Option<&'layout LayoutBox<'style_tree, 'dom>> {
        if layout.direct_node_id() == Some(id) {
            return Some(layout);
        }

        layout
            .children
            .iter()
            .find_map(|child| find_layout_by_direct_node_id(child, id))
    }

    fn paint_shapes_for_dom(dom: &Node) -> Vec<egui::epaint::ClippedShape> {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        let input_values = InputValueStore::new();
        let resources = NoopImageProvider;
        let ctx = egui::Context::default();
        let initial_clip = Rect::from_min_size(
            Pos2 {
                x: -100.0,
                y: -100.0,
            },
            Vec2 { x: 400.0, y: 400.0 },
        );
        ctx.run(
            RawInput {
                screen_rect: Some(initial_clip),
                ..Default::default()
            },
            |ctx| {
                let painter = Painter::new(
                    ctx.clone(),
                    LayerId::new(Order::Foreground, egui::Id::new("page-paint")),
                    initial_clip,
                );
                let measurer = EguiTextMeasurer::new(ctx);
                paint_page(
                    PaintPhaseInput::new(&layout),
                    PaintArgs {
                        painter: &painter,
                        origin: Pos2 { x: 0.0, y: 0.0 },
                        measurer: &measurer,
                        base_url: None,
                        resources: &resources,
                        input_values: &input_values,
                        focused: None,
                        focused_textarea_lines: None,
                        active: None,
                        selection_bg_fill: Color32::TRANSPARENT,
                        selection_stroke: Stroke::NONE,
                        fragment_rects: None,
                    },
                );
            },
        )
        .shapes
    }

    fn rect_fill_sequence(shapes: &[egui::epaint::ClippedShape]) -> Vec<Color32> {
        shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Rect(rect) if rect.fill != Color32::TRANSPARENT => Some(rect.fill),
                _ => None,
            })
            .collect()
    }

    fn rect_fill_signature(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Color32, Rect)> {
        shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Rect(rect) if rect.fill != Color32::TRANSPARENT => {
                    Some((rect.fill, shape.clip_rect))
                }
                _ => None,
            })
            .collect()
    }

    fn position_of_fill(fills: &[Color32], fill: Color32) -> Option<usize> {
        fills.iter().position(|candidate| *candidate == fill)
    }
}
