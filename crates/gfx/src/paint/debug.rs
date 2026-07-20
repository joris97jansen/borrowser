use std::fmt::Write;

use layout::{LayoutBox, Rectangle};

use super::contracts::PaintOrderPhase;
use super::{
    PaintBorder, PaintBorderSide, PaintClip, PaintClipScope, PaintColor, PaintInlineBox,
    PaintInput, PaintListMarker, PaintListMarkerKind, PaintNode, PaintOutline, PaintPrimitive,
    PaintReplaced, PaintReplacedKind, PaintSource, PaintText, PaintTextDecoration,
    PaintTextDecorationLine, StackingContextId, StackingContextNode, StackingOrderSlot,
};

/// Stable paint-owned operation snapshot for visual regression tests.
///
/// This serializes Borrowser paint primitives and paint-owned ordering rules.
/// It is not an egui command stream, retained display list, scene graph,
/// compositor model, or pixel snapshot.
pub fn paint_operation_debug_snapshot(input: &PaintInput<'_, '_, '_>) -> String {
    let mut writer = PaintOperationDebugWriter::new(input);
    writer.write_snapshot();
    writer.finish()
}

struct PaintOperationDebugWriter<'a, 'layout, 'style_tree, 'dom> {
    input: &'a PaintInput<'layout, 'style_tree, 'dom>,
    out: String,
    next_index: usize,
}

impl<'a, 'layout, 'style_tree, 'dom> PaintOperationDebugWriter<'a, 'layout, 'style_tree, 'dom> {
    fn new(input: &'a PaintInput<'layout, 'style_tree, 'dom>) -> Self {
        Self {
            input,
            out: String::new(),
            next_index: 0,
        }
    }

    fn write_snapshot(&mut self) {
        writeln!(&mut self.out, "version: 1").expect("write paint operation snapshot");
        writeln!(&mut self.out, "paint-operation-snapshot")
            .expect("write paint operation snapshot");
        writeln!(
            &mut self.out,
            "layout-root-id: {}",
            self.input.layout().root().node_id().0
        )
        .expect("write paint operation snapshot");
        writeln!(
            &mut self.out,
            "viewport-width: {:.2}",
            self.input.layout().viewport_width()
        )
        .expect("write paint operation snapshot");
        writeln!(
            &mut self.out,
            "document-rect: {}",
            rectangle_debug_label(self.input.layout().document_rect())
        )
        .expect("write paint operation snapshot");
        self.write_stacking_context(self.input.stacking_contexts().root());
    }

    fn finish(self) -> String {
        self.out
    }

    fn write_stacking_context(&mut self, context: &StackingContextNode) {
        if context.id() != self.input.stacking_contexts().root_id() {
            let clips = ancestor_overflow_clips(
                self.input.layout().root(),
                context.source().paint_source(),
            );
            self.write_stacking_context_with_ancestor_clips(&clips, context);
            return;
        }

        self.write_stacking_context_body(context);
    }

    fn write_stacking_context_with_ancestor_clips(
        &mut self,
        clips: &[PaintClip],
        context: &StackingContextNode,
    ) {
        let Some((clip, rest)) = clips.split_first() else {
            self.write_stacking_context_body(context);
            return;
        };

        self.write_clip_operation("begin-clip", clip);
        self.write_stacking_context_with_ancestor_clips(rest, context);
        self.write_clip_operation("end-clip", clip);
    }

    fn write_stacking_context_body(&mut self, context: &StackingContextNode) {
        for slot in self.input.stacking_contexts().ordered_slots(context.id()) {
            match slot {
                StackingOrderSlot::ChildContext(child_context_id) => {
                    if let Some(child) = self.input.stacking_contexts().context(child_context_id) {
                        self.write_stacking_context(child);
                    }
                }
                StackingOrderSlot::ContextSource(source) => {
                    if let Some(node) = self.input.tree().node_for_source(source) {
                        self.write_node(node, context.id());
                    }
                }
            }
        }
    }

