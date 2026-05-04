//! Layout-phase box-tree and geometry primitives.
//!
//! The architecture contract for Borrowser's box tree, formatting-context
//! model, and layout responsibility boundaries is documented in
//! `docs/rendering/w1-box-tree-layout-model-contract.md`; the W2 data model is
//! documented in `docs/rendering/w2-structured-box-tree-data-structures.md`;
//! anonymous generation is documented in
//! `docs/rendering/w4-anonymous-box-generation-supported-subset.md`;
//! containing-block relationships are documented in
//! `docs/rendering/w5-containing-block-relationships.md`; block formatting
//! context foundations are documented in
//! `docs/rendering/w6-block-formatting-context-foundations.md`.
//! `BoxTree` is the frame-local generated box-tree structure; `LayoutBox` is
//! the current geometry projection consumed by paint and hit testing.

mod box_tree;
mod text;
pub use box_tree::{
    AnonymousBoxKind, BlockFormattingParticipation, BoxGenerationRole, BoxId, BoxNode, BoxSource,
    BoxSuppressionReason, BoxTree, ContainingBlockId, DisplayBoxBehavior, DisplayBoxGeneration,
    FormattingContextId, FormattingContextKind, PrincipalBox,
};
pub use text::TextMeasurer;

pub mod inline;
pub use inline::{LineBox, layout_inline_for_paint};
pub mod hit_test;
pub use hit_test::{HitKind, hit_test};
pub mod replaced;

use css::{BoxMetrics, ComputedStyle, StylePhaseOutput, StyledNode};
use html::{Node, internal::Id};
use replaced::intrinsic::IntrinsicSize;
use std::fmt::Write;

/// A rectangle in CSS px units (we'll treat everything as px for now).
#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    pub fn approx_eq(self, other: Self, eps: f32) -> bool {
        (self.x - other.x).abs() <= eps
            && (self.y - other.y).abs() <= eps
            && (self.width - other.width).abs() <= eps
            && (self.height - other.height).abs() <= eps
    }
}

impl PartialEq for Rectangle {
    fn eq(&self, other: &Self) -> bool {
        const EPS: f32 = 1e-6;
        (*self).approx_eq(*other, EPS)
    }
}

/// Current supported layout participation category for a generated layout box.
///
/// This is not yet the full CSS box-generation model. Milestone W will expand
/// this toward explicit root, anonymous, marker, and formatting-context-aware
/// box roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    Block,
    Inline,
    InlineBlock,
    ReplacedInline,
    // Future: Root, AnonymousBlock, AnonymousInline, Marker, ListItem, etc.
}

