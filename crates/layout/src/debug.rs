use std::fmt::Write;

use css::StyledNode;
use html::Node;

use crate::{
    AnonymousBoxKind, BlockFormattingParticipation, BoxId, BoxKind, BoxSource, ContainingBlockId,
    FormattingContextId, FormattingContextKind, InlineFormattingContextId,
    InlineFormattingParticipation, LayoutBox, LayoutPhaseInput, LayoutPhaseOutput, ListMarker,
    Rectangle, ReplacedKind, replaced::intrinsic::IntrinsicSize,
    sizing::used_content_size_debug_label,
};

impl<'style_tree, 'dom, 'runtime> LayoutPhaseInput<'style_tree, 'dom, 'runtime> {
    /// Stable debug snapshot for the style-to-layout phase boundary.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "layout-phase-input").expect("write snapshot");
        writeln!(&mut out, "available-width: {:.2}", self.available_width())
            .expect("write snapshot");
        writeln!(&mut out, "style-root-id: {}", self.style_root().node_id.0)
            .expect("write snapshot");
        writeln!(
            &mut out,
            "style-root: {}",
            node_debug_label(self.style_root().node)
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "style-nodes: {}",
            count_styled_nodes(self.style_root())
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "has-replaced-info: {}",
            self.replaced_info().is_some()
        )
        .expect("write snapshot");
        out
    }
}

impl<'style_tree, 'dom> LayoutPhaseOutput<'style_tree, 'dom> {
    /// Stable debug snapshot for the layout-to-paint phase boundary.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "layout-phase-output").expect("write snapshot");
        writeln!(&mut out, "viewport-width: {:.2}", self.viewport_width()).expect("write snapshot");
        writeln!(
            &mut out,
            "document-rect: {}",
            rectangle_debug_label(self.document_rect())
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "layout-boxes: {}",
            count_layout_boxes(self.root())
        )
        .expect("write snapshot");
        append_layout_box_snapshot(&mut out, self.root(), 0, 0);
        out
    }

    /// Stable sizing-focused debug snapshot for normal-flow layout output.
    ///
    /// This surface complements `to_debug_snapshot()` by exposing content-box
    /// geometry, box metrics, flow participation, and the used-size metadata
    /// recorded by the normal-flow sizing pass.
    pub fn to_sizing_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "layout-sizing-flow").expect("write snapshot");
        writeln!(&mut out, "viewport-width: {:.2}", self.viewport_width()).expect("write snapshot");
        writeln!(
            &mut out,
            "document-rect: {}",
            rectangle_debug_label(self.document_rect())
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "layout-boxes: {}",
            count_layout_boxes(self.root())
        )
        .expect("write snapshot");
        append_layout_sizing_snapshot(&mut out, self.root(), 0, 0);
        out
    }
}

fn count_styled_nodes(node: &StyledNode<'_>) -> usize {
    1 + node
        .children
        .iter()
        .map(|child| count_styled_nodes(child))
        .sum::<usize>()
}

fn count_layout_boxes(layout: &LayoutBox<'_, '_>) -> usize {
    1 + layout
        .children
        .iter()
        .map(|child| count_layout_boxes(child))
        .sum::<usize>()
}