    fn write_node(&mut self, node: &PaintNode, owner_context: StackingContextId) {
        let clip_index = node
            .primitives()
            .iter()
            .position(|primitive| matches!(primitive, PaintPrimitive::Clip(_)));

        let Some(clip_index) = clip_index else {
            self.write_primitives(node.primitives());
            self.write_children(node, owner_context);
            self.write_primitives(node.post_primitives());
            return;
        };

        self.write_primitives(&node.primitives()[..clip_index]);

        let PaintPrimitive::Clip(clip) = &node.primitives()[clip_index] else {
            unreachable!("clip index points at a clip primitive");
        };
        self.write_clip_operation("begin-clip", clip);
        self.write_primitives(&node.primitives()[clip_index + 1..]);
        self.write_children(node, owner_context);
        self.write_clip_operation("end-clip", clip);
        self.write_primitives(node.post_primitives());
    }

    fn write_children(&mut self, node: &PaintNode, owner_context: StackingContextId) {
        for child in node.children() {
            if self
                .input
                .stacking_contexts()
                .source_starts_external_context(owner_context, child.source())
            {
                continue;
            }

            self.write_node(child, owner_context);
        }
    }

    fn write_primitives(&mut self, primitives: &[PaintPrimitive]) {
        for primitive in primitives {
            self.write_primitive(primitive);
        }
    }

    fn write_primitive(&mut self, primitive: &PaintPrimitive) {
        match primitive {
            PaintPrimitive::Background(background) => self.write_fill_rect(
                PaintOrderPhase::BoxBackground,
                "background",
                background.source,
                background.rect,
                background.color,
            ),
            PaintPrimitive::Border(border) => self.write_border(border),
            PaintPrimitive::Outline(outline) => self.write_outline(outline),
            PaintPrimitive::ListMarker(marker) => self.write_list_marker(marker),
            PaintPrimitive::Clip(_) => {
                unreachable!(
                    "clip primitives are serialized at paint-node scope as begin/end operation pairs"
                )
            }
            PaintPrimitive::Text(text) => self.write_text(text),
            PaintPrimitive::TextDecoration(decoration) => self.write_text_decoration(decoration),
            PaintPrimitive::InlineBox(inline_box) => self.write_inline_box(inline_box),
            PaintPrimitive::Replaced(replaced) => self.write_replaced(replaced),
        }
    }

    fn write_fill_rect(
        &mut self,
        phase: PaintOrderPhase,
        detail: &'static str,
        source: PaintSource,
        rect: Rectangle,
        color: PaintColor,
    ) {
        self.write_operation_prefix(phase, "fill-rect");
        writeln!(
            &mut self.out,
            " detail={} source={} rect={} color={}",
            detail,
            source_debug_label(source),
            rectangle_debug_label(rect),
            color_debug_label(color)
        )
        .expect("write paint operation snapshot");
    }

    fn write_border(&mut self, border: &PaintBorder) {
        let rect = border.rect;
        self.write_border_side(
            PaintOrderPhase::BoxBorder,
            "border-top",
            border.source,
            Rectangle {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: border.edges.top.width,
            },
            border.edges.top,
        );
        self.write_border_side(
            PaintOrderPhase::BoxBorder,
            "border-right",
            border.source,
            Rectangle {
                x: rect.x + (rect.width - border.edges.right.width).max(0.0),
                y: rect.y,
                width: border.edges.right.width,
                height: rect.height,
            },
            border.edges.right,
        );
        self.write_border_side(
            PaintOrderPhase::BoxBorder,
            "border-bottom",
            border.source,
            Rectangle {
                x: rect.x,
                y: rect.y + (rect.height - border.edges.bottom.width).max(0.0),
                width: rect.width,
                height: border.edges.bottom.width,
            },
            border.edges.bottom,
        );
        self.write_border_side(
            PaintOrderPhase::BoxBorder,
            "border-left",
            border.source,
            Rectangle {
                x: rect.x,
                y: rect.y,
                width: border.edges.left.width,
                height: rect.height,
            },
            border.edges.left,
        );
    }

