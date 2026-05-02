mod text;
pub use text::TextMeasurer;

pub mod inline;
pub use inline::{LineBox, layout_inline_for_paint};
pub mod hit_test;
pub use hit_test::{HitKind, hit_test};
pub mod replaced;

use css::{ComputedStyle, Display, StylePhaseOutput, StyledNode};
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

/// What kind of layout box this is. For now: only block.
#[derive(Clone, Copy, Debug)]
pub enum BoxKind {
    Block,
    Inline,
    InlineBlock,
    ReplacedInline,
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

/// A node in the layout tree:
/// - borrows one styled node from the frame-scoped style-phase output
/// - preserves the DOM identity carried by that styled node
/// - has a geometry rect
/// - has child layout boxes
pub struct LayoutBox<'style_tree, 'dom> {
    pub kind: BoxKind,
    pub style: &'style_tree ComputedStyle,
    pub node: &'style_tree StyledNode<'dom>,
    pub rect: Rectangle,
    pub children: Vec<LayoutBox<'style_tree, 'dom>>,
    pub list_marker: Option<ListMarker>,
    pub replaced: Option<ReplacedKind>,
    pub replaced_intrinsic: Option<IntrinsicSize>,
}

impl<'style_tree, 'dom> LayoutBox<'style_tree, 'dom> {
    pub fn node_id(&self) -> Id {
        self.node.node_id
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
    let mut root_box = layout_block_subtree(
        input.style_root(),
        0.0,
        0.0,
        input.available_width(),
        input.replaced_info(),
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
/// Builds a LayoutBox subtree with correct x/y/width, but height = 0.0.
/// The unified inline-aware pass will compute final heights.
fn layout_block_subtree<'style_tree, 'dom>(
    styled: &'style_tree StyledNode<'dom>,
    x: f32,
    y: f32,
    width: f32,
    replaced_info: Option<&dyn ReplacedElementInfoProvider>,
) -> LayoutBox<'style_tree, 'dom> {
    layout_box_for_styled_subtree(styled, x, y, width, replaced_info).unwrap_or_else(|| {
        // Keep the layout phase total even if a malformed input computes the
        // document root itself to display:none.
        LayoutBox {
            kind: BoxKind::Block,
            style: &styled.style,
            node: styled,
            rect: Rectangle {
                x,
                y,
                width,
                height: 0.0,
            },
            children: Vec::new(),
            list_marker: None,
            replaced: None,
            replaced_intrinsic: None,
        }
    })
}

fn layout_box_for_styled_subtree<'style_tree, 'dom>(
    styled: &'style_tree StyledNode<'dom>,
    x: f32,
    y: f32,
    width: f32,
    replaced_info: Option<&dyn ReplacedElementInfoProvider>,
) -> Option<LayoutBox<'style_tree, 'dom>> {
    if matches!(styled.node, Node::Element { .. }) && styled.style.display() == Display::None {
        return None;
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
            let Some(mut child_box) =
                layout_box_for_styled_subtree(child, x, y, width, replaced_info)
            else {
                continue;
            };

            // If this is a list container (<ul>/<ol>) and the child is a list-item,
            // assign a marker.
            if matches!(child.node, Node::Element { .. })
                && child_box.style.display() == Display::ListItem
            {
                if is_ul {
                    child_box.list_marker = Some(ListMarker::Unordered);
                } else if is_ol {
                    child_box.list_marker = Some(ListMarker::Ordered(next_ol_index));
                    next_ol_index += 1;
                }
            }

            children_boxes.push(child_box);
        }
    }

    // 2) Decide box kind based on node type + computed display.
    let style = &styled.style;
    let replaced_kind = classify_replaced_kind(styled.node);

    let kind = match styled.node {
        Node::Document { .. } => BoxKind::Block,
        // Transitional root-element handling.
        //
        // This is not a UA display-default shortcut. Ordinary element display
        // behavior must come from computed style. Until Milestone W introduces
        // an explicit box-tree/root-box model, the document element is forced
        // to produce the top-level layout container here.
        Node::Element { name, .. } if name.eq_ignore_ascii_case("html") => BoxKind::Block,

        Node::Text { .. } | Node::Comment { .. } => BoxKind::Block,

        _ => {
            // If it's a replaced element and it's inline-level, treat it as a replaced inline atom.
            if replaced_kind.is_some()
                && matches!(style.display(), Display::Inline | Display::InlineBlock)
            {
                BoxKind::ReplacedInline
            } else {
                match style.display() {
                    Display::Inline => BoxKind::Inline,
                    Display::InlineBlock => BoxKind::InlineBlock,
                    _ => BoxKind::Block,
                }
            }
        }
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

    let replaced = if matches!(kind, BoxKind::ReplacedInline) {
        debug_assert!(replaced_kind.is_some());
        replaced_kind
    } else {
        None
    };

    let replaced_intrinsic = match replaced_kind {
        Some(ReplacedKind::Img) => replaced_info.and_then(|p| p.intrinsic_for_img(styled.node)),
        _ => None,
    };

    Some(LayoutBox {
        kind,
        style,
        node: styled,
        rect,
        children: children_boxes,
        list_marker: None,
        replaced,
        replaced_intrinsic,
    })
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
        "{indent}box[{index}]: id={} node={} kind={} rect={} children={} marker={} replaced={} intrinsic={} style={}",
        layout.node_id().0,
        node_debug_label(layout.node.node),
        box_kind_debug_label(layout.kind),
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

fn node_debug_label(node: &Node) -> String {
    match node {
        Node::Document { .. } => "document".to_string(),
        Node::Element { name, .. } => format!("element(\"{name}\")"),
        Node::Text { text, .. } => format!("text(\"{}\")", text.escape_default()),
        Node::Comment { text, .. } => format!("comment(\"{}\")", text.escape_default()),
    }
}

fn box_kind_debug_label(kind: BoxKind) -> &'static str {
    match kind {
        BoxKind::Block => "block",
        BoxKind::Inline => "inline",
        BoxKind::InlineBlock => "inline-block",
        BoxKind::ReplacedInline => "replaced-inline",
    }
}

fn list_marker_debug_label(marker: Option<ListMarker>) -> String {
    match marker {
        None => "none".to_string(),
        Some(ListMarker::Unordered) => "unordered".to_string(),
        Some(ListMarker::Ordered(value)) => format!("ordered({value})"),
    }
}

fn replaced_kind_debug_label(replaced: Option<ReplacedKind>) -> String {
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

fn intrinsic_size_debug_label(size: Option<IntrinsicSize>) -> String {
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