fn append_layout_box_snapshot(
    out: &mut String,
    layout: &LayoutBox<'_, '_>,
    index: usize,
    depth: usize,
) -> usize {
    let indent = "  ".repeat(depth);
    writeln!(
        out,
        "{indent}box[{index}]: box-id={} anchor-id={} source={} node={} kind={} cb={} establishes-cb={} fc={} establishes-fc={} block-participation={} ifc={} establishes-ifc={} inline-participation={} rect={} children={} marker={} replaced={} intrinsic={} style={}",
        box_id_debug_label(layout.box_id()),
        layout.node_id().0,
        layout_box_source_debug_label(layout.source),
        node_debug_label(layout.node.node),
        box_kind_debug_label(layout.kind),
        optional_containing_block_id_debug_label(layout.containing_block()),
        bool_debug_label(layout.establishes_containing_block()),
        optional_formatting_context_id_debug_label(layout.formatting_context()),
        optional_formatting_context_kind_debug_label(layout.establishes_formatting_context()),
        block_formatting_participation_debug_label(layout.block_formatting_participation()),
        optional_inline_formatting_context_id_debug_label(layout.inline_formatting_context()),
        bool_debug_label(layout.establishes_inline_formatting_context()),
        inline_formatting_participation_debug_label(layout.inline_formatting_participation()),
        rectangle_debug_label(layout.rect),
        layout.children.len(),
        list_marker_debug_label(layout.list_marker),
        replaced_kind_debug_label(layout.replaced),
        intrinsic_size_debug_label(layout.replaced_intrinsic),
        layout.style.to_boundary_debug_label(),
    )
    .expect("write snapshot");

    let mut next_index = index + 1;
    for child in &layout.children {
        next_index = append_layout_box_snapshot(out, child, next_index, depth + 1);
    }
    next_index
}

fn append_layout_sizing_snapshot(
    out: &mut String,
    layout: &LayoutBox<'_, '_>,
    index: usize,
    depth: usize,
) -> usize {
    let indent = "  ".repeat(depth);
    let metrics = layout.box_metrics();
    let content_rect = Rectangle {
        x: layout.content_x_and_width().0,
        y: layout.content_y(),
        width: layout.content_x_and_width().1,
        height: layout.content_height(),
    };

    writeln!(
        out,
        "{indent}box[{index}]: box-id={} source={} node={} kind={} cb={} block-participation={} inline-participation={} border-box={} content-box={} margin={} padding={} used-size={} children={}",
        box_id_debug_label(layout.box_id()),
        layout_box_source_debug_label(layout.source),
        node_debug_label(layout.node.node),
        box_kind_debug_label(layout.kind),
        optional_containing_block_id_debug_label(layout.containing_block()),
        block_formatting_participation_debug_label(layout.block_formatting_participation()),
        inline_formatting_participation_debug_label(layout.inline_formatting_participation()),
        rectangle_debug_label(layout.rect),
        rectangle_debug_label(content_rect),
        box_metrics_margin_debug_label(metrics),
        box_metrics_padding_debug_label(metrics),
        used_content_size_debug_label(layout.used_content_size),
        layout.children.len(),
    )
    .expect("write snapshot");

    let mut next_index = index + 1;
    for child in &layout.children {
        next_index = append_layout_sizing_snapshot(out, child, next_index, depth + 1);
    }
    next_index
}

fn box_id_debug_label(id: BoxId) -> String {
    format!("b{}", id.index())
}

fn optional_containing_block_id_debug_label(id: Option<ContainingBlockId>) -> String {
    id.map(|id| box_id_debug_label(id.box_id()))
        .unwrap_or_else(|| "none".to_string())
}

fn optional_formatting_context_id_debug_label(id: Option<FormattingContextId>) -> String {
    id.map(|id| box_id_debug_label(id.box_id()))
        .unwrap_or_else(|| "none".to_string())
}

fn optional_formatting_context_kind_debug_label(
    kind: Option<FormattingContextKind>,
) -> &'static str {
    match kind {
        Some(FormattingContextKind::Block) => "block",
        None => "none",
    }
}

fn block_formatting_participation_debug_label(
    participation: BlockFormattingParticipation,
) -> &'static str {
    match participation {
        BlockFormattingParticipation::Root => "root",
        BlockFormattingParticipation::BlockLevel => "block-level",
        BlockFormattingParticipation::InlineLevel => "inline-level",
        BlockFormattingParticipation::AtomicInline => "atomic-inline",
        BlockFormattingParticipation::None => "none",
    }
}

fn optional_inline_formatting_context_id_debug_label(
    id: Option<InlineFormattingContextId>,
) -> String {
    id.map(|id| box_id_debug_label(id.box_id()))
        .unwrap_or_else(|| "none".to_string())
}