    fn write_outline(&mut self, outline: &PaintOutline) {
        let side = PaintBorderSide {
            width: outline.width,
            color: outline.color,
        };
        self.write_border_side(
            PaintOrderPhase::BoxOutline,
            "outline-top",
            outline.source,
            Rectangle {
                x: outline.outer_rect.x,
                y: outline.outer_rect.y,
                width: outline.outer_rect.width,
                height: outline.width,
            },
            side,
        );
        self.write_border_side(
            PaintOrderPhase::BoxOutline,
            "outline-right",
            outline.source,
            Rectangle {
                x: outline.border_rect.x + outline.border_rect.width,
                y: outline.border_rect.y,
                width: outline.width,
                height: outline.border_rect.height,
            },
            side,
        );
        self.write_border_side(
            PaintOrderPhase::BoxOutline,
            "outline-bottom",
            outline.source,
            Rectangle {
                x: outline.outer_rect.x,
                y: outline.border_rect.y + outline.border_rect.height,
                width: outline.outer_rect.width,
                height: outline.width,
            },
            side,
        );
        self.write_border_side(
            PaintOrderPhase::BoxOutline,
            "outline-left",
            outline.source,
            Rectangle {
                x: outline.outer_rect.x,
                y: outline.border_rect.y,
                width: outline.width,
                height: outline.border_rect.height,
            },
            side,
        );
    }

    fn write_border_side(
        &mut self,
        phase: PaintOrderPhase,
        detail: &'static str,
        source: PaintSource,
        rect: Rectangle,
        side: PaintBorderSide,
    ) {
        if !side.is_visible() {
            return;
        }

        self.write_fill_rect(phase, detail, source, rect, side.color);
    }

    fn write_list_marker(&mut self, marker: &PaintListMarker) {
        self.write_operation_prefix(PaintOrderPhase::ListMarker, "draw-list-marker");
        writeln!(
            &mut self.out,
            " source={} rect={} marker-kind={} color={} font-size={:.2}",
            source_debug_label(marker.source),
            rectangle_debug_label(marker.rect),
            list_marker_kind_debug_label(marker.kind),
            color_debug_label(marker.color),
            marker.font_size_px
        )
        .expect("write paint operation snapshot");
    }

    fn write_clip_operation(&mut self, kind: &'static str, clip: &PaintClip) {
        self.write_operation_prefix(PaintOrderPhase::OverflowClipForContentsAndDescendants, kind);
        writeln!(
            &mut self.out,
            " source={} rect={} scope={}",
            source_debug_label(clip.source),
            rectangle_debug_label(clip.rect),
            clip_scope_debug_label(clip.scope)
        )
        .expect("write paint operation snapshot");
    }

    fn write_text(&mut self, text: &PaintText) {
        self.write_operation_prefix(PaintOrderPhase::InlineFormattingContent, "draw-text");
        writeln!(
            &mut self.out,
            " source={} rect={} color={} font-size={:.2} text={:?}",
            source_debug_label(text.source),
            rectangle_debug_label(text.rect),
            color_debug_label(text.color),
            text.font_size_px,
            text.text
        )
        .expect("write paint operation snapshot");
    }

    fn write_text_decoration(&mut self, decoration: &PaintTextDecoration) {
        self.write_operation_prefix(PaintOrderPhase::InlineFormattingContent, "fill-rect");
        writeln!(
            &mut self.out,
            " detail=text-decoration source={} rect={} line={} color={} thickness={:.2}",
            source_debug_label(decoration.source),
            rectangle_debug_label(decoration.rect),
            text_decoration_line_debug_label(decoration.line),
            color_debug_label(decoration.color),
            decoration.thickness
        )
        .expect("write paint operation snapshot");
    }

