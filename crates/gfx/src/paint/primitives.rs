use std::fmt::Write;

use css::{Display, Length, TextDecorationLine};
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
    post_primitives: Vec<PaintPrimitive>,
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

    pub fn post_primitives(&self) -> &[PaintPrimitive] {
        &self.post_primitives
    }

    fn from_layout(layout: &LayoutBox<'_, '_>, measurer: &dyn TextMeasurer) -> Self {
        let source = PaintSource::from_layout(layout);
        if is_non_rendering_element(layout.node.node) {
            return Self {
                source,
                primitives: Vec::new(),
                children: Vec::new(),
                post_primitives: Vec::new(),
            };
        }

        let mut primitives = Vec::new();
        append_box_primitives(layout, measurer, &mut primitives);
        let mut post_primitives = Vec::new();
        append_post_child_primitives(layout, source, &mut post_primitives);

        let children = layout
            .children
            .iter()
            .map(|child| PaintNode::from_layout(child, measurer))
            .collect();

        Self {
            source,
            primitives,
            children,
            post_primitives,
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
            self.primitives.len() + self.post_primitives.len(),
            self.children.len()
        )?;
        for primitive in &self.primitives {
            writeln!(out, "{}  primitive {}", indent, primitive.to_debug_label())?;
        }
        for child in &self.children {
            child.append_debug_snapshot(out, depth + 1)?;
        }
        for primitive in &self.post_primitives {
            writeln!(
                out,
                "{}  post-primitive {}",
                indent,
                primitive.to_debug_label()
            )?;
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
    pub(super) fn from_layout(layout: &LayoutBox<'_, '_>) -> Self {
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
    Outline(PaintOutline),
    ListMarker(PaintListMarker),
    Clip(PaintClip),
    Text(PaintText),
    TextDecoration(PaintTextDecoration),
    InlineBox(PaintInlineBox),
    Replaced(PaintReplaced),
}

impl PaintPrimitive {
    pub fn kind(&self) -> PaintPrimitiveKind {
        match self {
            Self::Background(_) => PaintPrimitiveKind::Background,
            Self::Border(_) => PaintPrimitiveKind::Border,
            Self::Outline(_) => PaintPrimitiveKind::Outline,
            Self::ListMarker(_) => PaintPrimitiveKind::ListMarker,
            Self::Clip(_) => PaintPrimitiveKind::Clip,
            Self::Text(_) => PaintPrimitiveKind::Text,
            Self::TextDecoration(_) => PaintPrimitiveKind::TextDecoration,
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
                "border rect={} top=({:.2},{}) right=({:.2},{}) bottom=({:.2},{}) left=({:.2},{})",
                rectangle_debug_label(border.rect),
                border.edges.top.width,
                border.edges.top.color.to_debug_label(),
                border.edges.right.width,
                border.edges.right.color.to_debug_label(),
                border.edges.bottom.width,
                border.edges.bottom.color.to_debug_label(),
                border.edges.left.width,
                border.edges.left.color.to_debug_label()
            ),
            Self::Outline(outline) => format!(
                "outline border-rect={} outer-rect={} width={:.2} color={}",
                rectangle_debug_label(outline.border_rect),
                rectangle_debug_label(outline.outer_rect),
                outline.width,
                outline.color.to_debug_label()
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
            Self::TextDecoration(decoration) => format!(
                "text-decoration rect={} line={} color={} thickness={:.2}",
                rectangle_debug_label(decoration.rect),
                decoration.line.to_debug_label(),
                decoration.color.to_debug_label(),
                decoration.thickness
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
    Outline,
    ListMarker,
    Clip,
    Text,
    TextDecoration,
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
pub struct PaintOutline {
    pub source: PaintSource,
    pub border_rect: Rectangle,
    pub outer_rect: Rectangle,
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
pub struct PaintTextDecoration {
    pub source: PaintSource,
    pub rect: Rectangle,
    pub line: PaintTextDecorationLine,
    pub color: PaintColor,
    pub thickness: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintTextDecorationLine {
    Underline,
}

impl PaintTextDecorationLine {
    fn to_debug_label(self) -> &'static str {
        match self {
            Self::Underline => "underline",
        }
    }
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
    append_border_primitive(layout, source, primitives);
    append_list_marker_primitive(layout, source, primitives);
    append_clip_primitive(layout, source, primitives);
    append_inline_primitives(layout, measurer, primitives);
}

fn append_post_child_primitives(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
    primitives: &mut Vec<PaintPrimitive>,
) {
    if let Some(outline) = outline_primitive_from_layout(layout, source) {
        primitives.push(PaintPrimitive::Outline(outline));
    }
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

pub(super) fn outline_primitive_from_layout(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
) -> Option<PaintOutline> {
    if layout.is_anonymous() {
        return None;
    }

    let outline = layout.style.outline();
    if !outline.is_paint_visible() {
        return None;
    }

    let width = outline.width;
    let border_rect = layout.rect;
    let outer_rect = Rectangle {
        x: border_rect.x - width,
        y: border_rect.y - width,
        width: border_rect.width + width * 2.0,
        height: border_rect.height + width * 2.0,
    };

    Some(PaintOutline {
        source,
        border_rect,
        outer_rect,
        width,
        color: PaintColor::from_rgba(outline.color),
    })
}

fn append_border_primitive(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
    primitives: &mut Vec<PaintPrimitive>,
) {
    if let Some(border) = border_primitive_from_layout(layout, source) {
        primitives.push(PaintPrimitive::Border(border));
    }
}

pub(super) fn border_primitive_from_layout(
    layout: &LayoutBox<'_, '_>,
    source: PaintSource,
) -> Option<PaintBorder> {
    if layout.is_anonymous() {
        return None;
    }

    let edges = layout.style.border_edges();
    let border = PaintBorder {
        source,
        rect: layout.rect,
        edges: PaintBorderEdges {
            top: paint_border_side(edges.top),
            right: paint_border_side(edges.right),
            bottom: paint_border_side(edges.bottom),
            left: paint_border_side(edges.left),
        },
    };

    border.has_visible_side().then_some(border)
}

impl PaintBorder {
    pub fn has_visible_side(&self) -> bool {
        [
            self.edges.top,
            self.edges.right,
            self.edges.bottom,
            self.edges.left,
        ]
        .iter()
        .any(|side| side.is_visible())
    }
}

impl PaintBorderSide {
    pub fn is_visible(self) -> bool {
        self.width > 0.0 && self.color.is_visible()
    }
}

fn paint_border_side(side: css::BorderSide) -> PaintBorderSide {
    PaintBorderSide {
        width: if side.is_paint_visible() {
            side.used_width()
        } else {
            0.0
        },
        color: PaintColor::from_rgba(side.color),
    }
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
            let fragment_rect = fragment.paint_rect.rect();
            let fragment_ascent = fragment.ascent;
            let fragment_baseline_shift = fragment.baseline_shift;
            match fragment.kind {
                InlineFragment::Text {
                    text,
                    style,
                    decoration,
                    ..
                } => {
                    primitives.push(PaintPrimitive::Text(PaintText {
                        source: PaintSource::from_layout(layout),
                        rect: fragment.paint_rect.rect(),
                        text,
                        color: PaintColor::from_rgba(style.color()),
                        font_size_px: font_size_px(style.font_size()),
                    }));
                    if let Some(decoration) = decoration
                        && let Some(primitive) = text_decoration_primitive_from_fragment(
                            layout,
                            fragment_rect,
                            fragment_ascent,
                            fragment_baseline_shift,
                            decoration,
                        )
                    {
                        primitives.push(PaintPrimitive::TextDecoration(primitive));
                    }
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

fn text_decoration_primitive_from_fragment(
    layout: &LayoutBox<'_, '_>,
    text_rect: Rectangle,
    fragment_ascent: f32,
    fragment_baseline_shift: f32,
    decoration: layout::inline::InlineTextDecoration,
) -> Option<PaintTextDecoration> {
    if !matches!(decoration.line, TextDecorationLine::Underline) {
        return None;
    }

    if text_rect.width <= 0.0 || decoration.thickness <= 0.0 || decoration.color.3 == 0 {
        return None;
    }

    let baseline = text_rect.y + fragment_ascent + fragment_baseline_shift;
    let rect = Rectangle {
        x: text_rect.x,
        y: baseline + decoration.underline_offset,
        width: text_rect.width,
        height: decoration.thickness,
    };

    Some(PaintTextDecoration {
        source: PaintSource::from_layout(layout),
        rect,
        line: PaintTextDecorationLine::Underline,
        color: PaintColor::from_rgba(decoration.color),
        thickness: decoration.thickness,
    })
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
    use css::{ComputedStyle, Length, TextDecorationLine};
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
            .chain(node.post_primitives().iter())
            .any(|primitive| primitive.kind() == kind)
        {
            return Some(node);
        }

        node.children()
            .iter()
            .find_map(|child| first_node_with_primitive(child, kind))
    }

    fn non_anonymous_node_by_source_id(node: &PaintNode, id: Id) -> Option<&PaintNode> {
        if node.source().node_id == id && !node.source().anonymous {
            return Some(node);
        }

        node.children()
            .iter()
            .find_map(|child| non_anonymous_node_by_source_id(child, id))
    }

    fn primitive_kinds(node: &PaintNode) -> Vec<PaintPrimitiveKind> {
        node.primitives().iter().map(PaintPrimitive::kind).collect()
    }

    fn flattened_paint_order(node: &PaintNode) -> Vec<PaintPrimitiveKind> {
        let mut order = node
            .primitives()
            .iter()
            .map(PaintPrimitive::kind)
            .collect::<Vec<_>>();
        for child in node.children() {
            order.extend(flattened_paint_order(child));
        }
        order.extend(node.post_primitives().iter().map(PaintPrimitive::kind));
        order
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
                    ("border-top-width".to_string(), "2px".to_string()),
                    ("border-top-style".to_string(), "solid".to_string()),
                    ("border-top-color".to_string(), "red".to_string()),
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
                PaintPrimitiveKind::Border,
                PaintPrimitiveKind::Clip,
                PaintPrimitiveKind::Text,
            ]
        );

        assert!(node.primitives().iter().any(|primitive| matches!(
            primitive,
            PaintPrimitive::Border(PaintBorder { edges, .. })
                if edges.top.width == 2.0 && edges.top.color == PaintColor::from_rgba((255, 0, 0, 255))
        )));
        assert!(node.primitives().iter().any(|primitive| matches!(
            primitive,
            PaintPrimitive::Text(PaintText { text, .. }) if text == "Hello"
        )));
    }

    #[test]
    fn paint_tree_clip_primitive_uses_layout_owned_rect_and_scope() {
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
                    ("overflow".to_string(), "hidden".to_string()),
                ],
                children: Vec::new(),
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let node = non_anonymous_node_by_source_id(input.tree().root(), Id(2))
            .expect("section paint node");
        let clip = node
            .primitives()
            .iter()
            .find_map(|primitive| match primitive {
                PaintPrimitive::Clip(clip) => Some(*clip),
                _ => None,
            })
            .expect("layout-owned overflow clip primitive");

        assert_eq!(clip.source.node_id, Id(2));
        assert_eq!(
            clip.rect,
            Rectangle {
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 40.0,
            }
        );
        assert_eq!(clip.scope, PaintClipScope::ContentsAndDescendants);
        assert!(input.to_debug_snapshot().contains(
            "primitive clip rect=x=0.00 y=0.00 w=120.00 h=40.00 scope=contents-and-descendants"
        ));
    }

    #[test]
    fn paint_tree_emits_clips_only_for_layout_provided_overflow_clips() {
        for (node_id, overflow) in [(2, "hidden"), (3, "clip"), (4, "scroll"), (5, "auto")] {
            let dom = Node::Document {
                id: Id(1),
                doctype: None,
                children: vec![Node::Element {
                    id: Id(node_id),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("display".to_string(), "block".to_string()),
                        ("width".to_string(), "80px".to_string()),
                        ("height".to_string(), "30px".to_string()),
                        ("overflow".to_string(), overflow.to_string()),
                    ],
                    children: Vec::new(),
                }],
            };

            let styled = build_style_tree(&dom);
            let layout = build_layout_for(&styled);
            let input = build_paint_input(&layout);
            let node = non_anonymous_node_by_source_id(input.tree().root(), Id(node_id))
                .expect("section paint node");

            assert!(
                node.primitives()
                    .iter()
                    .any(|primitive| matches!(primitive, PaintPrimitive::Clip(_))),
                "overflow:{overflow} should emit a layout-provided clip primitive"
            );
        }

        let visible_dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(10),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "80px".to_string()),
                    ("height".to_string(), "30px".to_string()),
                    ("overflow".to_string(), "visible".to_string()),
                ],
                children: Vec::new(),
            }],
        };
        let styled = build_style_tree(&visible_dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let visible = non_anonymous_node_by_source_id(input.tree().root(), Id(10))
            .expect("visible section paint node");

        assert!(
            visible
                .primitives()
                .iter()
                .all(|primitive| primitive.kind() != PaintPrimitiveKind::Clip),
            "paint must not invent a clip when layout exposes none"
        );

        let inline_dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(20),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: vec![Node::Element {
                    id: Id(21),
                    name: Arc::from("span"),
                    attributes: Vec::new(),
                    style: vec![("overflow".to_string(), "hidden".to_string())],
                    children: vec![Node::Text {
                        id: Id(22),
                        text: "inline".to_string(),
                    }],
                }],
            }],
        };
        let styled = build_style_tree(&inline_dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let span = non_anonymous_node_by_source_id(input.tree().root(), Id(21))
            .expect("inline span paint node");

        assert!(
            span.primitives()
                .iter()
                .all(|primitive| primitive.kind() != PaintPrimitiveKind::Clip),
            "paint must follow layout and not clip ordinary inline overflow boxes"
        );
    }

    #[test]
    fn paint_tree_emits_text_decoration_after_decorated_text_fragments() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "200px".to_string()),
                    ("font-size".to_string(), "20px".to_string()),
                    ("color".to_string(), "red".to_string()),
                    ("text-decoration-line".to_string(), "underline".to_string()),
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
        let node = first_node_with_primitive(input.tree().root(), PaintPrimitiveKind::Text)
            .expect("section text paint node");

        assert_eq!(
            primitive_kinds(node),
            vec![PaintPrimitiveKind::Text, PaintPrimitiveKind::TextDecoration,]
        );
        assert!(matches!(
            &node.primitives()[0],
            PaintPrimitive::Text(PaintText { text, .. }) if text == "Hello"
        ));

        let decoration = node
            .primitives()
            .iter()
            .find_map(|primitive| match primitive {
                PaintPrimitive::TextDecoration(decoration) => Some(*decoration),
                _ => None,
            })
            .expect("text decoration primitive");

        assert_eq!(decoration.line, PaintTextDecorationLine::Underline);
        assert_eq!(decoration.color, PaintColor::from_rgba((255, 0, 0, 255)));
        assert_eq!(decoration.thickness, 1.25);
        assert_eq!(
            decoration.rect,
            Rectangle {
                x: 4.0,
                y: 24.1,
                width: 40.0,
                height: 1.25,
            }
        );
    }

    #[test]
    fn paint_tree_omits_text_decoration_for_none_and_atomic_inline_fragments() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "200px".to_string()),
                ],
                children: vec![
                    Node::Text {
                        id: Id(3),
                        text: "Hello".to_string(),
                    },
                    Node::Element {
                        id: Id(4),
                        name: Arc::from("span"),
                        attributes: Vec::new(),
                        style: vec![("display".to_string(), "inline-block".to_string())],
                        children: vec![Node::Text {
                            id: Id(5),
                            text: "Box".to_string(),
                        }],
                    },
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);

        assert!(
            first_node_with_primitive(input.tree().root(), PaintPrimitiveKind::TextDecoration)
                .is_none()
        );
    }

    #[test]
    fn paint_tree_propagates_inline_container_underline_to_child_text() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![
                    ("display".to_string(), "block".to_string()),
                    ("width".to_string(), "200px".to_string()),
                    ("text-decoration-line".to_string(), "underline".to_string()),
                ],
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("span"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: vec![Node::Text {
                        id: Id(4),
                        text: "Child".to_string(),
                    }],
                }],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);

        assert!(
            first_node_with_primitive(input.tree().root(), PaintPrimitiveKind::TextDecoration)
                .is_some()
        );
        let span = styled
            .children
            .first()
            .and_then(|section| section.children.first())
            .expect("span styled node");
        assert_eq!(
            span.style.text_decoration_line(),
            TextDecorationLine::None,
            "underline propagation must not rely on inherited computed style"
        );
    }

    #[test]
    fn paint_tree_emits_outline_as_distinct_post_child_primitive() {
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
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "red".to_string()),
                ],
                children: Vec::new(),
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let node = non_anonymous_node_by_source_id(input.tree().root(), Id(2))
            .expect("section outline paint node");

        assert!(
            node.primitives()
                .iter()
                .all(|primitive| primitive.kind() != PaintPrimitiveKind::Border)
        );
        assert!(node.post_primitives().iter().any(|primitive| matches!(
            primitive,
            PaintPrimitive::Outline(PaintOutline { width: 3.0, color, .. })
                if *color == PaintColor::from_rgba((255, 0, 0, 255))
        )));
    }

    #[test]
    fn paint_tree_orders_outline_after_contents_and_child_subtrees() {
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
                    ("background-color".to_string(), "white".to_string()),
                    ("border-top-width".to_string(), "2px".to_string()),
                    ("border-top-style".to_string(), "solid".to_string()),
                    ("border-top-color".to_string(), "blue".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "red".to_string()),
                ],
                children: vec![
                    Node::Text {
                        id: Id(3),
                        text: "Hello".to_string(),
                    },
                    Node::Element {
                        id: Id(4),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![
                            ("display".to_string(), "block".to_string()),
                            ("width".to_string(), "20px".to_string()),
                            ("height".to_string(), "10px".to_string()),
                            ("background-color".to_string(), "green".to_string()),
                        ],
                        children: Vec::new(),
                    },
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let node = non_anonymous_node_by_source_id(input.tree().root(), Id(2))
            .expect("section outline paint node");

        assert!(matches!(
            node.primitives(),
            [PaintPrimitive::Background(_), PaintPrimitive::Border(_), ..]
        ));
        assert!(node.post_primitives().iter().any(|primitive| matches!(
            primitive,
            PaintPrimitive::Outline(PaintOutline { source, .. }) if source.node_id == Id(2)
        )));

        let order = flattened_paint_order(node);
        assert_eq!(order.last(), Some(&PaintPrimitiveKind::Outline));
        let child_background = order
            .iter()
            .position(|kind| *kind == PaintPrimitiveKind::Background)
            .expect("section background");
        let final_outline = order
            .iter()
            .rposition(|kind| *kind == PaintPrimitiveKind::Outline)
            .expect("section outline");
        assert!(child_background < final_outline);
    }

    #[test]
    fn paint_tree_outline_geometry_expands_outside_border_box() {
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
                    ("height".to_string(), "20px".to_string()),
                    ("outline-width".to_string(), "4px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "red".to_string()),
                ],
                children: Vec::new(),
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let node = non_anonymous_node_by_source_id(input.tree().root(), Id(2))
            .expect("section outline paint node");

        let outline = node
            .post_primitives()
            .iter()
            .find_map(|primitive| match primitive {
                PaintPrimitive::Outline(outline) => Some(*outline),
                _ => None,
            })
            .expect("outline primitive");

        assert_eq!(
            outline.border_rect,
            Rectangle {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 20.0,
            }
        );
        assert_eq!(
            outline.outer_rect,
            Rectangle {
                x: -4.0,
                y: -4.0,
                width: 108.0,
                height: 28.0,
            }
        );
    }

    #[test]
    fn paint_tree_skips_invisible_border_primitives_deterministically() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![
                Node::Element {
                    id: Id(2),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("border-top-width".to_string(), "2px".to_string()),
                        ("border-top-style".to_string(), "solid".to_string()),
                    ],
                    children: Vec::new(),
                },
                Node::Element {
                    id: Id(3),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("border-top-width".to_string(), "2px".to_string()),
                        ("border-top-color".to_string(), "red".to_string()),
                    ],
                    children: Vec::new(),
                },
                Node::Element {
                    id: Id(4),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("border-top-style".to_string(), "solid".to_string()),
                        ("border-top-color".to_string(), "red".to_string()),
                    ],
                    children: Vec::new(),
                },
                Node::Element {
                    id: Id(5),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("border-top-width".to_string(), "2px".to_string()),
                        ("border-top-style".to_string(), "solid".to_string()),
                        ("border-top-color".to_string(), "transparent".to_string()),
                    ],
                    children: Vec::new(),
                },
            ],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);

        assert!(
            first_node_with_primitive(input.tree().root(), PaintPrimitiveKind::Border).is_none()
        );
    }

    #[test]
    fn paint_tree_skips_invisible_outline_primitives_deterministically() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![
                Node::Element {
                    id: Id(2),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("outline-style".to_string(), "solid".to_string()),
                        ("outline-color".to_string(), "red".to_string()),
                    ],
                    children: Vec::new(),
                },
                Node::Element {
                    id: Id(3),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("outline-width".to_string(), "2px".to_string()),
                        ("outline-color".to_string(), "red".to_string()),
                    ],
                    children: Vec::new(),
                },
                Node::Element {
                    id: Id(4),
                    name: Arc::from("section"),
                    attributes: Vec::new(),
                    style: vec![
                        ("outline-width".to_string(), "2px".to_string()),
                        ("outline-style".to_string(), "solid".to_string()),
                        ("outline-color".to_string(), "transparent".to_string()),
                    ],
                    children: Vec::new(),
                },
            ],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);

        assert!(
            first_node_with_primitive(input.tree().root(), PaintPrimitiveKind::Outline).is_none()
        );
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