fn inline_formatting_participation_debug_label(
    participation: InlineFormattingParticipation,
) -> &'static str {
    match participation {
        InlineFormattingParticipation::None => "none",
        InlineFormattingParticipation::InlineContainer => "inline-container",
        InlineFormattingParticipation::TextRun => "text-run",
        InlineFormattingParticipation::AtomicInline => "atomic-inline",
    }
}

fn bool_debug_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn layout_box_source_debug_label(source: BoxSource<'_, '_>) -> String {
    match source {
        BoxSource::DomNode(node) => format!("dom({})", node.node_id.0),
        BoxSource::Anonymous { parent, kind } => {
            format!(
                "{}(anchor={})",
                anonymous_box_kind_debug_label(kind),
                parent.node_id.0
            )
        }
        BoxSource::Marker { list_item } => format!("marker(anchor={})", list_item.node_id.0),
    }
}

fn anonymous_box_kind_debug_label(kind: AnonymousBoxKind) -> &'static str {
    match kind {
        AnonymousBoxKind::Block => "anonymous-block",
        AnonymousBoxKind::Inline => "anonymous-inline",
    }
}

pub(crate) fn node_debug_label(node: &Node) -> String {
    match node {
        Node::Document { .. } => "document".to_string(),
        Node::Element { name, .. } => format!("element(\"{name}\")"),
        Node::Text { text, .. } => format!("text(\"{}\")", text.escape_default()),
        Node::Comment { text, .. } => format!("comment(\"{}\")", text.escape_default()),
    }
}

pub(crate) fn box_kind_debug_label(kind: BoxKind) -> &'static str {
    match kind {
        BoxKind::Block => "block",
        BoxKind::Inline => "inline",
        BoxKind::InlineBlock => "inline-block",
        BoxKind::ReplacedInline => "replaced-inline",
    }
}

pub(crate) fn list_marker_debug_label(marker: Option<ListMarker>) -> String {
    match marker {
        None => "none".to_string(),
        Some(ListMarker::Unordered) => "unordered".to_string(),
        Some(ListMarker::Ordered(value)) => format!("ordered({value})"),
    }
}

pub(crate) fn replaced_kind_debug_label(replaced: Option<ReplacedKind>) -> String {
    match replaced {
        None => "none".to_string(),
        Some(ReplacedKind::Img) => "img".to_string(),
        Some(ReplacedKind::InputText) => "input-text".to_string(),
        Some(ReplacedKind::TextArea) => "textarea".to_string(),
        Some(ReplacedKind::InputCheckbox) => "input-checkbox".to_string(),
        Some(ReplacedKind::InputRadio) => "input-radio".to_string(),
        Some(ReplacedKind::Button) => "button".to_string(),
    }
}

pub(crate) fn intrinsic_size_debug_label(size: Option<IntrinsicSize>) -> String {
    match size {
        None => "none".to_string(),
        Some(size) => format!(
            "w={} h={} ratio={}",
            optional_px_debug_label(size.width),
            optional_px_debug_label(size.height),
            optional_ratio_debug_label(size.ratio),
        ),
    }
}

fn optional_px_debug_label(value: Option<f32>) -> String {
    match value {
        Some(value) => format!("{value:.2}px"),
        None => "none".to_string(),
    }
}

fn optional_ratio_debug_label(value: Option<f32>) -> String {
    match value {
        Some(value) => format!("{value:.4}"),
        None => "none".to_string(),
    }
}

fn rectangle_debug_label(rect: Rectangle) -> String {
    format!(
        "x={:.2} y={:.2} w={:.2} h={:.2}",
        rect.x, rect.y, rect.width, rect.height
    )
}

fn box_metrics_margin_debug_label(metrics: css::BoxMetrics) -> String {
    format!(
        "(top={:.2}px right={:.2}px bottom={:.2}px left={:.2}px)",
        metrics.margin_top, metrics.margin_right, metrics.margin_bottom, metrics.margin_left
    )
}

fn box_metrics_padding_debug_label(metrics: css::BoxMetrics) -> String {
    format!(
        "(top={:.2}px right={:.2}px bottom={:.2}px left={:.2}px)",
        metrics.padding_top, metrics.padding_right, metrics.padding_bottom, metrics.padding_left
    )
}
