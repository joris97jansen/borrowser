use std::fmt::Write;

use css::{Display, Length, TextDecorationLine};
use html::dom_utils::is_non_rendering_element;
use html::internal::Id;
use layout::inline::{InlineFragment, layout_inline_for_paint};
use layout::{LayoutBox, LayoutPhaseOutput, ListMarker, Rectangle, ReplacedKind, TextMeasurer};

use super::PaintPhaseInput;
use super::contracts::PaintOrderPhase;
use super::stacking::{
    StackingContextId, StackingContextSource, StackingContextTree, StackingOrderSlot,
};

/// Paint-owned semantic input derived from layout output for one paint phase.
///
/// This is frame-local semantic paint data. It is not a retained display list,
/// scene graph, backend command buffer, or compositor structure.
pub struct PaintInput<'layout, 'style_tree, 'dom> {
    layout: &'layout LayoutPhaseOutput<'style_tree, 'dom>,
    artifact: PaintArtifact,
}

impl<'layout, 'style_tree, 'dom> PaintInput<'layout, 'style_tree, 'dom> {
    pub fn from_phase_input(
        input: PaintPhaseInput<'layout, 'style_tree, 'dom>,
        measurer: &dyn TextMeasurer,
    ) -> Self {
        Self {
            layout: input.layout(),
            artifact: PaintArtifact::from_phase_input(input, measurer),
        }
    }

