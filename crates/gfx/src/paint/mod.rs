pub mod contracts;

mod context;
mod debug;
mod images;
mod inline;
mod primitives;
mod replaced;
mod stacking;
mod text_control;

pub(crate) use context::PaintCtx;
pub use images::{ImageProvider, ImageState};
pub use primitives::{
    PaintArtifact, PaintBackground, PaintBorder, PaintBorderEdges, PaintBorderSide, PaintClip,
    PaintClipScope, PaintColor, PaintInlineBox, PaintInput, PaintListMarker, PaintListMarkerKind,
    PaintNode, PaintOutline, PaintPrimitive, PaintPrimitiveKind, PaintReplaced, PaintReplacedKind,
    PaintSource, PaintText, PaintTextDecoration, PaintTextDecorationLine, PaintTree,
};
pub use stacking::{
    StackablePaintItem, StackingContextId, StackingContextNode, StackingContextSource,
    StackingContextTree, StackingLayerKind, StackingOrderKey, StackingOrderSlot,
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
        if let Some((tree, owner_context)) = ctx.stacking_contexts
            && tree.source_starts_external_context(owner_context, PaintSource::from_layout(child))
        {
            continue;
        }

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
    let artifact = PaintArtifact::from_phase_input(input, args.measurer);
    paint_page_with_artifact(input, &artifact, args);
}

pub fn paint_page_with_artifact(
    input: PaintPhaseInput<'_, '_, '_>,
    artifact: &PaintArtifact,
    args: PaintArgs<'_>,
) {
    let ctx = PaintCtx {
        painter: args.painter,
        origin: args.origin,
        measurer: args.measurer,
        resources: args.resources,
        input_values: args.input_values,
        focused: args.focused,
        focused_textarea_lines: args.focused_textarea_lines,
        active: args.active,
        selection_bg_fill: args.selection_bg_fill,
        selection_stroke: args.selection_stroke,
        fragment_rects: args.fragment_rects,
        stacking_contexts: None,
    };

    paint_stacking_context(
        artifact.stacking_contexts().root_id(),
        input.layout(),
        artifact,
        ctx,
        true,
    );
}

fn paint_stacking_context(
    context_id: StackingContextId,
    layout: &LayoutPhaseOutput<'_, '_>,
    artifact: &PaintArtifact,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    let ancestor_clips = if context_id == artifact.stacking_contexts().root_id() {
        Vec::new()
    } else {
        artifact
            .stacking_contexts()
            .context(context_id)
            .map(|context| {
                ancestor_overflow_clip_rects(layout.root(), context.source().paint_source())
            })
            .unwrap_or_default()
    };
    paint_stacking_context_with_clip_chain(
        &ancestor_clips,
        context_id,
        layout,
        artifact,
        ctx,
        skip_inline_block_children,
    );
}

fn paint_stacking_context_with_clip_chain(
    clips: &[Rectangle],
    context_id: StackingContextId,
    layout: &LayoutPhaseOutput<'_, '_>,
    artifact: &PaintArtifact,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    let Some((clip, rest)) = clips.split_first() else {
        paint_stacking_context_body(
            context_id,
            layout,
            artifact,
            ctx,
            skip_inline_block_children,
        );
        return;
    };

    let clip_painter = ctx
        .painter
        .with_clip_rect(backend_rect_from_layout_rect(*clip, ctx.origin));
    let clipped_ctx = PaintCtx {
        painter: &clip_painter,
        ..ctx
    };
    paint_stacking_context_with_clip_chain(
        rest,
        context_id,
        layout,
        artifact,
        clipped_ctx,
        skip_inline_block_children,
    );
}

fn paint_stacking_context_body(
    context_id: StackingContextId,
    layout: &LayoutPhaseOutput<'_, '_>,
    artifact: &PaintArtifact,
    ctx: PaintCtx<'_>,
    skip_inline_block_children: bool,
) {
    for slot in artifact.stacking_contexts().ordered_slots(context_id) {
        match slot {
            StackingOrderSlot::ChildContext(child_context_id) => {
                paint_stacking_context(
                    child_context_id,
                    layout,
                    artifact,
                    ctx,
                    skip_inline_block_children,
                );
            }
            StackingOrderSlot::ContextSource(source) => {
                if let Some(layout_box) = find_layout_by_paint_source(layout.root(), source) {
                    let context_ctx = PaintCtx {
                        stacking_contexts: Some((artifact.stacking_contexts(), context_id)),
                        ..ctx
                    };
                    paint_layout_box(layout_box, context_ctx, skip_inline_block_children);
                }
            }
        }
    }
}