/// What kind of list marker this block has, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMarker {
    /// Bullet for unordered lists (<ul><li>).
    Unordered,
    /// Numbered marker for ordered lists (<ol><li>), 1-based.
    Ordered(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacedKind {
    Img,
    InputText,
    TextArea,
    InputCheckbox,
    InputRadio,
    Button,
}

/// Optional, host-provided info for replaced elements (e.g. decoded image sizes).
pub trait ReplacedElementInfoProvider {
    fn intrinsic_for_img(&self, node: &html::Node) -> Option<IntrinsicSize>;
}

/// Classify replaced elements for layout purposes.
fn classify_replaced_kind(node: &Node) -> Option<ReplacedKind> {
    match node {
        Node::Element {
            name, attributes, ..
        } => {
            if name.eq_ignore_ascii_case("img") {
                return Some(ReplacedKind::Img);
            }

            if name.eq_ignore_ascii_case("input") {
                // Phase 1: basic <input> replaced controls.
                let mut ty: Option<&str> = None;
                for (k, v) in attributes {
                    if k.eq_ignore_ascii_case("type") {
                        ty = v.as_deref().map(str::trim).filter(|s| !s.is_empty());
                        break;
                    }
                }

                match ty {
                    None => return Some(ReplacedKind::InputText),
                    Some(t) if t.eq_ignore_ascii_case("text") => {
                        return Some(ReplacedKind::InputText);
                    }
                    Some(t) if t.eq_ignore_ascii_case("checkbox") => {
                        return Some(ReplacedKind::InputCheckbox);
                    }
                    Some(t) if t.eq_ignore_ascii_case("radio") => {
                        return Some(ReplacedKind::InputRadio);
                    }
                    _ => {}
                }
            }

            if name.eq_ignore_ascii_case("textarea") {
                return Some(ReplacedKind::TextArea);
            }

            if name.eq_ignore_ascii_case("button") {
                return Some(ReplacedKind::Button);
            }

            None
        }
        _ => None,
    }
}

/// A geometry projection of one generated box-tree node.
///
/// `source` preserves whether the box is directly DOM-backed or generated by a
/// layout rule. `node` remains a source anchor during the current bridge phase
/// so downstream paint/input code can keep using existing DOM-oriented helpers
/// while anonymous boxes become explicit layout participants.
pub struct LayoutBox<'style_tree, 'dom> {
    pub box_id: BoxId,
    pub kind: BoxKind,
    pub style: &'style_tree ComputedStyle,
    pub source: BoxSource<'style_tree, 'dom>,
    pub node: &'style_tree StyledNode<'dom>,
    pub rect: Rectangle,
    pub children: Vec<LayoutBox<'style_tree, 'dom>>,
    pub containing_block: Option<ContainingBlockId>,
    pub establishes_containing_block: bool,
    pub formatting_context: Option<FormattingContextId>,
    pub establishes_formatting_context: Option<FormattingContextKind>,
    pub block_formatting_participation: BlockFormattingParticipation,
    pub list_marker: Option<ListMarker>,
    pub replaced: Option<ReplacedKind>,
    pub replaced_intrinsic: Option<IntrinsicSize>,
}

impl<'style_tree, 'dom> LayoutBox<'style_tree, 'dom> {
    pub fn box_id(&self) -> BoxId {
        self.box_id
    }

    /// Returns the anchor DOM node ID for this layout box.
    ///
    /// For DOM-backed boxes this is the direct source node. For anonymous or
    /// other generated boxes this is the source anchor used for inherited style
    /// and bridge compatibility. Use `direct_node_id()` when code needs to know
    /// whether the box directly represents a DOM node.
    pub fn node_id(&self) -> Id {
        self.source.anchor_node_id()
    }

    pub fn direct_node_id(&self) -> Option<Id> {
        self.source.direct_node_id()
    }

    pub fn containing_block(&self) -> Option<ContainingBlockId> {
        self.containing_block
    }

    pub fn establishes_containing_block(&self) -> bool {
        self.establishes_containing_block
    }

    pub fn formatting_context(&self) -> Option<FormattingContextId> {
        self.formatting_context
    }

    pub fn establishes_formatting_context(&self) -> Option<FormattingContextKind> {
        self.establishes_formatting_context
    }

    pub fn block_formatting_participation(&self) -> BlockFormattingParticipation {
        self.block_formatting_participation
    }

    pub fn is_anonymous(&self) -> bool {
        matches!(self.source, BoxSource::Anonymous { .. })
    }

    pub fn source_node(&self) -> Option<&'style_tree StyledNode<'dom>> {
        self.source.direct_styled_node()
    }

    pub fn box_metrics(&self) -> BoxMetrics {
        if self.is_anonymous() {
            BoxMetrics::zero()
        } else {
            self.style.box_metrics()
        }
    }

    pub fn content_x_and_width(&self) -> (f32, f32) {
        let bm = self.box_metrics();
        let content_x = self.rect.x + bm.padding_left;
        let content_width = (self.rect.width - bm.padding_left - bm.padding_right).max(0.0);
        (content_x, content_width)
    }

    pub fn content_y(&self) -> f32 {
        self.rect.y + self.box_metrics().padding_top
    }

    pub fn content_height(&self) -> f32 {
        let bm = self.box_metrics();
        (self.rect.height - bm.padding_top - bm.padding_bottom).max(0.0)
    }
}

/// Structured layout-phase input consumed by the layout engine.
///
/// `'style_tree` is the borrow of the rebuilt style-phase output for this
/// pipeline execution. `'dom` is the lifetime of DOM references stored inside
/// `StyledNode`. Keeping them distinct avoids over-constraining layout to treat
/// a frame-scoped style-tree borrow as if it were the DOM lifetime itself.
pub struct LayoutPhaseInput<'style_tree, 'dom, 'runtime> {
    style_root: &'style_tree StyledNode<'dom>,
    available_width: f32,
    measurer: &'runtime dyn TextMeasurer,
    replaced_info: Option<&'runtime dyn ReplacedElementInfoProvider>,
}

impl<'style_tree, 'dom, 'runtime> LayoutPhaseInput<'style_tree, 'dom, 'runtime> {
    pub fn new(
        style_root: &'style_tree StyledNode<'dom>,
        available_width: f32,
        measurer: &'runtime dyn TextMeasurer,
        replaced_info: Option<&'runtime dyn ReplacedElementInfoProvider>,
    ) -> Self {
        Self {
            style_root,
            available_width,
            measurer,
            replaced_info,
        }
    }

    pub fn from_style_output(
        style_output: &'style_tree StylePhaseOutput<'dom>,
        available_width: f32,
        measurer: &'runtime dyn TextMeasurer,
        replaced_info: Option<&'runtime dyn ReplacedElementInfoProvider>,
    ) -> Self {
        Self::new(
            style_output.root(),
            available_width,
            measurer,
            replaced_info,
        )
    }

    pub fn style_root(&self) -> &'style_tree StyledNode<'dom> {
        self.style_root
    }

    pub fn available_width(&self) -> f32 {
        self.available_width
    }

    pub fn measurer(&self) -> &'runtime dyn TextMeasurer {
        self.measurer
    }

    pub fn replaced_info(&self) -> Option<&'runtime dyn ReplacedElementInfoProvider> {
        self.replaced_info
    }

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

/// Structured layout-phase output handed to downstream paint and input phases.
///
/// `available_width` is stored explicitly as part of the layout environment for
/// this pass. It must not be inferred from `root.rect.width`, because future
/// layout features may allow those values to diverge.
pub struct LayoutPhaseOutput<'style_tree, 'dom> {
    root: LayoutBox<'style_tree, 'dom>,
    available_width: f32,
}