    fn write_inline_box(&mut self, inline_box: &PaintInlineBox) {
        self.write_operation_prefix(PaintOrderPhase::InlineFormattingContent, "inline-box");
        writeln!(
            &mut self.out,
            " source={} rect={} fallback-background={}",
            optional_source_debug_label(inline_box.source),
            rectangle_debug_label(inline_box.rect),
            optional_color_debug_label(inline_box.fallback_background)
        )
        .expect("write paint operation snapshot");
    }

    fn write_replaced(&mut self, replaced: &PaintReplaced) {
        self.write_operation_prefix(PaintOrderPhase::InlineFormattingContent, "replaced");
        writeln!(
            &mut self.out,
            " source={} rect={} replaced-kind={}",
            optional_source_debug_label(replaced.source),
            rectangle_debug_label(replaced.rect),
            replaced_kind_debug_label(replaced.kind)
        )
        .expect("write paint operation snapshot");
    }

    fn write_operation_prefix(&mut self, phase: PaintOrderPhase, kind: &'static str) {
        write!(
            &mut self.out,
            "op[{}]: phase={} kind={}",
            self.next_index,
            phase.debug_label(),
            kind
        )
        .expect("write paint operation snapshot");
        self.next_index += 1;
    }
}

fn rectangle_debug_label(rect: Rectangle) -> String {
    format!(
        "x={:.2} y={:.2} w={:.2} h={:.2}",
        rect.x, rect.y, rect.width, rect.height
    )
}

fn source_debug_label(source: PaintSource) -> String {
    format!(
        "(box={} node={} anonymous={})",
        source.box_id, source.node_id.0, source.anonymous
    )
}

fn optional_source_debug_label(source: Option<PaintSource>) -> String {
    source
        .map(source_debug_label)
        .unwrap_or_else(|| "none".to_string())
}

fn color_debug_label(color: PaintColor) -> String {
    format!("rgba({},{},{},{})", color.r, color.g, color.b, color.a)
}

fn optional_color_debug_label(color: Option<PaintColor>) -> String {
    color
        .map(color_debug_label)
        .unwrap_or_else(|| "none".to_string())
}

fn list_marker_kind_debug_label(kind: PaintListMarkerKind) -> String {
    match kind {
        PaintListMarkerKind::Unordered => "unordered".to_string(),
        PaintListMarkerKind::Ordered(index) => format!("ordered({index})"),
    }
}

fn clip_scope_debug_label(scope: PaintClipScope) -> &'static str {
    match scope {
        PaintClipScope::ContentsAndDescendants => "contents-and-descendants",
    }
}

fn text_decoration_line_debug_label(line: PaintTextDecorationLine) -> &'static str {
    match line {
        PaintTextDecorationLine::Underline => "underline",
    }
}

fn replaced_kind_debug_label(kind: PaintReplacedKind) -> &'static str {
    match kind {
        PaintReplacedKind::Img => "img",
        PaintReplacedKind::InputText => "input-text",
        PaintReplacedKind::TextArea => "textarea",
        PaintReplacedKind::InputCheckbox => "input-checkbox",
        PaintReplacedKind::InputRadio => "input-radio",
        PaintReplacedKind::Button => "button",
    }
}

fn ancestor_overflow_clips(root: &LayoutBox<'_, '_>, source: PaintSource) -> Vec<PaintClip> {
    let mut clips = Vec::new();
    if collect_ancestor_overflow_clips(root, source, &mut clips) {
        clips.reverse();
    }
    clips
}