fn find_layout_by_paint_source<'layout, 'style_tree, 'dom>(
    layout: &'layout LayoutBox<'style_tree, 'dom>,
    source: PaintSource,
) -> Option<&'layout LayoutBox<'style_tree, 'dom>> {
    if PaintSource::from_layout(layout) == source {
        return Some(layout);
    }

    layout
        .children
        .iter()
        .find_map(|child| find_layout_by_paint_source(child, source))
}

fn ancestor_overflow_clip_rects(root: &LayoutBox<'_, '_>, source: PaintSource) -> Vec<Rectangle> {
    let mut clips = Vec::new();
    if collect_ancestor_overflow_clip_rects(root, source, &mut clips) {
        clips.reverse();
    }
    clips
}

fn collect_ancestor_overflow_clip_rects(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
    clips: &mut Vec<Rectangle>,
) -> bool {
    if PaintSource::from_layout(layout) == source {
        return true;
    }

    for child in &layout.children {
        if collect_ancestor_overflow_clip_rects(child, source, clips) {
            if let Some(clip) = layout.overflow_clip() {
                clips.push(clip.rect());
            }
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::{ComputedStyle, Length};
    use egui::{LayerId, Order, RawInput, Shape};
    use html::{Node, internal::Id};
    use layout::LayoutPhaseInput;

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
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
                    ("width".to_string(), "100px".to_string()),
                    ("height".to_string(), "20px".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                ],
                Vec::new(),
            )],
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
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
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
                vec![html::internal::node_element_from_parts(
                    Id(3),
                    html::internal::html_name("div"),
                    Vec::new(),
                    vec![
                        ("display".to_string(), "block".to_string()),
                        ("width".to_string(), "140px".to_string()),
                        ("height".to_string(), "70px".to_string()),
                        ("background-color".to_string(), "#5a646e".to_string()),
                        ("outline-width".to_string(), "4px".to_string()),
                        ("outline-style".to_string(), "solid".to_string()),
                        ("outline-color".to_string(), "#78828c".to_string()),
                    ],
                    Vec::new(),
                )],
            )],
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
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
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
                vec![html::internal::node_element_from_parts(
                    Id(3),
                    html::internal::html_name("div"),
                    Vec::new(),
                    vec![
                        ("display".to_string(), "block".to_string()),
                        ("width".to_string(), "40px".to_string()),
                        ("height".to_string(), "20px".to_string()),
                        ("background-color".to_string(), "#708090".to_string()),
                    ],
                    Vec::new(),
                )],
            )],
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
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "120px".to_string()),
                ],
                vec![
                    html::internal::node_element_from_parts(
                        Id(3),
                        html::internal::html_name("div"),
                        Vec::new(),
                        vec![
                            ("display".to_string(), "block".to_string()),
                            ("width".to_string(), "40px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#112233".to_string()),
                        ],
                        Vec::new(),
                    ),
                    html::internal::node_element_from_parts(
                        Id(4),
                        html::internal::html_name("div"),
                        Vec::new(),
                        vec![
                            ("display".to_string(), "block".to_string()),
                            ("width".to_string(), "40px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#445566".to_string()),
                        ],
                        Vec::new(),
                    ),
                ],
            )],
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
    fn immediate_paint_uses_ab3_z_index_layer_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "120px".to_string()),
                    ("height".to_string(), "20px".to_string()),
                    ("background-color".to_string(), "#00aa00".to_string()),
                ],
                vec![
                    positioned_block(Id(3), "0", "#0000aa", Vec::new()),
                    positioned_block(Id(4), "-1", "#aa0000", Vec::new()),
                    positioned_block(Id(5), "2", "#aaaa00", Vec::new()),
                ],
            )],
        };

        let fills = rect_fill_sequence(&paint_shapes_for_dom(&dom));
        let negative = position_of_fill(&fills, Color32::from_rgb(0xaa, 0x00, 0x00))
            .expect("negative z-index fill");
        let normal = position_of_fill(&fills, Color32::from_rgb(0x00, 0xaa, 0x00))
            .expect("normal flow fill");
        let zero =
            position_of_fill(&fills, Color32::from_rgb(0x00, 0x00, 0xaa)).expect("zero fill");
        let positive = position_of_fill(&fills, Color32::from_rgb(0xaa, 0xaa, 0x00))
            .expect("positive z-index fill");

        assert!(negative < normal);
        assert!(normal < zero);
        assert!(zero < positive);
    }

    #[test]
    fn paint_order_operation_snapshot_and_immediate_paint_agree_on_stacking_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "120px".to_string()),
                    ("height".to_string(), "20px".to_string()),
                    ("background-color".to_string(), "#00aa00".to_string()),
                ],
                vec![
                    positioned_block(Id(3), "0", "#0000aa", Vec::new()),
                    positioned_block(Id(4), "-1", "#aa0000", Vec::new()),
                    positioned_block(Id(5), "2", "#aaaa00", Vec::new()),
                ],
            )],
        };

        let order_snapshot = paint_order_snapshot_for_dom(&dom);
        let operation_snapshot = paint_operation_snapshot_for_dom(&dom);
        assert_color_order(
            &order_snapshot,
            [
                "color=rgba(170,0,0,255)",
                "color=rgba(0,170,0,255)",
                "color=rgba(0,0,170,255)",
                "color=rgba(170,170,0,255)",
            ],
        );
        assert_color_order(
            &operation_snapshot,
            [
                "color=rgba(170,0,0,255)",
                "color=rgba(0,170,0,255)",
                "color=rgba(0,0,170,255)",
                "color=rgba(170,170,0,255)",
            ],
        );

        let fills = rect_fill_sequence(&paint_shapes_for_dom(&dom));
        let negative = position_of_fill(&fills, Color32::from_rgb(0xaa, 0x00, 0x00))
            .expect("negative z-index fill");
        let normal = position_of_fill(&fills, Color32::from_rgb(0x00, 0xaa, 0x00))
            .expect("normal flow fill");
        let zero =
            position_of_fill(&fills, Color32::from_rgb(0x00, 0x00, 0xaa)).expect("zero fill");
        let positive = position_of_fill(&fills, Color32::from_rgb(0xaa, 0xaa, 0x00))
            .expect("positive z-index fill");

        assert!(negative < normal);
        assert!(normal < zero);
        assert!(zero < positive);
    }

    #[test]
    fn immediate_paint_keeps_static_integer_and_positioned_auto_z_index_in_source_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![("display".to_string(), "block".to_string())],
                vec![
                    html::internal::node_element_from_parts(
                        Id(3),
                        html::internal::html_name("div"),
                        Vec::new(),
                        vec![
                            ("display".to_string(), "block".to_string()),
                            ("z-index".to_string(), "9".to_string()),
                            ("width".to_string(), "20px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#aa0000".to_string()),
                        ],
                        Vec::new(),
                    ),
                    html::internal::node_element_from_parts(
                        Id(4),
                        html::internal::html_name("div"),
                        Vec::new(),
                        vec![
                            ("display".to_string(), "block".to_string()),
                            ("position".to_string(), "relative".to_string()),
                            ("z-index".to_string(), "auto".to_string()),
                            ("width".to_string(), "20px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#00aa00".to_string()),
                        ],
                        Vec::new(),
                    ),
                    positioned_block(Id(5), "1", "#0000aa", Vec::new()),
                ],
            )],
        };

        let fills = rect_fill_sequence(&paint_shapes_for_dom(&dom));
        let static_integer = position_of_fill(&fills, Color32::from_rgb(0xaa, 0x00, 0x00))
            .expect("static integer z-index fill");
        let positioned_auto = position_of_fill(&fills, Color32::from_rgb(0x00, 0xaa, 0x00))
            .expect("positioned auto z-index fill");
        let positive_context = position_of_fill(&fills, Color32::from_rgb(0x00, 0x00, 0xaa))
            .expect("positive child-context fill");

        assert!(static_integer < positioned_auto);
        assert!(positioned_auto < positive_context);
    }

    #[test]
    fn immediate_paint_keeps_child_context_atomic_relative_to_siblings() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![("display".to_string(), "block".to_string())],
                vec![
                    positioned_block(
                        Id(3),
                        "1",
                        "#aa0000",
                        vec![positioned_block(Id(4), "-1", "#00aa00", Vec::new())],
                    ),
                    positioned_block(Id(5), "2", "#0000aa", Vec::new()),
                ],
            )],
        };

        let fills = rect_fill_sequence(&paint_shapes_for_dom(&dom));
        let nested_negative = position_of_fill(&fills, Color32::from_rgb(0x00, 0xaa, 0x00))
            .expect("nested negative fill");
        let parent =
            position_of_fill(&fills, Color32::from_rgb(0xaa, 0x00, 0x00)).expect("parent fill");
        let sibling =
            position_of_fill(&fills, Color32::from_rgb(0x00, 0x00, 0xaa)).expect("sibling fill");

        assert!(nested_negative < parent);
        assert!(parent < sibling);
    }

    #[test]
    fn immediate_paint_keeps_positioned_child_context_under_ancestor_overflow_clip() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "40px".to_string()),
                    ("height".to_string(), "20px".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                ],
                vec![positioned_block(Id(3), "1", "#aa0000", Vec::new())],
            )],
        };

        let styled = css::build_style_tree(&dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        let section_clip = find_layout_by_direct_node_id(layout.root(), Id(2))
            .and_then(LayoutBox::overflow_clip)
            .map(|clip| backend_rect_from_layout_rect(clip.rect(), Pos2 { x: 0.0, y: 0.0 }))
            .expect("layout-owned section overflow clip");
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

        assert_eq!(
            clip_rects_for_fill(&output.shapes, Color32::from_rgb(0xaa, 0x00, 0x00)),
            vec![section_clip],
            "positioned child contexts remain descendants for overflow clipping"
        );
    }

    #[test]
    fn immediate_paint_repeated_execution_has_stable_rect_fill_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "120px".to_string()),
                    ("height".to_string(), "60px".to_string()),
                    ("background-color".to_string(), "#102030".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                ],
                vec![html::internal::node_element_from_parts(
                    Id(3),
                    html::internal::html_name("div"),
                    Vec::new(),
                    vec![
                        ("display".to_string(), "block".to_string()),
                        ("width".to_string(), "40px".to_string()),
                        ("height".to_string(), "20px".to_string()),
                        ("background-color".to_string(), "#708090".to_string()),
                    ],
                    Vec::new(),
                )],
            )],
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

    fn paint_order_snapshot_for_dom(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        PaintPhaseInput::new(&layout)
            .to_paint_input(&TestMeasurer)
            .to_order_debug_snapshot()
    }

    fn paint_operation_snapshot_for_dom(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        PaintPhaseInput::new(&layout)
            .to_paint_input(&TestMeasurer)
            .to_operation_debug_snapshot()
    }

    fn assert_color_order<const N: usize>(snapshot: &str, colors: [&str; N]) {
        let mut previous = None;
        for color in colors {
            let index = snapshot
                .lines()
                .position(|line| line.contains(color))
                .unwrap_or_else(|| panic!("snapshot should contain {color:?}\n{snapshot}"));
            if let Some(previous) = previous {
                assert!(previous < index, "{color} should appear after prior color");
            }
            previous = Some(index);
        }
    }

    fn positioned_block(id: Id, z_index: &str, color: &str, children: Vec<Node>) -> Node {
        html::internal::node_element_from_parts(
            id,
            html::internal::html_name("div"),
            Vec::new(),
            vec![
                ("display".to_string(), "block".to_string()),
                ("position".to_string(), "relative".to_string()),
                ("z-index".to_string(), z_index.to_string()),
                ("width".to_string(), "20px".to_string()),
                ("height".to_string(), "20px".to_string()),
                ("background-color".to_string(), color.to_string()),
            ],
            children,
        )
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