impl<'style_tree, 'dom> LayoutPhaseOutput<'style_tree, 'dom> {
    pub fn new(root: LayoutBox<'style_tree, 'dom>, available_width: f32) -> Self {
        Self {
            root,
            available_width,
        }
    }

    pub fn root(&self) -> &LayoutBox<'style_tree, 'dom> {
        &self.root
    }

    pub fn into_root(self) -> LayoutBox<'style_tree, 'dom> {
        self.root
    }

    pub fn document_rect(&self) -> Rectangle {
        self.root.rect
    }

    pub fn viewport_width(&self) -> f32 {
        self.available_width
    }

    pub fn content_height(&self) -> f32 {
        self.root.rect.height
    }

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
}

/// The inner "content box" of a layout box: border box minus padding.
/// We expose it via small helpers so that all code computes content
/// geometry in a single, consistent way.
pub fn content_x_and_width(style: &ComputedStyle, border_x: f32, border_width: f32) -> (f32, f32) {
    let bm = style.box_metrics();

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
    let bm = style.box_metrics();
    border_y + bm.padding_top
}

/// Height of the content box (border box height minus vertical padding).
pub fn content_height(style: &ComputedStyle, border_height: f32) -> f32 {
    let bm = style.box_metrics();
    let content_height = (border_height - bm.padding_top - bm.padding_bottom).max(0.0);

    debug_assert!(
        content_height >= 0.0,
        "content_height produced negative height: border_height={border_height}, paddings=({}, {})",
        bm.padding_top,
        bm.padding_bottom,
    );

    content_height
}

/// Compute block layout for a style tree.
/// - `root` is the style-tree root (usually the document node)
/// - `page_width` is the available content width in px
/// - `measurer` is used to measure text during inline layout
pub fn layout_block_tree<'style_tree, 'dom>(
    root: &'style_tree StyledNode<'dom>,
    page_width: f32,
    measurer: &dyn TextMeasurer,
    replaced_info: Option<&dyn ReplacedElementInfoProvider>,
) -> LayoutBox<'style_tree, 'dom> {
    layout_document(LayoutPhaseInput::new(
        root,
        page_width,
        measurer,
        replaced_info,
    ))
    .into_root()
}

/// Run the layout phase using an explicit structured handoff model.
pub fn layout_document<'style_tree, 'dom>(
    input: LayoutPhaseInput<'style_tree, 'dom, '_>,
) -> LayoutPhaseOutput<'style_tree, 'dom> {
    // 1) Build the layout tree structure (no real geometry yet).
    let box_tree = BoxTree::generate(input.style_root(), input.replaced_info());
    let mut root_box = layout_box_from_generated_tree(
        &box_tree,
        box_tree.root_id(),
        0.0,
        0.0,
        input.available_width(),
    );

    // 2) Single authoritative geometry pass: inline + block layout.
    //
    //    This computes x/y/width/height for *all* LayoutBoxes,
    //    using the same inline token / LineBox pipeline that painting uses.
    crate::inline::refine_layout_with_inline(input.measurer(), &mut root_box);

    LayoutPhaseOutput::new(root_box, input.available_width())
}

/// Internal recursive function:
/// - `x`, `y` = top-left of this box
/// - `width`  = available width
///
fn layout_box_from_generated_tree<'style_tree, 'dom>(
    box_tree: &BoxTree<'style_tree, 'dom>,
    box_id: BoxId,
    x: f32,
    y: f32,
    width: f32,
) -> LayoutBox<'style_tree, 'dom> {
    let box_node = box_tree.node(box_id);
    let source = box_node.source();
    let styled = source.anchor_styled_node();
    let children_boxes = box_node
        .children()
        .iter()
        .map(|child| layout_box_from_generated_tree(box_tree, *child, x, y, width))
        .collect();

    // Border-box rect: x/y/width are authoritative here.
    //    Height is always 0.0 in this phase; it will be computed by
    //    the inline-aware layout pass (recompute_block_heights).
    let rect = Rectangle {
        x,
        y,
        width,
        height: 0.0,
    };

    LayoutBox {
        box_id: box_node.id(),
        kind: box_node.kind(),
        style: box_node.style(),
        source,
        node: styled,
        rect,
        children: children_boxes,
        containing_block: box_node.containing_block(),
        establishes_containing_block: box_node.establishes_containing_block(),
        formatting_context: box_node.formatting_context(),
        establishes_formatting_context: box_node.establishes_formatting_context(),
        block_formatting_participation: box_node.block_formatting_participation(),
        list_marker: box_node.list_marker(),
        replaced: box_node.replaced(),
        replaced_intrinsic: box_node.replaced_intrinsic(),
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
        "{indent}box[{index}]: box-id={} anchor-id={} source={} node={} kind={} cb={} establishes-cb={} fc={} establishes-fc={} block-participation={} rect={} children={} marker={} replaced={} intrinsic={} style={}",
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
