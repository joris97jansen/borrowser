use std::fmt::Write;

use css::{Display, Length};
use html::dom_utils::is_non_rendering_element;
use html::internal::Id;
use layout::inline::{InlineFragment, layout_inline_for_paint};
use layout::{LayoutBox, LayoutPhaseOutput, ListMarker, Rectangle, ReplacedKind, TextMeasurer};

use super::PaintPhaseInput;

/// Paint-owned semantic input derived from layout output for one paint phase.
///
/// This is frame-local semantic paint data. It is not a retained display list,
/// scene graph, backend command buffer, or compositor structure.
pub struct PaintInput<'layout, 'style_tree, 'dom> {
    layout: &'layout LayoutPhaseOutput<'style_tree, 'dom>,
    tree: PaintTree,
}

impl<'layout, 'style_tree, 'dom> PaintInput<'layout, 'style_tree, 'dom> {
    pub fn from_phase_input(
        input: PaintPhaseInput<'layout, 'style_tree, 'dom>,
        measurer: &dyn TextMeasurer,
    ) -> Self {
        Self {
            layout: input.layout(),
            tree: PaintTree::from_layout(input.layout_root(), measurer),
        }
    }

    pub fn layout(&self) -> &'layout LayoutPhaseOutput<'style_tree, 'dom> {
        self.layout
    }

    pub fn tree(&self) -> &PaintTree {
        &self.tree
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write paint input snapshot");
        writeln!(&mut out, "paint-input").expect("write paint input snapshot");
        writeln!(
            &mut out,
            "layout-root-id: {}",
            self.layout.root().node_id().0
        )
        .expect("write paint input snapshot");
        writeln!(
            &mut out,
            "viewport-width: {:.2}",
            self.layout.viewport_width()
        )
        .expect("write paint input snapshot");
        writeln!(
            &mut out,
            "document-rect: {}",
            rectangle_debug_label(self.layout.document_rect())
        )
        .expect("write paint input snapshot");
        self.tree
            .append_debug_snapshot(&mut out)
            .expect("write paint input snapshot");
        out
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaintTree {
    root: PaintNode,
}

impl PaintTree {
    pub fn root(&self) -> &PaintNode {
        &self.root
    }

    fn from_layout(layout: &LayoutBox<'_, '_>, measurer: &dyn TextMeasurer) -> Self {
        Self {
            root: PaintNode::from_layout(layout, measurer),
        }
    }

    fn append_debug_snapshot(&self, out: &mut String) -> std::fmt::Result {
        writeln!(out, "paint-tree")?;
        self.root.append_debug_snapshot(out, 0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaintNode {
    source: PaintSource,
    primitives: Vec<PaintPrimitive>,
    children: Vec<PaintNode>,
}

impl PaintNode {
    pub fn source(&self) -> PaintSource {
        self.source
    }

    pub fn primitives(&self) -> &[PaintPrimitive] {
        &self.primitives
    }

    pub fn children(&self) -> &[PaintNode] {
        &self.children
    }

    fn from_layout(layout: &LayoutBox<'_, '_>, measurer: &dyn TextMeasurer) -> Self {
        let source = PaintSource::from_layout(layout);
        if is_non_rendering_element(layout.node.node) {
            return Self {
                source,
                primitives: Vec::new(),
                children: Vec::new(),
            };
        }

        let mut primitives = Vec::new();
        append_box_primitives(layout, measurer, &mut primitives);

        let children = layout
            .children
            .iter()
            .map(|child| PaintNode::from_layout(child, measurer))
            .collect();

        Self {
            source,
            primitives,
            children,
        }
    }

    fn append_debug_snapshot(&self, out: &mut String, depth: usize) -> std::fmt::Result {
        let indent = "  ".repeat(depth);
        writeln!(
            out,
            "{}box={} node={} anonymous={} primitives={} children={}",
            indent,
            self.source.box_id,
            self.source.node_id.0,
            self.source.anonymous,
            self.primitives.len(),
            self.children.len()
        )?;
        for primitive in &self.primitives {
            writeln!(out, "{}  primitive {}", indent, primitive.to_debug_label())?;
        }
        for child in &self.children {
            child.append_debug_snapshot(out, depth + 1)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintSource {
    pub box_id: usize,
    pub node_id: Id,
    pub anonymous: bool,
}

impl PaintSource {
    fn from_layout(layout: &LayoutBox<'_, '_>) -> Self {
        Self {
            box_id: layout.box_id().index(),
            node_id: layout.node_id(),
            anonymous: layout.is_anonymous(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaintPrimitive {
    Background(PaintBackground),
    Border(PaintBorder),
    ListMarker(PaintListMarker),
    Clip(PaintClip),
    Text(PaintText),
    InlineBox(PaintInlineBox),
    Replaced(PaintReplaced),
}

impl PaintPrimitive {
    pub fn kind(&self) -> PaintPrimitiveKind {
        match self {
            Self::Background(_) => PaintPrimitiveKind::Background,
            Self::Border(_) => PaintPrimitiveKind::Border,
            Self::ListMarker(_) => PaintPrimitiveKind::ListMarker,
            Self::Clip(_) => PaintPrimitiveKind::Clip,
            Self::Text(_) => PaintPrimitiveKind::Text,
            Self::InlineBox(_) => PaintPrimitiveKind::InlineBox,
            Self::Replaced(_) => PaintPrimitiveKind::Replaced,
        }
    }

    fn to_debug_label(&self) -> String {
        match self {
            Self::Background(background) => format!(
                "background rect={} color={}",
                rectangle_debug_label(background.rect),
                background.color.to_debug_label()
            ),
            Self::Border(border) => format!(
                "border rect={} top={:.2} right={:.2} bottom={:.2} left={:.2}",
                rectangle_debug_label(border.rect),
                border.edges.top.width,
                border.edges.right.width,
                border.edges.bottom.width,
                border.edges.left.width
            ),
            Self::ListMarker(marker) => format!(
                "list-marker rect={} kind={}",
                rectangle_debug_label(marker.rect),
                marker.kind.to_debug_label()
            ),
            Self::Clip(clip) => format!(
                "clip rect={} scope={}",
                rectangle_debug_label(clip.rect),
                clip.scope.to_debug_label()
            ),
            Self::Text(text) => format!(
                "text rect={} color={} font-size={:.2} text={:?}",
                rectangle_debug_label(text.rect),
                text.color.to_debug_label(),
                text.font_size_px,
                text.text
            ),
            Self::InlineBox(inline_box) => format!(
                "inline-box rect={} source={}",
                rectangle_debug_label(inline_box.rect),
                optional_source_debug_label(inline_box.source)
            ),
            Self::Replaced(replaced) => format!(
                "replaced rect={} kind={} source={}",
                rectangle_debug_label(replaced.rect),
                replaced.kind.to_debug_label(),
                optional_source_debug_label(replaced.source)
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintPrimitiveKind {
    Background,
    Border,
    ListMarker,
    Clip,
    Text,
    InlineBox,
    Replaced,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintBackground {
    pub source: PaintSource,
    pub rect: Rectangle,
    pub color: PaintColor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintBorder {
    pub source: PaintSource,
    pub rect: Rectangle,
    pub edges: PaintBorderEdges,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintBorderEdges {
    pub top: PaintBorderSide,
    pub right: PaintBorderSide,
    pub bottom: PaintBorderSide,
    pub left: PaintBorderSide,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintBorderSide {
    pub width: f32,
    pub color: PaintColor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintListMarker {
    pub source: PaintSource,
    pub rect: Rectangle,
    pub kind: PaintListMarkerKind,
    pub color: PaintColor,
    pub font_size_px: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintListMarkerKind {
    Unordered,
    Ordered(u32),
}

impl PaintListMarkerKind {
    fn to_debug_label(self) -> String {
        match self {
            Self::Unordered => "unordered".to_string(),
            Self::Ordered(index) => format!("ordered({index})"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintClip {
    pub source: PaintSource,
    pub rect: Rectangle,
    pub scope: PaintClipScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintClipScope {
    ContentsAndDescendants,
}

impl PaintClipScope {
    fn to_debug_label(self) -> &'static str {
        match self {
            Self::ContentsAndDescendants => "contents-and-descendants",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaintText {
    pub source: PaintSource,
    pub rect: Rectangle,
    pub text: String,
    pub color: PaintColor,
    pub font_size_px: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintInlineBox {
    pub source: Option<PaintSource>,
    pub rect: Rectangle,
    pub fallback_background: Option<PaintColor>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintReplaced {
    pub source: Option<PaintSource>,
    pub rect: Rectangle,
    pub kind: PaintReplacedKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintReplacedKind {
    Img,
    InputText,
    TextArea,
    InputCheckbox,
    InputRadio,
    Button,
}

impl PaintReplacedKind {
    fn to_debug_label(self) -> &'static str {
        match self {
            Self::Img => "img",
            Self::InputText => "input-text",
            Self::TextArea => "textarea",
            Self::InputCheckbox => "input-checkbox",
            Self::InputRadio => "input-radio",
            Self::Button => "button",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl PaintColor {
    pub fn from_rgba((r, g, b, a): (u8, u8, u8, u8)) -> Self {
        Self { r, g, b, a }
    }

    pub fn is_visible(self) -> bool {
        self.a > 0
    }

    fn to_debug_label(self) -> String {
        format!("rgba({},{},{},{})", self.r, self.g, self.b, self.a)
    }
}

fn append_box_primitives(
    layout: &LayoutBox<'_, '_>,
    measurer: &dyn TextMeasurer,
    primitives: &mut Vec<PaintPrimitive>,
) {
    let source = PaintSource::from_layout(layout);
    append_background_primitive(layout, source, primitives);
    append_list_marker_primitive(layout, source, primitives);
    append_clip_primitive(layout, source, primitives);
    append_inline_primitives(layout, measurer, primitives);
}

fn append_background_primitive(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
    primitives: &mut Vec<PaintPrimitive>,
) {
    let color = PaintColor::from_rgba(layout.style.background_color());
    if layout.is_anonymous() || !color.is_visible() {
        return;
    }

    primitives.push(PaintPrimitive::Background(PaintBackground {
        source,
        rect: layout.rect,
        color,
    }));
}

fn append_list_marker_primitive(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
    primitives: &mut Vec<PaintPrimitive>,
) {
    let marker = match layout.list_marker {
        Some(marker)
            if !layout.is_anonymous() && matches!(layout.style.display(), Display::ListItem) =>
        {
            marker
        }
        _ => return,
    };

    let metrics = layout.box_metrics();
    let marker_rect = Rectangle {
        x: layout.rect.x,
        y: layout.rect.y + metrics.padding_top,
        width: metrics.padding_left,
        height: layout.content_height(),
    };

    primitives.push(PaintPrimitive::ListMarker(PaintListMarker {
        source,
        rect: marker_rect,
        kind: match marker {
            ListMarker::Unordered => PaintListMarkerKind::Unordered,
            ListMarker::Ordered(index) => PaintListMarkerKind::Ordered(index),
        },
        color: PaintColor::from_rgba(layout.style.color()),
        font_size_px: font_size_px(layout.style.font_size()),
    }));
}

fn append_clip_primitive(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
    primitives: &mut Vec<PaintPrimitive>,
) {
    if let Some(clip) = layout.overflow_clip() {
        primitives.push(PaintPrimitive::Clip(PaintClip {
            source,
            rect: clip.rect(),
            scope: PaintClipScope::ContentsAndDescendants,
        }));
    }
}

fn append_inline_primitives(
    layout: &LayoutBox<'_, '_>,
    measurer: &dyn TextMeasurer,
    primitives: &mut Vec<PaintPrimitive>,
) {
    if layout.replaced.is_some() {
        return;
    }

    match layout.node.node {
        html::Node::Element { .. } => {
            if matches!(layout.style.display(), Display::Inline) {
                return;
            }
        }
        html::Node::Document { .. } => return,
        _ => return,
    }

    let (content_x, content_width) = layout.content_x_and_width();
    let block_rect = Rectangle {
        x: content_x,
        y: layout.content_y(),
        width: content_width,
        height: layout.content_height(),
    };

    for line in layout_inline_for_paint(measurer, block_rect, layout) {
        for fragment in line.fragments {
            match fragment.kind {
                InlineFragment::Text { text, style, .. } => {
                    primitives.push(PaintPrimitive::Text(PaintText {
                        source: PaintSource::from_layout(layout),
                        rect: fragment.paint_rect.rect(),
                        text,
                        color: PaintColor::from_rgba(style.color()),
                        font_size_px: font_size_px(style.font_size()),
                    }));
                }
                InlineFragment::Box { style, layout, .. } => {
                    let fallback = PaintColor::from_rgba(style.background_color());
                    primitives.push(PaintPrimitive::InlineBox(PaintInlineBox {
                        source: layout.map(PaintSource::from_layout),
                        rect: fragment.paint_rect.rect(),
                        fallback_background: fallback.is_visible().then_some(fallback),
                    }));
                }
                InlineFragment::Replaced { kind, layout, .. } => {
                    primitives.push(PaintPrimitive::Replaced(PaintReplaced {
                        source: layout.map(PaintSource::from_layout),
                        rect: fragment.paint_rect.rect(),
                        kind: PaintReplacedKind::from_layout(kind),
                    }));
                }
            }
        }
    }
}

impl PaintReplacedKind {
    fn from_layout(kind: ReplacedKind) -> Self {
        match kind {
            ReplacedKind::Img => Self::Img,
            ReplacedKind::InputText => Self::InputText,
            ReplacedKind::TextArea => Self::TextArea,
            ReplacedKind::InputCheckbox => Self::InputCheckbox,
            ReplacedKind::InputRadio => Self::InputRadio,
            ReplacedKind::Button => Self::Button,
        }
    }
}

fn font_size_px(length: Length) -> f32 {
    match length {
        Length::Px(px) => px,
    }
}

fn rectangle_debug_label(rect: Rectangle) -> String {
    format!(
        "x={:.2} y={:.2} w={:.2} h={:.2}",
        rect.x, rect.y, rect.width, rect.height
    )
}

fn optional_source_debug_label(source: Option<PaintSource>) -> String {
    source
        .map(|source| format!("box={} node={}", source.box_id, source.node_id.0))
        .unwrap_or_else(|| "none".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use css::{ComputedStyle, Length};
    use html::{Node, internal::Id};
    use layout::LayoutPhaseInput;
    use std::sync::Arc;

    struct TestMeasurer;

    impl TextMeasurer for TestMeasurer {
        fn measure(&self, text: &str, _style: &ComputedStyle) -> f32 {
            text.chars().count() as f32 * 8.0
        }

        fn line_height(&self, style: &ComputedStyle) -> f32 {
            let Length::Px(px) = style.font_size();
            px * 1.2
        }
    }

    fn build_layout_for<'style_tree, 'dom>(
        styled: &'style_tree css::StyledNode<'dom>,
    ) -> LayoutPhaseOutput<'style_tree, 'dom> {
        layout::layout_document(LayoutPhaseInput::new(styled, 500.0, &TestMeasurer, None))
    }

    fn build_paint_input<'layout, 'style_tree, 'dom>(
        layout: &'layout LayoutPhaseOutput<'style_tree, 'dom>,
    ) -> PaintInput<'layout, 'style_tree, 'dom> {
        PaintInput::from_phase_input(PaintPhaseInput::new(layout), &TestMeasurer)
    }

    fn build_style_tree<'dom>(dom: &'dom Node) -> css::StyledNode<'dom> {
        css::build_style_tree(dom, None)
    }

    fn build_paint_input_for_debug(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        PaintInput::from_phase_input(PaintPhaseInput::new(&layout), &TestMeasurer)
            .to_debug_snapshot()
    }

    fn first_node_with_primitive(node: &PaintNode, kind: PaintPrimitiveKind) -> Option<&PaintNode> {
        if node
            .primitives()
            .iter()
            .any(|primitive| primitive.kind() == kind)
        {
            return Some(node);
        }

        node.children()
            .iter()
            .find_map(|child| first_node_with_primitive(child, kind))
    }

    fn primitive_kinds(node: &PaintNode) -> Vec<PaintPrimitiveKind> {
        node.primitives().iter().map(PaintPrimitive::kind).collect()
    }

    #[test]
    fn paint_input_keeps_layout_phase_output_as_authoritative_handoff() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: Vec::new(),
        };
        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = PaintPhaseInput::new(&layout).to_paint_input(&TestMeasurer);

        assert_eq!(input.layout().viewport_width(), 500.0);
        assert_eq!(input.tree().root().source().node_id, Id(1));
    }

    #[test]
    fn paint_tree_emits_background_clip_and_text_primitives_in_supported_order() {
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
                    ("height".to_string(), "40px".to_string()),
                    ("background-color".to_string(), "#112233".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                ],
                children: vec![Node::Text {
                    id: Id(3),
                    text: "Hello".to_string(),
                }],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let node = first_node_with_primitive(input.tree().root(), PaintPrimitiveKind::Background)
            .expect("section paint node");

        assert_eq!(
            primitive_kinds(node),
            vec![
                PaintPrimitiveKind::Background,
                PaintPrimitiveKind::Clip,
                PaintPrimitiveKind::Text,
            ]
        );

        assert!(node.primitives().iter().any(|primitive| matches!(
            primitive,
            PaintPrimitive::Text(PaintText { text, .. }) if text == "Hello"
        )));
    }

    #[test]
    fn paint_tree_derives_list_marker_from_layout_metadata() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("ul"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("li"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: vec![Node::Text {
                        id: Id(4),
                        text: "Item".to_string(),
                    }],
                }],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let marker_node =
            first_node_with_primitive(input.tree().root(), PaintPrimitiveKind::ListMarker)
                .expect("list item marker node");

        assert!(marker_node.primitives().iter().any(|primitive| matches!(
            primitive,
            PaintPrimitive::ListMarker(PaintListMarker {
                kind: PaintListMarkerKind::Unordered,
                ..
            })
        )));
    }

    #[test]
    fn paint_input_debug_snapshot_is_deterministic_and_semantic() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("background-color".to_string(), "#abcdef".to_string())],
                children: Vec::new(),
            }],
        };

        let first = build_paint_input_for_debug(&dom);
        let second = build_paint_input_for_debug(&dom);

        assert_eq!(first, second);
        assert!(first.contains("paint-input"));
        assert!(first.contains("primitive background"));
        assert!(!first.contains("egui"));
    }
}