    pub fn layout(&self) -> &'layout LayoutPhaseOutput<'style_tree, 'dom> {
        self.layout
    }

    pub fn tree(&self) -> &PaintTree {
        self.artifact.tree()
    }

    pub fn stacking_contexts(&self) -> &StackingContextTree {
        self.artifact.stacking_contexts()
    }

    pub fn artifact(&self) -> &PaintArtifact {
        &self.artifact
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
        self.tree()
            .append_debug_snapshot(&mut out)
            .expect("write paint input snapshot");
        out
    }

    /// Stable semantic paint-order snapshot for the supported AA subset.
    ///
    /// This walks the paint tree in construction order. It is a debug and
    /// regression surface, not a sorted display list or retained paint scene.
    pub fn to_order_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write paint order snapshot");
        writeln!(&mut out, "paint-order").expect("write paint order snapshot");
        writeln!(
            &mut out,
            "layout-root-id: {}",
            self.layout.root().node_id().0
        )
        .expect("write paint order snapshot");
        self.append_stacking_order_debug_snapshot(&mut out)
            .expect("write paint order snapshot");
        out
    }

    /// Stable paint-owned stacking-context snapshot for AB2.
    ///
    /// This is a frame-local semantic representation derived before immediate
    /// backend drawing. It is not a compositor layer tree, retained scene,
    /// display list, backend command stream, or paint invalidation surface.
    pub fn to_stacking_context_debug_snapshot(&self) -> String {
        self.stacking_contexts().to_debug_snapshot()
    }

    /// Stable paint-owned layering snapshot for AB7 regression tests.
    ///
    /// This exposes the canonical stacking-context slot order used by semantic
    /// order snapshots, operation snapshots, and immediate painting. It is not
    /// a compositor layer tree, retained scene, display list, backend command
    /// stream, or paint invalidation surface.
    pub fn to_layering_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write paint layering snapshot");
        writeln!(&mut out, "paint-layering-snapshot").expect("write paint layering snapshot");
        writeln!(
            &mut out,
            "layout-root-id: {}",
            self.layout.root().node_id().0
        )
        .expect("write paint layering snapshot");
        writeln!(
            &mut out,
            "root-context: {}",
            self.stacking_contexts().root_id().index()
        )
        .expect("write paint layering snapshot");
        self.append_layering_context_debug_snapshot(
            self.stacking_contexts().root_id(),
            &mut out,
            0,
        )
        .expect("write paint layering snapshot");
        out
    }

    /// Stable paint-owned operation snapshot for visual regression tests.
    ///
    /// This is derived from semantic paint primitives and AA ordering rules.
    /// It is not a backend command stream, pixel snapshot, retained display
    /// list, scene graph, or compositor artifact.
    pub fn to_operation_debug_snapshot(&self) -> String {
        super::debug::paint_operation_debug_snapshot(self)
    }

    fn append_stacking_order_debug_snapshot(&self, out: &mut String) -> std::fmt::Result {
        writeln!(out, "paint-tree-order")?;
        self.append_context_order_debug_snapshot(self.stacking_contexts().root_id(), out, 0)
    }

    fn append_context_order_debug_snapshot(
        &self,
        context_id: StackingContextId,
        out: &mut String,
        depth: usize,
    ) -> std::fmt::Result {
        for slot in self.stacking_contexts().ordered_slots(context_id) {
            match slot {
                StackingOrderSlot::ChildContext(child_context_id) => {
                    let Some(child) = self.stacking_contexts().context(child_context_id) else {
                        continue;
                    };
                    let indent = "  ".repeat(depth);
                    writeln!(
                        out,
                        "{}phase=stacking-context child-context id={} layer={} z-index={} tree-order={}",
                        indent,
                        child.id().index(),
                        child.order_key().layer().debug_label(),
                        optional_i32_debug_label(child.order_key().z_index()),
                        child.order_key().tree_order(),
                    )?;
                    self.append_context_order_debug_snapshot(child.id(), out, depth + 1)?;
                }
                StackingOrderSlot::ContextSource(source) => {
                    if let Some(node) = self.tree().node_for_source(source) {
                        node.append_order_debug_snapshot_with_stacking_contexts(
                            out,
                            depth,
                            context_id,
                            self.stacking_contexts(),
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    fn append_layering_context_debug_snapshot(
        &self,
        context_id: StackingContextId,
        out: &mut String,
        depth: usize,
    ) -> std::fmt::Result {
        let Some(context) = self.stacking_contexts().context(context_id) else {
            return Ok(());
        };
        let indent = "  ".repeat(depth);
        writeln!(
            out,
            "{}context id={} parent={} source={} layer={} z-index={} tree-order={} children={} items={}",
            indent,
            context.id().index(),
            optional_context_debug_label(context.parent()),
            context_source_debug_label(context.source()),
            context.order_key().layer().debug_label(),
            optional_i32_debug_label(context.order_key().z_index()),
            context.order_key().tree_order(),
            context.children().len(),
            context.items().len()
        )?;
        writeln!(out, "{}  items:", indent)?;
        for (index, item) in context.items().iter().enumerate() {
            writeln!(
                out,
                "{}    item[{index}]: source={} layer={} z-index={} tree-order={}",
                indent,
                paint_source_debug_label(item.source()),
                item.order_key().layer().debug_label(),
                optional_i32_debug_label(item.order_key().z_index()),
                item.order_key().tree_order()
            )?;
        }
        writeln!(out, "{}  ordered-slots:", indent)?;
        for (index, slot) in self
            .stacking_contexts()
            .ordered_slots(context_id)
            .iter()
            .enumerate()
        {
            match *slot {
                StackingOrderSlot::ChildContext(child_context_id) => {
                    let Some(child) = self.stacking_contexts().context(child_context_id) else {
                        continue;
                    };
                    writeln!(
                        out,
                        "{}    slot[{index}]: child-context id={} source={} layer={} z-index={} tree-order={}",
                        indent,
                        child.id().index(),
                        context_source_debug_label(child.source()),
                        child.order_key().layer().debug_label(),
                        optional_i32_debug_label(child.order_key().z_index()),
                        child.order_key().tree_order()
                    )?;
                    self.append_layering_context_debug_snapshot(child.id(), out, depth + 2)?;
                }
                StackingOrderSlot::ContextSource(source) => {
                    writeln!(
                        out,
                        "{}    slot[{index}]: context-source source={}",
                        indent,
                        paint_source_debug_label(source)
                    )?;
                }
            }
        }

        Ok(())
    }
}

/// Paint-owned semantic artifact derived from layout output for immediate
/// painting.
///
/// This owned artifact is not a browser/runtime cache key, backend command
/// stream, display-list architecture, compositor layer tree, or GPU resource.
/// Browser/runtime may retain it, but paint remains the owner of its semantic
/// meaning.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintArtifact {
    tree: PaintTree,
    stacking_contexts: StackingContextTree,
}

impl PaintArtifact {
    pub fn from_phase_input(
        input: PaintPhaseInput<'_, '_, '_>,
        measurer: &dyn TextMeasurer,
    ) -> Self {
        Self {
            tree: PaintTree::from_layout(input.layout_root(), measurer),
            stacking_contexts: StackingContextTree::from_layout(input.layout_root()),
        }
    }

    pub fn tree(&self) -> &PaintTree {
        &self.tree
    }

    pub fn stacking_contexts(&self) -> &StackingContextTree {
        &self.stacking_contexts
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write paint artifact snapshot");
        writeln!(&mut out, "paint-artifact").expect("write paint artifact snapshot");
        self.tree
            .append_debug_snapshot(&mut out)
            .expect("write paint artifact snapshot");
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

    pub fn node_for_source(&self, source: PaintSource) -> Option<&PaintNode> {
        self.root.node_for_source(source)
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

    fn node_for_source(&self, source: PaintSource) -> Option<&PaintNode> {
        if self.source == source {
            return Some(self);
        }

        self.children
            .iter()
            .find_map(|child| child.node_for_source(source))
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

    fn append_order_debug_snapshot_with_stacking_contexts(
        &self,
        out: &mut String,
        depth: usize,
        owner_context: StackingContextId,
        stacking_contexts: &StackingContextTree,
    ) -> std::fmt::Result {
        let indent = "  ".repeat(depth);
        writeln!(
            out,
            "{}box={} node={} anonymous={}",
            indent, self.source.box_id, self.source.node_id.0, self.source.anonymous
        )?;
        for primitive in &self.primitives {
            writeln!(
                out,
                "{}  phase={} primitive {}",
                indent,
                primitive.order_phase().debug_label(),
                primitive.to_debug_label()
            )?;
        }
        for child in &self.children {
            if stacking_contexts.source_starts_external_context(owner_context, child.source) {
                continue;
            }

            writeln!(
                out,
                "{}  phase={} child-subtree box={} node={}",
                indent,
                PaintOrderPhase::ChildSubtree.debug_label(),
                child.source.box_id,
                child.source.node_id.0
            )?;
            child.append_order_debug_snapshot_with_stacking_contexts(
                out,
                depth + 1,
                owner_context,
                stacking_contexts,
            )?;
        }
        for primitive in &self.post_primitives {
            writeln!(
                out,
                "{}  phase={} primitive {}",
                indent,
                primitive.order_phase().debug_label(),
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

    pub fn order_phase(&self) -> PaintOrderPhase {
        match self {
            Self::Background(_) => PaintOrderPhase::BoxBackground,
            Self::Border(_) => PaintOrderPhase::BoxBorder,
            Self::Outline(_) => PaintOrderPhase::BoxOutline,
            Self::ListMarker(_) => PaintOrderPhase::ListMarker,
            Self::Clip(_) => PaintOrderPhase::OverflowClipForContentsAndDescendants,
            Self::Text(_) | Self::TextDecoration(_) | Self::InlineBox(_) | Self::Replaced(_) => {
                PaintOrderPhase::InlineFormattingContent
            }
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

fn optional_i32_debug_label(value: Option<i32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "auto".to_string())
}

fn optional_context_debug_label(id: Option<StackingContextId>) -> String {
    id.map(|id| id.index().to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn context_source_debug_label(source: StackingContextSource) -> String {
    match source {
        StackingContextSource::RootDocument(source) => {
            format!("root-document({})", paint_source_debug_label(source))
        }
        StackingContextSource::PositionedElement(source) => {
            format!("positioned-element({})", paint_source_debug_label(source))
        }
    }
}

fn paint_source_debug_label(source: PaintSource) -> String {
    format!(
        "box={} node={} anonymous={}",
        source.box_id, source.node_id.0, source.anonymous
    )
}

#[cfg(test)]
mod tests {
    use super::super::stacking::{
        StackingContextId, StackingContextSource, StackingLayerKind, StackingOrderSlot,
    };
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

    fn build_paint_order_snapshot(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        PaintInput::from_phase_input(PaintPhaseInput::new(&layout), &TestMeasurer)
            .to_order_debug_snapshot()
    }

    fn build_stacking_context_snapshot(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        PaintInput::from_phase_input(PaintPhaseInput::new(&layout), &TestMeasurer)
            .to_stacking_context_debug_snapshot()
    }

    fn build_layering_snapshot(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        PaintInput::from_phase_input(PaintPhaseInput::new(&layout), &TestMeasurer)
            .to_layering_debug_snapshot()
    }

    fn build_paint_operation_snapshot(dom: &Node) -> String {
        let styled = css::build_style_tree(dom, None);
        let layout =
            layout::layout_document(LayoutPhaseInput::new(&styled, 500.0, &TestMeasurer, None));
        PaintInput::from_phase_input(PaintPhaseInput::new(&layout), &TestMeasurer)
            .to_operation_debug_snapshot()
    }

    fn line_index(snapshot: &str, pattern: &str) -> usize {
        snapshot
            .lines()
            .position(|line| line.contains(pattern))
            .unwrap_or_else(|| panic!("snapshot should contain {pattern:?}\n{snapshot}"))
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

    fn positioned_block(id: Id, z_index: &str, color: &str, children: Vec<Node>) -> Node {
        Node::Element {
            id,
            name: Arc::from("div"),
            attributes: Vec::new(),
            style: vec![
                ("display".to_string(), "block".to_string()),
                ("position".to_string(), "relative".to_string()),
                ("z-index".to_string(), z_index.to_string()),
                ("width".to_string(), "20px".to_string()),
                ("height".to_string(), "20px".to_string()),
                ("background-color".to_string(), color.to_string()),
            ],
            children,
        }
    }

    fn child_context_node_ids(contexts: &StackingContextTree, layer: StackingLayerKind) -> Vec<Id> {
        contexts
            .child_contexts_for_layer(StackingContextId::ROOT, layer)
            .iter()
            .map(|context| context.source().paint_source().node_id)
            .collect()
    }

    fn ordered_slot_labels(
        contexts: &StackingContextTree,
        context: StackingContextId,
    ) -> Vec<String> {
        contexts
            .ordered_slots(context)
            .iter()
            .map(|slot| match *slot {
                StackingOrderSlot::ChildContext(child_context) => {
                    let node_id = contexts
                        .context(child_context)
                        .expect("child context")
                        .source()
                        .paint_source()
                        .node_id;
                    format!("child-context({})", node_id.0)
                }
                StackingOrderSlot::ContextSource(source) => {
                    format!("context-source({})", source.node_id.0)
                }
            })
            .collect()
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

    fn flattened_paint_phases(node: &PaintNode) -> Vec<PaintOrderPhase> {
        let mut order = node
            .primitives()
            .iter()
            .map(PaintPrimitive::order_phase)
            .collect::<Vec<_>>();
        for child in node.children() {
            order.push(PaintOrderPhase::ChildSubtree);
            order.extend(flattened_paint_phases(child));
        }
        order.extend(
            node.post_primitives()
                .iter()
                .map(PaintPrimitive::order_phase),
        );
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
    fn stacking_context_tree_always_has_deterministic_root_context() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: Vec::new(),
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let contexts = input.stacking_contexts();
        let root = contexts.root();

        assert_eq!(contexts.root_id(), StackingContextId::ROOT);
        assert_eq!(root.id(), StackingContextId::ROOT);
        assert_eq!(root.parent(), None);
        assert!(root.children().is_empty());
        assert_eq!(contexts.contexts().len(), 1);
        assert_eq!(contexts.context(StackingContextId::ROOT), Some(root));
        assert!(matches!(
            root.source(),
            StackingContextSource::RootDocument(PaintSource {
                box_id: 0,
                node_id: Id(1),
                anonymous: false,
            })
        ));
    }

    #[test]
    fn stacking_context_tree_associates_paintable_boxes_with_root_in_layout_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("display".to_string(), "block".to_string())],
                children: vec![
                    Node::Element {
                        id: Id(3),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![("display".to_string(), "block".to_string())],
                        children: Vec::new(),
                    },
                    Node::Element {
                        id: Id(4),
                        name: Arc::from("aside"),
                        attributes: Vec::new(),
                        style: vec![("display".to_string(), "block".to_string())],
                        children: Vec::new(),
                    },
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let root = input.stacking_contexts().root();
        let item_sources = root
            .items()
            .iter()
            .map(|item| item.source())
            .collect::<Vec<_>>();

        assert_eq!(
            item_sources,
            vec![
                PaintSource {
                    box_id: 0,
                    node_id: Id(1),
                    anonymous: false,
                },
                PaintSource {
                    box_id: 1,
                    node_id: Id(2),
                    anonymous: false,
                },
                PaintSource {
                    box_id: 2,
                    node_id: Id(3),
                    anonymous: false,
                },
                PaintSource {
                    box_id: 3,
                    node_id: Id(4),
                    anonymous: false,
                },
            ]
        );

        for item in root.items() {
            assert_eq!(item.context(), StackingContextId::ROOT);
            assert_eq!(
                input.stacking_contexts().context_for_source(item.source()),
                Some(StackingContextId::ROOT)
            );
        }
    }

    #[test]
    fn stacking_context_debug_snapshot_is_deterministic_and_backend_independent() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: vec![Node::Element {
                    id: Id(3),
                    name: Arc::from("div"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: Vec::new(),
                }],
            }],
        };

        let first = build_stacking_context_snapshot(&dom);
        let second = build_stacking_context_snapshot(&dom);

        assert_eq!(first, second);
        assert!(first.starts_with("version: 2\nstacking-context-tree\n"));
        assert!(first.contains("root-context: 0"));
        assert!(first.contains(
            "context id=0 parent=none source=root-document(box=0 node=1 anonymous=false) layer=normal-flow z-index=auto tree-order=0"
        ));
        assert!(first.contains(
            "item source=box=1 node=2 anonymous=false context=0 layer=normal-flow z-index=auto"
        ));
        assert!(!first.contains("egui"));
        assert!(!first.contains("gpu"));
        assert!(!first.contains("texture"));
        assert!(!first.contains("compositor"));
    }

    #[test]
    fn paint_layering_snapshot_exact_overlap_fixture() {
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
                    ("height".to_string(), "20px".to_string()),
                    ("background-color".to_string(), "#00aa00".to_string()),
                ],
                children: vec![
                    positioned_block(Id(3), "0", "#0000aa", Vec::new()),
                    positioned_block(Id(4), "-1", "#aa0000", Vec::new()),
                    positioned_block(Id(5), "2", "#aaaa00", Vec::new()),
                ],
            }],
        };

        let snapshot = build_layering_snapshot(&dom);
        let expected = concat!(
            "version: 1\n",
            "paint-layering-snapshot\n",
            "layout-root-id: 1\n",
            "root-context: 0\n",
            "context id=0 parent=none source=root-document(box=0 node=1 anonymous=false) layer=normal-flow z-index=auto tree-order=0 children=3 items=2\n",
            "  items:\n",
            "    item[0]: source=box=0 node=1 anonymous=false layer=normal-flow z-index=auto tree-order=0\n",
            "    item[1]: source=box=1 node=2 anonymous=false layer=normal-flow z-index=auto tree-order=1\n",
            "  ordered-slots:\n",
            "    slot[0]: child-context id=2 source=positioned-element(box=3 node=4 anonymous=false) layer=negative-z-index z-index=-1 tree-order=3\n",
            "    context id=2 parent=0 source=positioned-element(box=3 node=4 anonymous=false) layer=negative-z-index z-index=-1 tree-order=3 children=0 items=1\n",
            "      items:\n",
            "        item[0]: source=box=3 node=4 anonymous=false layer=normal-flow z-index=auto tree-order=3\n",
            "      ordered-slots:\n",
            "        slot[0]: context-source source=box=3 node=4 anonymous=false\n",
            "    slot[1]: context-source source=box=0 node=1 anonymous=false\n",
            "    slot[2]: child-context id=1 source=positioned-element(box=2 node=3 anonymous=false) layer=zero-z-index z-index=0 tree-order=2\n",
            "    context id=1 parent=0 source=positioned-element(box=2 node=3 anonymous=false) layer=zero-z-index z-index=0 tree-order=2 children=0 items=1\n",
            "      items:\n",
            "        item[0]: source=box=2 node=3 anonymous=false layer=normal-flow z-index=auto tree-order=2\n",
            "      ordered-slots:\n",
            "        slot[0]: context-source source=box=2 node=3 anonymous=false\n",
            "    slot[3]: child-context id=3 source=positioned-element(box=4 node=5 anonymous=false) layer=positive-z-index z-index=2 tree-order=4\n",
            "    context id=3 parent=0 source=positioned-element(box=4 node=5 anonymous=false) layer=positive-z-index z-index=2 tree-order=4 children=0 items=1\n",
            "      items:\n",
            "        item[0]: source=box=4 node=5 anonymous=false layer=normal-flow z-index=auto tree-order=4\n",
            "      ordered-slots:\n",
            "        slot[0]: context-source source=box=4 node=5 anonymous=false\n",
        );

        assert_eq!(snapshot, expected);
    }

    #[test]
    fn paint_layering_snapshot_keeps_child_context_atomic() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("display".to_string(), "block".to_string())],
                children: vec![
                    positioned_block(
                        Id(3),
                        "1",
                        "#aa0000",
                        vec![positioned_block(Id(4), "-1", "#00aa00", Vec::new())],
                    ),
                    positioned_block(Id(5), "2", "#0000aa", Vec::new()),
                ],
            }],
        };

        let snapshot = build_layering_snapshot(&dom);
        let parent_context = snapshot
            .lines()
            .position(|line| {
                line.contains("source=positioned-element(box=2 node=3 anonymous=false)")
                    && line.contains("layer=positive-z-index")
            })
            .expect("parent child context");
        let nested_context = snapshot
            .lines()
            .position(|line| {
                line.contains("source=positioned-element(box=3 node=4 anonymous=false)")
                    && line.contains("layer=negative-z-index")
            })
            .expect("nested child context");
        let sibling_context = snapshot
            .lines()
            .position(|line| {
                line.contains("source=positioned-element(box=4 node=5 anonymous=false)")
                    && line.contains("layer=positive-z-index")
            })
            .expect("sibling child context");

        assert!(parent_context < nested_context);
        assert!(nested_context < sibling_context);
    }

    #[test]
    fn overflow_clips_do_not_create_stacking_contexts() {
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
                    ("overflow".to_string(), "clip".to_string()),
                ],
                children: Vec::new(),
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let section = non_anonymous_node_by_source_id(input.tree().root(), Id(2))
            .expect("section paint node");

        assert!(
            section
                .primitives()
                .iter()
                .any(|primitive| matches!(primitive, PaintPrimitive::Clip(_)))
        );
        assert_eq!(input.stacking_contexts().contexts().len(), 1);
        assert_eq!(input.stacking_contexts().root().items().len(), 2);
    }

    #[test]
    fn positioned_integer_z_index_creates_child_context_only_for_supported_trigger() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("display".to_string(), "block".to_string())],
                children: vec![
                    Node::Element {
                        id: Id(3),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![
                            ("display".to_string(), "block".to_string()),
                            ("position".to_string(), "relative".to_string()),
                            ("z-index".to_string(), "2".to_string()),
                        ],
                        children: Vec::new(),
                    },
                    Node::Element {
                        id: Id(4),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![
                            ("display".to_string(), "block".to_string()),
                            ("z-index".to_string(), "9".to_string()),
                        ],
                        children: Vec::new(),
                    },
                    Node::Element {
                        id: Id(5),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![
                            ("display".to_string(), "block".to_string()),
                            ("position".to_string(), "relative".to_string()),
                            ("z-index".to_string(), "auto".to_string()),
                        ],
                        children: Vec::new(),
                    },
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let contexts = input.stacking_contexts();
        let root = contexts.root();

        assert_eq!(contexts.contexts().len(), 2);
        assert_eq!(root.children().len(), 1);
        let child_id = root.children()[0];
        let child = contexts.context(child_id).expect("child context");
        assert!(matches!(
            child.source(),
            StackingContextSource::PositionedElement(PaintSource { node_id: Id(3), .. })
        ));
        assert_eq!(child.order_key().layer(), StackingLayerKind::PositiveZIndex);
        assert_eq!(child.order_key().z_index(), Some(2));
        assert_eq!(
            contexts.context_for_source(child.source().paint_source()),
            Some(child_id)
        );
        assert!(
            root.items()
                .iter()
                .any(|item| item.source().node_id == Id(4))
        );
        assert!(
            root.items()
                .iter()
                .any(|item| item.source().node_id == Id(5))
        );
    }

    #[test]
    fn child_context_layers_and_ties_are_deterministic() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("display".to_string(), "block".to_string())],
                children: vec![
                    positioned_block(Id(3), "-1", "#111111", Vec::new()),
                    positioned_block(Id(4), "0", "#222222", Vec::new()),
                    positioned_block(Id(5), "2", "#333333", Vec::new()),
                    positioned_block(Id(6), "2", "#444444", Vec::new()),
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let contexts = input.stacking_contexts();

        assert_eq!(
            child_context_node_ids(contexts, StackingLayerKind::NegativeZIndex),
            vec![Id(3)]
        );
        assert_eq!(
            child_context_node_ids(contexts, StackingLayerKind::ZeroZIndex),
            vec![Id(4)]
        );
        assert_eq!(
            child_context_node_ids(contexts, StackingLayerKind::PositiveZIndex),
            vec![Id(5), Id(6)]
        );
    }

    #[test]
    fn stacking_context_ordered_slots_are_canonical_ab4_paint_order() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("display".to_string(), "block".to_string())],
                children: vec![
                    positioned_block(Id(3), "0", "#111111", Vec::new()),
                    positioned_block(Id(4), "-2", "#222222", Vec::new()),
                    positioned_block(Id(5), "3", "#333333", Vec::new()),
                    positioned_block(Id(6), "3", "#444444", Vec::new()),
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);

        assert_eq!(
            ordered_slot_labels(input.stacking_contexts(), StackingContextId::ROOT),
            vec![
                "child-context(4)",
                "context-source(1)",
                "child-context(3)",
                "child-context(5)",
                "child-context(6)",
            ]
        );
    }

    #[test]
    fn child_context_roots_are_external_to_parent_source_traversal() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("display".to_string(), "block".to_string())],
                children: vec![
                    positioned_block(Id(3), "1", "#111111", Vec::new()),
                    Node::Element {
                        id: Id(4),
                        name: Arc::from("div"),
                        attributes: Vec::new(),
                        style: vec![("display".to_string(), "block".to_string())],
                        children: Vec::new(),
                    },
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let child_context = input
            .stacking_contexts()
            .contexts()
            .iter()
            .find(|context| context.source().paint_source().node_id == Id(3))
            .expect("positioned child context");
        let normal_source = input
            .stacking_contexts()
            .root()
            .items()
            .iter()
            .find(|item| item.source().node_id == Id(4))
            .expect("normal root-context item")
            .source();

        assert!(input.stacking_contexts().source_starts_external_context(
            StackingContextId::ROOT,
            child_context.source().paint_source()
        ));
        assert!(
            !input
                .stacking_contexts()
                .source_starts_external_context(StackingContextId::ROOT, normal_source)
        );
    }

    #[test]
    fn nested_positioned_z_index_contexts_are_owned_by_nearest_context() {
        let dom = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: Id(2),
                name: Arc::from("section"),
                attributes: Vec::new(),
                style: vec![("display".to_string(), "block".to_string())],
                children: vec![
                    positioned_block(
                        Id(3),
                        "1",
                        "#111111",
                        vec![positioned_block(Id(4), "-1", "#222222", Vec::new())],
                    ),
                    positioned_block(Id(5), "2", "#333333", Vec::new()),
                ],
            }],
        };

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let contexts = input.stacking_contexts();
        let first = contexts
            .contexts()
            .iter()
            .find(|context| context.source().paint_source().node_id == Id(3))
            .expect("first positioned context");
        let nested = contexts
            .contexts()
            .iter()
            .find(|context| context.source().paint_source().node_id == Id(4))
            .expect("nested positioned context");

        assert_eq!(first.parent(), Some(StackingContextId::ROOT));
        assert_eq!(nested.parent(), Some(first.id()));
        assert_eq!(first.children(), &[nested.id()]);
        assert_eq!(
            child_context_node_ids(contexts, StackingLayerKind::PositiveZIndex),
            vec![Id(3), Id(5)]
        );
        assert_eq!(
            contexts
                .child_contexts_for_layer(first.id(), StackingLayerKind::NegativeZIndex)
                .iter()
                .map(|context| context.source().paint_source().node_id)
                .collect::<Vec<_>>(),
            vec![Id(4)]
        );
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
    fn paint_order_snapshot_records_supported_phases_without_sorting() {
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
                    ("border-top-color".to_string(), "#445566".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "#778899".to_string()),
                ],
                children: vec![Node::Text {
                    id: Id(3),
                    text: "Hello".to_string(),
                }],
            }],
        };

        let snapshot = build_paint_order_snapshot(&dom);
        let second = build_paint_order_snapshot(&dom);

        assert_eq!(snapshot, second);
        assert!(snapshot.contains("paint-order"));
        assert!(snapshot.contains("phase=box-background primitive background"));
        assert!(snapshot.contains("phase=box-border primitive border"));
        assert!(
            snapshot.contains("phase=overflow-clip-for-contents-and-descendants primitive clip")
        );
        assert!(snapshot.contains("phase=inline-formatting-content primitive text"));
        assert!(snapshot.contains("phase=box-outline primitive outline"));
        assert!(!snapshot.contains("stacking-context"));
        assert!(!snapshot.contains("egui"));
    }

    #[test]
    fn paint_order_keeps_parent_background_and_border_before_child_subtree() {
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

        let styled = build_style_tree(&dom);
        let layout = build_layout_for(&styled);
        let input = build_paint_input(&layout);
        let section = non_anonymous_node_by_source_id(input.tree().root(), Id(2))
            .expect("section paint node");

        assert_eq!(
            &flattened_paint_phases(section)[..3],
            &[
                PaintOrderPhase::BoxBackground,
                PaintOrderPhase::BoxBorder,
                PaintOrderPhase::ChildSubtree,
            ]
        );
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
        assert_eq!(
            node.primitives()
                .iter()
                .map(PaintPrimitive::order_phase)
                .collect::<Vec<_>>(),
            vec![
                PaintOrderPhase::InlineFormattingContent,
                PaintOrderPhase::InlineFormattingContent,
            ]
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

        let phases = flattened_paint_phases(node);
        assert_eq!(phases.last(), Some(&PaintOrderPhase::BoxOutline));
        assert!(
            phases
                .iter()
                .position(|phase| *phase == PaintOrderPhase::ChildSubtree)
                .expect("child subtree phase")
                < phases
                    .iter()
                    .rposition(|phase| *phase == PaintOrderPhase::BoxOutline)
                    .expect("outline phase")
        );
    }

    #[test]
    fn paint_order_preserves_layout_sibling_order() {
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
                            ("height".to_string(), "10px".to_string()),
                            ("background-color".to_string(), "#010203".to_string()),
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
                            ("height".to_string(), "10px".to_string()),
                            ("background-color".to_string(), "#040506".to_string()),
                        ],
                        children: Vec::new(),
                    },
                ],
            }],
        };

        let snapshot = build_paint_order_snapshot(&dom);
        let lines = snapshot.lines().collect::<Vec<_>>();
        let first_child = lines
            .iter()
            .position(|line| {
                line.contains("phase=child-subtree child-subtree") && line.contains("node=3")
            })
            .expect("first child subtree order");
        let second_child = lines
            .iter()
            .position(|line| {
                line.contains("phase=child-subtree child-subtree") && line.contains("node=4")
            })
            .expect("second child subtree order");

        assert!(first_child < second_child);
    }

    #[test]
    fn paint_order_snapshot_uses_shared_stacking_context_slots() {
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
                    ("height".to_string(), "20px".to_string()),
                    ("background-color".to_string(), "#00aa00".to_string()),
                ],
                children: vec![
                    positioned_block(Id(3), "0", "#0000aa", Vec::new()),
                    positioned_block(Id(4), "-1", "#aa0000", Vec::new()),
                    positioned_block(Id(5), "2", "#aaaa00", Vec::new()),
                ],
            }],
        };

        let snapshot = build_paint_order_snapshot(&dom);
        let lines = snapshot.lines().collect::<Vec<_>>();
        let negative_context = lines
            .iter()
            .position(|line| {
                line.contains("phase=stacking-context") && line.contains("layer=negative-z-index")
            })
            .expect("negative child context slot");
        let section_background = lines
            .iter()
            .position(|line| {
                line.contains("phase=box-background") && line.contains("color=rgba(0,170,0,255)")
            })
            .expect("normal context source background");
        let zero_context = lines
            .iter()
            .position(|line| {
                line.contains("phase=stacking-context") && line.contains("layer=zero-z-index")
            })
            .expect("zero child context slot");
        let positive_context = lines
            .iter()
            .position(|line| {
                line.contains("phase=stacking-context") && line.contains("layer=positive-z-index")
            })
            .expect("positive child context slot");

        assert!(negative_context < section_background);
        assert!(section_background < zero_context);
        assert!(zero_context < positive_context);
    }

    #[test]
    fn layering_order_snapshot_and_operation_snapshot_stay_aligned() {
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
                    ("height".to_string(), "20px".to_string()),
                    ("background-color".to_string(), "#00aa00".to_string()),
                ],
                children: vec![
                    positioned_block(Id(3), "0", "#0000aa", Vec::new()),
                    positioned_block(Id(4), "-1", "#aa0000", Vec::new()),
                    positioned_block(Id(5), "2", "#aaaa00", Vec::new()),
                ],
            }],
        };

        let layering = build_layering_snapshot(&dom);
        let order = build_paint_order_snapshot(&dom);
        let operations = build_paint_operation_snapshot(&dom);

        let layering_negative = line_index(&layering, "slot[0]: child-context id=2");
        let layering_normal = line_index(&layering, "slot[1]: context-source");
        let layering_zero = line_index(&layering, "slot[2]: child-context id=1");
        let layering_positive = line_index(&layering, "slot[3]: child-context id=3");
        assert!(layering_negative < layering_normal);
        assert!(layering_normal < layering_zero);
        assert!(layering_zero < layering_positive);

        let order_negative = line_index(&order, "layer=negative-z-index");
        let order_normal = line_index(&order, "color=rgba(0,170,0,255)");
        let order_zero = line_index(&order, "layer=zero-z-index");
        let order_positive = line_index(&order, "layer=positive-z-index");
        assert!(order_negative < order_normal);
        assert!(order_normal < order_zero);
        assert!(order_zero < order_positive);

        let operation_negative = line_index(&operations, "color=rgba(170,0,0,255)");
        let operation_normal = line_index(&operations, "color=rgba(0,170,0,255)");
        let operation_zero = line_index(&operations, "color=rgba(0,0,170,255)");
        let operation_positive = line_index(&operations, "color=rgba(170,170,0,255)");
        assert!(operation_negative < operation_normal);
        assert!(operation_normal < operation_zero);
        assert!(operation_zero < operation_positive);
    }

    #[test]
    fn paint_order_keeps_clip_before_contents_without_reordering_box_visuals() {
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
                    ("border-top-color".to_string(), "#445566".to_string()),
                    ("overflow".to_string(), "clip".to_string()),
                    ("outline-width".to_string(), "3px".to_string()),
                    ("outline-style".to_string(), "solid".to_string()),
                    ("outline-color".to_string(), "#778899".to_string()),
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
        let section = non_anonymous_node_by_source_id(input.tree().root(), Id(2))
            .expect("section paint node");

        let phases = flattened_paint_phases(section);
        assert_eq!(
            &phases[..4],
            &[
                PaintOrderPhase::BoxBackground,
                PaintOrderPhase::BoxBorder,
                PaintOrderPhase::OverflowClipForContentsAndDescendants,
                PaintOrderPhase::InlineFormattingContent,
            ]
        );
        assert_eq!(phases.last(), Some(&PaintOrderPhase::BoxOutline));
        assert!(
            phases
                .iter()
                .position(|phase| *phase == PaintOrderPhase::OverflowClipForContentsAndDescendants)
                .expect("overflow clip phase")
                < phases
                    .iter()
                    .position(|phase| *phase == PaintOrderPhase::InlineFormattingContent)
                    .expect("inline formatting phase")
        );
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