fn collect_ancestor_overflow_clips(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
    clips: &mut Vec<PaintClip>,
) -> bool {
    if PaintSource::from_layout(layout) == source {
        return true;
    }

    for child in &layout.children {
        if collect_ancestor_overflow_clips(child, source, clips) {
            if let Some(clip) = layout.overflow_clip() {
                clips.push(PaintClip {
                    source: PaintSource::from_layout(layout),
                    rect: clip.rect(),
                    scope: PaintClipScope::ContentsAndDescendants,
                });
            }
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use css::{ComputedStyle, Length};
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

    fn build_paint_operation_snapshot(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        let input = super::super::PaintPhaseInput::new(&layout).to_paint_input(&TestMeasurer);
        input.to_operation_debug_snapshot()
    }

    fn line_index(snapshot: &str, pattern: &str) -> usize {
        snapshot
            .lines()
            .position(|line| line.contains(pattern))
            .unwrap_or_else(|| panic!("snapshot should contain {pattern:?}\n{snapshot}"))
    }

    fn line_index_after(snapshot: &str, pattern: &str, after: usize) -> usize {
        snapshot
            .lines()
            .enumerate()
            .skip(after + 1)
            .find_map(|(index, line)| line.contains(pattern).then_some(index))
            .unwrap_or_else(|| {
                panic!("snapshot should contain {pattern:?} after line {after}\n{snapshot}")
            })
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

    #[test]
    fn paint_operation_snapshot_exact_representative_fixture() {
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
                    ("overflow".to_string(), "clip".to_string()),
                    ("background-color".to_string(), "#102030".to_string()),
                    ("border-top-width".to_string(), "2px".to_string()),
                    ("border-top-style".to_string(), "solid".to_string()),
                    ("border-top-color".to_string(), "#405060".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "#a0b0c0".to_string()),
                    ("font-size".to_string(), "20px".to_string()),
                    ("color".to_string(), "#aabbcc".to_string()),
                    ("text-decoration-line".to_string(), "underline".to_string()),
                ],
                vec![
                    Node::Text {
                        id: Id(3),
                        text: "AA8".to_string(),
                    },
                    html::internal::node_element_from_parts(
                        Id(4),
                        html::internal::html_name("div"),
                        Vec::new(),
                        vec![
                            ("display".to_string(), "block".to_string()),
                            ("width".to_string(), "40px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#708090".to_string()),
                        ],
                        Vec::new(),
                    ),
                ],
            )],
        };

        let snapshot = build_paint_operation_snapshot(&dom);
        let expected = concat!(
            "version: 1\n",
            "paint-operation-snapshot\n",
            "layout-root-id: 1\n",
            "viewport-width: 500.00\n",
            "document-rect: x=0.00 y=0.00 w=500.00 h=62.00\n",
            "op[0]: phase=box-background kind=fill-rect detail=background source=(box=1 node=2 anonymous=false) rect=x=0.00 y=0.00 w=120.00 h=62.00 color=rgba(16,32,48,255)\n",
            "op[1]: phase=box-border kind=fill-rect detail=border-top source=(box=1 node=2 anonymous=false) rect=x=0.00 y=0.00 w=120.00 h=2.00 color=rgba(64,80,96,255)\n",
            "op[2]: phase=overflow-clip-for-contents-and-descendants kind=begin-clip source=(box=1 node=2 anonymous=false) rect=x=0.00 y=0.00 w=120.00 h=62.00 scope=contents-and-descendants\n",
            "op[3]: phase=inline-formatting-content kind=draw-text source=(box=2 node=2 anonymous=true) rect=x=4.00 y=6.00 w=24.00 h=24.00 color=rgba(170,187,204,255) font-size=20.00 text=\"AA8\"\n",
            "op[4]: phase=inline-formatting-content kind=fill-rect detail=text-decoration source=(box=2 node=2 anonymous=true) rect=x=4.00 y=26.10 w=24.00 h=1.25 line=underline color=rgba(170,187,204,255) thickness=1.25\n",
            "op[5]: phase=box-background kind=fill-rect detail=background source=(box=3 node=3 anonymous=false) rect=x=0.00 y=0.00 w=500.00 h=0.00 color=rgba(16,32,48,255)\n",
            "op[6]: phase=box-border kind=fill-rect detail=border-top source=(box=3 node=3 anonymous=false) rect=x=0.00 y=0.00 w=500.00 h=2.00 color=rgba(64,80,96,255)\n",
            "op[7]: phase=box-outline kind=fill-rect detail=outline-top source=(box=3 node=3 anonymous=false) rect=x=-3.00 y=-3.00 w=506.00 h=3.00 color=rgba(160,176,192,255)\n",
            "op[8]: phase=box-outline kind=fill-rect detail=outline-right source=(box=3 node=3 anonymous=false) rect=x=500.00 y=0.00 w=3.00 h=0.00 color=rgba(160,176,192,255)\n",
            "op[9]: phase=box-outline kind=fill-rect detail=outline-bottom source=(box=3 node=3 anonymous=false) rect=x=-3.00 y=0.00 w=506.00 h=3.00 color=rgba(160,176,192,255)\n",
            "op[10]: phase=box-outline kind=fill-rect detail=outline-left source=(box=3 node=3 anonymous=false) rect=x=-3.00 y=0.00 w=3.00 h=0.00 color=rgba(160,176,192,255)\n",
            "op[11]: phase=box-background kind=fill-rect detail=background source=(box=4 node=4 anonymous=false) rect=x=0.00 y=34.00 w=40.00 h=20.00 color=rgba(112,128,144,255)\n",
            "op[12]: phase=overflow-clip-for-contents-and-descendants kind=end-clip source=(box=1 node=2 anonymous=false) rect=x=0.00 y=0.00 w=120.00 h=62.00 scope=contents-and-descendants\n",
            "op[13]: phase=box-outline kind=fill-rect detail=outline-top source=(box=1 node=2 anonymous=false) rect=x=-3.00 y=-3.00 w=126.00 h=3.00 color=rgba(160,176,192,255)\n",
            "op[14]: phase=box-outline kind=fill-rect detail=outline-right source=(box=1 node=2 anonymous=false) rect=x=120.00 y=0.00 w=3.00 h=62.00 color=rgba(160,176,192,255)\n",
            "op[15]: phase=box-outline kind=fill-rect detail=outline-bottom source=(box=1 node=2 anonymous=false) rect=x=-3.00 y=62.00 w=126.00 h=3.00 color=rgba(160,176,192,255)\n",
            "op[16]: phase=box-outline kind=fill-rect detail=outline-left source=(box=1 node=2 anonymous=false) rect=x=-3.00 y=0.00 w=3.00 h=62.00 color=rgba(160,176,192,255)\n",
        );

        assert_eq!(snapshot, expected);
    }

    #[test]
    fn paint_operation_snapshot_uses_ab3_z_index_layer_order() {
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

        let snapshot = build_paint_operation_snapshot(&dom);
        let negative = line_index(&snapshot, "color=rgba(170,0,0,255)");
        let normal = line_index(&snapshot, "color=rgba(0,170,0,255)");
        let zero = line_index(&snapshot, "color=rgba(0,0,170,255)");
        let positive = line_index(&snapshot, "color=rgba(170,170,0,255)");

        assert!(negative < normal);
        assert!(normal < zero);
        assert!(zero < positive);
    }

    #[test]
    fn paint_operation_snapshot_keeps_child_context_atomic_relative_to_siblings() {
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

        let snapshot = build_paint_operation_snapshot(&dom);
        let nested_negative = line_index(&snapshot, "color=rgba(0,170,0,255)");
        let parent = line_index(&snapshot, "color=rgba(170,0,0,255)");
        let sibling = line_index(&snapshot, "color=rgba(0,0,170,255)");

        assert!(nested_negative < parent);
        assert!(parent < sibling);
    }

    #[test]
    fn paint_operation_snapshot_keeps_positioned_child_context_under_ancestor_overflow_clip() {
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

        let snapshot = build_paint_operation_snapshot(&dom);
        let clip_pattern =
            "source=(box=1 node=2 anonymous=false) rect=x=0.00 y=0.00 w=40.00 h=20.00";
        let begin_clip = line_index_after(&snapshot, "kind=begin-clip", 5);
        let child_background = line_index(&snapshot, "color=rgba(170,0,0,255)");
        let end_clip = line_index_after(&snapshot, "kind=end-clip", child_background);

        assert!(
            snapshot
                .lines()
                .nth(begin_clip)
                .unwrap()
                .contains(clip_pattern)
        );
        assert!(
            snapshot
                .lines()
                .nth(end_clip)
                .unwrap()
                .contains(clip_pattern)
        );

        assert!(begin_clip < child_background);
        assert!(child_background < end_clip);
    }

    #[test]
    fn paint_operation_snapshot_is_deterministic_and_backend_independent() {
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
                    ("height".to_string(), "40px".to_string()),
                    ("background-color".to_string(), "#abcdef".to_string()),
                ],
                Vec::new(),
            )],
        };

        let first = build_paint_operation_snapshot(&dom);
        let second = build_paint_operation_snapshot(&dom);

        assert_eq!(first, second);
        assert!(first.starts_with("version: 1\npaint-operation-snapshot\n"));
        assert!(first.contains("layout-root-id: 1"));
        assert!(first.contains("op[0]: phase=box-background kind=fill-rect"));
        for backend_term in [
            "egui",
            "Shape",
            "Painter",
            "TextureId",
            "gpu",
            "compositor",
            "display-list",
        ] {
            assert!(
                !first.contains(backend_term),
                "operation snapshot must not expose backend internals: {backend_term}"
            );
        }
    }

    #[test]
    fn paint_operation_snapshot_covers_box_decorations_and_text() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "160px".to_string()),
                    ("height".to_string(), "50px".to_string()),
                    ("background-color".to_string(), "#112233".to_string()),
                    ("border-top-width".to_string(), "2px".to_string()),
                    ("border-top-style".to_string(), "solid".to_string()),
                    ("border-top-color".to_string(), "#445566".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "#778899".to_string()),
                    ("font-size".to_string(), "20px".to_string()),
                    ("color".to_string(), "#aabbcc".to_string()),
                    ("text-decoration-line".to_string(), "underline".to_string()),
                ],
                vec![Node::Text {
                    id: Id(3),
                    text: "Hello".to_string(),
                }],
            )],
        };

        let snapshot = build_paint_operation_snapshot(&dom);

        assert!(snapshot.contains(
            "phase=box-background kind=fill-rect detail=background source=(box=1 node=2 anonymous=false)"
        ));
        assert!(snapshot.contains(
            "phase=box-border kind=fill-rect detail=border-top source=(box=1 node=2 anonymous=false)"
        ));
        assert!(snapshot.contains(
            "phase=inline-formatting-content kind=draw-text source=(box=1 node=2 anonymous=false)"
        ));
        assert!(snapshot.contains("text=\"Hello\""));
        assert!(snapshot.contains("detail=text-decoration"));
        assert!(snapshot.contains("line=underline"));
        assert!(snapshot.contains(
            "phase=box-outline kind=fill-rect detail=outline-top source=(box=1 node=2 anonymous=false)"
        ));
    }

    #[test]
    fn paint_operation_snapshot_scopes_overflow_clip_to_contents_and_descendants() {
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
                    ("overflow".to_string(), "clip".to_string()),
                    ("background-color".to_string(), "#102030".to_string()),
                    ("border-top-width".to_string(), "2px".to_string()),
                    ("border-top-style".to_string(), "solid".to_string()),
                    ("border-top-color".to_string(), "#405060".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "#a0b0c0".to_string()),
                ],
                vec![
                    Node::Text {
                        id: Id(3),
                        text: "Clipped".to_string(),
                    },
                    html::internal::node_element_from_parts(
                        Id(4),
                        html::internal::html_name("div"),
                        Vec::new(),
                        vec![
                            ("display".to_string(), "block".to_string()),
                            ("width".to_string(), "150px".to_string()),
                            ("height".to_string(), "20px".to_string()),
                            ("background-color".to_string(), "#708090".to_string()),
                        ],
                        Vec::new(),
                    ),
                ],
            )],
        };

        let snapshot = build_paint_operation_snapshot(&dom);
        let parent_background = line_index(&snapshot, "detail=background source=(box=1 node=2");
        let parent_border = line_index(&snapshot, "detail=border-top source=(box=1 node=2");
        let begin_clip = line_index(&snapshot, "kind=begin-clip source=(box=1 node=2");
        let text = line_index(&snapshot, "kind=draw-text");
        let child_background = line_index(&snapshot, "node=4 anonymous=false");
        let end_clip = line_index(&snapshot, "kind=end-clip source=(box=1 node=2");
        let parent_outline = line_index(&snapshot, "detail=outline-top source=(box=1 node=2");

        assert!(parent_background < begin_clip);
        assert!(parent_border < begin_clip);
        assert!(begin_clip < text);
        assert!(text < child_background);
        assert!(child_background < end_clip);
        assert!(end_clip < parent_outline);
        assert!(snapshot.contains("scope=contents-and-descendants"));
    }

    #[test]
    fn paint_operation_snapshot_records_list_marker_and_inline_content() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("ul"),
                Vec::new(),
                Vec::new(),
                vec![html::internal::node_element_from_parts(
                    Id(3),
                    html::internal::html_name("li"),
                    Vec::new(),
                    vec![("padding-left".to_string(), "20px".to_string())],
                    vec![
                        Node::Text {
                            id: Id(4),
                            text: "Item ".to_string(),
                        },
                        html::internal::node_element_from_parts(
                            Id(5),
                            html::internal::html_name("span"),
                            Vec::new(),
                            vec![("display".to_string(), "inline-block".to_string())],
                            vec![Node::Text {
                                id: Id(6),
                                text: "Box".to_string(),
                            }],
                        ),
                    ],
                )],
            )],
        };

        let snapshot = build_paint_operation_snapshot(&dom);

        assert!(snapshot.contains(
            "phase=list-marker kind=draw-list-marker source=(box=2 node=3 anonymous=false)"
        ));
        assert!(snapshot.contains("marker-kind=unordered"));
        assert!(snapshot.contains("phase=inline-formatting-content kind=draw-text"));
        assert!(snapshot.contains("phase=inline-formatting-content kind=inline-box"));
        assert!(snapshot.contains("node=5 anonymous=false"));
    }

    #[test]
    fn paint_operation_snapshot_records_replaced_image_structurally_without_resource_state() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("section"),
                Vec::new(),
                vec![("display".to_string(), "block".to_string())],
                vec![html::internal::node_element_from_parts(
                    Id(3),
                    html::internal::html_name("img"),
                    vec![html::internal::unqualified_attribute("src", "image.png")],
                    vec![
                        ("width".to_string(), "40px".to_string()),
                        ("height".to_string(), "20px".to_string()),
                    ],
                    Vec::new(),
                )],
            )],
        };

        let snapshot = build_paint_operation_snapshot(&dom);

        assert!(snapshot.contains("phase=inline-formatting-content kind=replaced"));
        assert!(snapshot.contains("source=(box=2 node=3 anonymous=false)"));
        assert!(snapshot.contains("replaced-kind=img"));
        assert!(!snapshot.contains("image.png"));
        assert!(!snapshot.contains("TextureId"));
    }

    #[test]
    fn paint_operation_snapshot_preserves_aa_ordering_phases() {
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

        let snapshot = build_paint_operation_snapshot(&dom);
        let parent_background = line_index(&snapshot, "phase=box-background");
        let parent_border = line_index(&snapshot, "phase=box-border");
        let child_background = line_index(&snapshot, "source=(box=2 node=3");
        let parent_outline = line_index(&snapshot, "phase=box-outline");

        assert!(parent_background < parent_border);
        assert!(parent_border < child_background);
        assert!(child_background < parent_outline);
    }
}
