use std::collections::HashMap;

use css::StyledNode;
use html::internal::Id;

use crate::{
    BlockFlowBlockPlacement, BlockFormattingParticipation, BoxId, BoxKind, BoxSource,
    ContainingBlockId, DisplayBoxBehavior, FlexFormattingParticipation, FlowParticipation,
    FormattingContextId, FormattingContextKind, InlineFormattingContextId,
    InlineFormattingParticipation, LayoutBox, LayoutPhaseOutput, ListMarker, OverflowPolicy,
    PositionedContainingBlockId, PositioningScheme, Rectangle, ReplacedKind, UsedContentSize,
    flex::{
        FlexContainerCrossAxisLayout, FlexContainerMainAxisLayout, FlexItemCrossAxisLayout,
        FlexItemMainAxisLayout,
    },
    replaced::intrinsic::IntrinsicSize,
};

use crate::box_tree::AnonymousBoxKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedViewportWidthKey(i64);

impl RetainedViewportWidthKey {
    pub fn from_css_px(width: f32) -> Self {
        Self((width * 2.0).round() as i64)
    }

    pub fn value(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedLayoutKeySeed {
    pub identity_domain: u64,
    pub layout_input_generation: u64,
    pub layout_style_generation: u64,
    pub text_measurement_generation: u64,
    pub replaced_metadata_generation: u64,
}

impl RetainedLayoutKeySeed {
    pub fn for_viewport_width(self, width: f32) -> RetainedLayoutKey {
        RetainedLayoutKey {
            identity_domain: self.identity_domain,
            layout_input_generation: self.layout_input_generation,
            layout_style_generation: self.layout_style_generation,
            viewport_width: RetainedViewportWidthKey::from_css_px(width),
            text_measurement_generation: self.text_measurement_generation,
            replaced_metadata_generation: self.replaced_metadata_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedLayoutKey {
    pub identity_domain: u64,
    pub layout_input_generation: u64,
    pub layout_style_generation: u64,
    pub viewport_width: RetainedViewportWidthKey,
    pub text_measurement_generation: u64,
    pub replaced_metadata_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedLayoutFallbackReason {
    MissingRetainedArtifact,
    KeyMismatch,
    MaterializationFailed,
    DirtyLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedLayoutFrameAction {
    Reused,
    Recomputed,
    ConservativeFallback(RetainedLayoutFallbackReason),
}

#[derive(Clone, Debug)]
pub struct RetainedLayoutFrameResult {
    pub key: RetainedLayoutKey,
    pub action: RetainedLayoutFrameAction,
    pub artifact: RetainedLayoutArtifact,
}

#[derive(Clone, Debug)]
pub struct RetainedLayoutArtifact {
    key: RetainedLayoutKey,
    root: RetainedLayoutBox,
}

impl RetainedLayoutArtifact {
    pub fn from_layout_output(key: RetainedLayoutKey, output: &LayoutPhaseOutput<'_, '_>) -> Self {
        Self {
            key,
            root: RetainedLayoutBox::from_layout(output.root()),
        }
    }

    pub fn key(&self) -> RetainedLayoutKey {
        self.key
    }

    pub fn materialize<'style_tree, 'dom>(
        &self,
        style_root: &'style_tree StyledNode<'dom>,
    ) -> Result<LayoutPhaseOutput<'style_tree, 'dom>, RetainedLayoutMaterializationError> {
        let mut anchors = HashMap::new();
        collect_styled_nodes(style_root, &mut anchors);
        let root = self.root.materialize(&anchors)?;
        Ok(LayoutPhaseOutput::new(
            root,
            self.key.viewport_width.value() as f32 / 2.0,
        ))
    }

    #[cfg(test)]
    pub fn contains_frame_local_box_id_debug_text(&self) -> bool {
        format!("{self:?}").contains("BoxId")
    }

    #[cfg(test)]
    pub fn contains_artifact_local_box_ordinal_debug_text(&self) -> bool {
        format!("{self:?}").contains("RetainedLayoutArtifactBoxOrdinal")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetainedLayoutMaterializationError {
    MissingAnchor { node_id: Id },
}

#[derive(Clone, Debug)]
struct RetainedLayoutBox {
    box_ordinal: RetainedLayoutArtifactBoxOrdinal,
    kind: BoxKind,
    display_behavior: DisplayBoxBehavior,
    source: RetainedBoxSource,
    rect: Rectangle,
    children: Vec<RetainedLayoutBox>,
    containing_block: Option<RetainedLayoutArtifactBoxOrdinal>,
    establishes_containing_block: bool,
    positioning_scheme: PositioningScheme,
    flow_participation: FlowParticipation,
    positioned_containing_block: Option<RetainedLayoutArtifactBoxOrdinal>,
    establishes_positioned_containing_block: bool,
    formatting_context: Option<RetainedLayoutArtifactBoxOrdinal>,
    establishes_formatting_context: Option<FormattingContextKind>,
    block_formatting_participation: BlockFormattingParticipation,
    flex_formatting_participation: FlexFormattingParticipation,
    inline_formatting_context: Option<RetainedLayoutArtifactBoxOrdinal>,
    establishes_inline_formatting_context: bool,
    inline_formatting_participation: InlineFormattingParticipation,
    flex_container_main_axis: Option<FlexContainerMainAxisLayout>,
    flex_item_main_axis: Option<FlexItemMainAxisLayout>,
    flex_container_cross_axis: Option<FlexContainerCrossAxisLayout>,
    flex_item_cross_axis: Option<FlexItemCrossAxisLayout>,
    list_marker: Option<ListMarker>,
    replaced: Option<ReplacedKind>,
    replaced_intrinsic: Option<IntrinsicSize>,
    used_content_size: Option<UsedContentSize>,
    block_flow_placement: Option<BlockFlowBlockPlacement>,
    overflow_policy: OverflowPolicy,
}

impl RetainedLayoutBox {
    fn from_layout(layout: &LayoutBox<'_, '_>) -> Self {
        Self {
            box_ordinal: RetainedLayoutArtifactBoxOrdinal::from_frame_local_box_index(
                layout.box_id,
            ),
            kind: layout.kind,
            display_behavior: layout.display_behavior,
            source: RetainedBoxSource::from_box_source(layout.source),
            rect: layout.rect,
            children: layout.children.iter().map(Self::from_layout).collect(),
            containing_block: layout
                .containing_block
                .map(RetainedLayoutArtifactBoxOrdinal::from_frame_local_box_index),
            establishes_containing_block: layout.establishes_containing_block,
            positioning_scheme: layout.positioning_scheme,
            flow_participation: layout.flow_participation,
            positioned_containing_block: layout
                .positioned_containing_block
                .map(RetainedLayoutArtifactBoxOrdinal::from_frame_local_box_index),
            establishes_positioned_containing_block: layout.establishes_positioned_containing_block,
            formatting_context: layout
                .formatting_context
                .map(RetainedLayoutArtifactBoxOrdinal::from_frame_local_box_index),
            establishes_formatting_context: layout.establishes_formatting_context,
            block_formatting_participation: layout.block_formatting_participation,
            flex_formatting_participation: layout.flex_formatting_participation,
            inline_formatting_context: layout
                .inline_formatting_context
                .map(RetainedLayoutArtifactBoxOrdinal::from_frame_local_box_index),
            establishes_inline_formatting_context: layout.establishes_inline_formatting_context,
            inline_formatting_participation: layout.inline_formatting_participation,
            flex_container_main_axis: layout.flex_container_main_axis,
            flex_item_main_axis: layout.flex_item_main_axis,
            flex_container_cross_axis: layout.flex_container_cross_axis,
            flex_item_cross_axis: layout.flex_item_cross_axis,
            list_marker: layout.list_marker,
            replaced: layout.replaced,
            replaced_intrinsic: layout.replaced_intrinsic,
            used_content_size: layout.used_content_size,
            block_flow_placement: layout.block_flow_placement,
            overflow_policy: layout.overflow_policy,
        }
    }

    fn materialize<'style_tree, 'dom>(
        &self,
        anchors: &HashMap<Id, &'style_tree StyledNode<'dom>>,
    ) -> Result<LayoutBox<'style_tree, 'dom>, RetainedLayoutMaterializationError> {
        let anchor = anchors.get(&self.source.anchor_node_id()).copied().ok_or(
            RetainedLayoutMaterializationError::MissingAnchor {
                node_id: self.source.anchor_node_id(),
            },
        )?;
        let source = self.source.materialize(anchor);
        let children = self
            .children
            .iter()
            .map(|child| child.materialize(anchors))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(LayoutBox {
            box_id: self.box_ordinal.to_frame_local_box_id(),
            kind: self.kind,
            display_behavior: self.display_behavior,
            style: &anchor.style,
            source,
            node: anchor,
            rect: self.rect,
            children,
            containing_block: self
                .containing_block
                .map(RetainedLayoutArtifactBoxOrdinal::to_frame_local_containing_block),
            establishes_containing_block: self.establishes_containing_block,
            positioning_scheme: self.positioning_scheme,
            flow_participation: self.flow_participation,
            positioned_containing_block: self
                .positioned_containing_block
                .map(RetainedLayoutArtifactBoxOrdinal::to_frame_local_positioned_containing_block),
            establishes_positioned_containing_block: self.establishes_positioned_containing_block,
            formatting_context: self
                .formatting_context
                .map(RetainedLayoutArtifactBoxOrdinal::to_frame_local_formatting),
            establishes_formatting_context: self.establishes_formatting_context,
            block_formatting_participation: self.block_formatting_participation,
            flex_formatting_participation: self.flex_formatting_participation,
            inline_formatting_context: self
                .inline_formatting_context
                .map(RetainedLayoutArtifactBoxOrdinal::to_frame_local_inline_formatting),
            establishes_inline_formatting_context: self.establishes_inline_formatting_context,
            inline_formatting_participation: self.inline_formatting_participation,
            flex_container_main_axis: self.flex_container_main_axis,
            flex_item_main_axis: self.flex_item_main_axis,
            flex_container_cross_axis: self.flex_container_cross_axis,
            flex_item_cross_axis: self.flex_item_cross_axis,
            list_marker: self.list_marker,
            replaced: self.replaced,
            replaced_intrinsic: self.replaced_intrinsic,
            used_content_size: self.used_content_size,
            block_flow_placement: self.block_flow_placement,
            overflow_policy: self.overflow_policy,
        })
    }
}

/// Artifact-local structural ordinal used to reconstruct a `LayoutBox` tree.
///
/// This value is copied from the frame-local layout box index when the artifact
/// is captured and converted back into frame-local layout IDs only while
/// materializing a borrowed `LayoutPhaseOutput` for the current frame. It is
/// not a retained identity, not stable across separately captured artifacts,
/// and not part of the browser/runtime retained render identity domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RetainedLayoutArtifactBoxOrdinal(u64);

impl RetainedLayoutArtifactBoxOrdinal {
    fn from_frame_local_box_index<T: FrameLocalBoxIndex>(id: T) -> Self {
        Self(id.index() as u64)
    }

    fn to_frame_local_box_id(self) -> BoxId {
        BoxId::from_index(self.0 as usize)
    }

    fn to_frame_local_containing_block(self) -> ContainingBlockId {
        ContainingBlockId::from_box_id(self.to_frame_local_box_id())
    }

    fn to_frame_local_positioned_containing_block(self) -> PositionedContainingBlockId {
        PositionedContainingBlockId::from_box_id(self.to_frame_local_box_id())
    }

    fn to_frame_local_formatting(self) -> FormattingContextId {
        FormattingContextId::from_box_id(self.to_frame_local_box_id())
    }

    fn to_frame_local_inline_formatting(self) -> InlineFormattingContextId {
        InlineFormattingContextId::from_box_id(self.to_frame_local_box_id())
    }
}

trait FrameLocalBoxIndex {
    fn index(self) -> usize;
}

impl FrameLocalBoxIndex for BoxId {
    fn index(self) -> usize {
        self.index()
    }
}

impl FrameLocalBoxIndex for ContainingBlockId {
    fn index(self) -> usize {
        self.index()
    }
}

impl FrameLocalBoxIndex for PositionedContainingBlockId {
    fn index(self) -> usize {
        self.index()
    }
}

impl FrameLocalBoxIndex for FormattingContextId {
    fn index(self) -> usize {
        self.index()
    }
}

impl FrameLocalBoxIndex for InlineFormattingContextId {
    fn index(self) -> usize {
        self.index()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RetainedBoxSource {
    DomNode {
        node_id: Id,
    },
    Anonymous {
        parent_node_id: Id,
        kind: AnonymousBoxKind,
    },
    Marker {
        list_item_node_id: Id,
    },
}

impl RetainedBoxSource {
    fn from_box_source(source: BoxSource<'_, '_>) -> Self {
        match source {
            BoxSource::DomNode(node) => Self::DomNode {
                node_id: node.node_id,
            },
            BoxSource::Anonymous { parent, kind } => Self::Anonymous {
                parent_node_id: parent.node_id,
                kind,
            },
            BoxSource::Marker { list_item } => Self::Marker {
                list_item_node_id: list_item.node_id,
            },
        }
    }

    fn anchor_node_id(self) -> Id {
        match self {
            Self::DomNode { node_id } => node_id,
            Self::Anonymous { parent_node_id, .. } => parent_node_id,
            Self::Marker { list_item_node_id } => list_item_node_id,
        }
    }

    fn materialize<'style_tree, 'dom>(
        self,
        anchor: &'style_tree StyledNode<'dom>,
    ) -> BoxSource<'style_tree, 'dom> {
        match self {
            Self::DomNode { .. } => BoxSource::DomNode(anchor),
            Self::Anonymous { kind, .. } => BoxSource::Anonymous {
                parent: anchor,
                kind,
            },
            Self::Marker { .. } => BoxSource::Marker { list_item: anchor },
        }
    }
}

fn collect_styled_nodes<'style_tree, 'dom>(
    node: &'style_tree StyledNode<'dom>,
    nodes: &mut HashMap<Id, &'style_tree StyledNode<'dom>>,
) {
    nodes.insert(node.node_id, node);
    for child in &node.children {
        collect_styled_nodes(child, nodes);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use css::{ComputedStyle, Length, build_style_tree};
    use html::{Node, internal::Id};

    use super::*;
    use crate::{LayoutPhaseInput, TextMeasurer, layout_document};

    struct TestMeasurer;

    impl TextMeasurer for TestMeasurer {
        fn measure(&self, text: &str, style: &ComputedStyle) -> f32 {
            let Length::Px(font_px) = style.font_size();
            text.chars().count() as f32 * font_px * 0.5
        }

        fn line_height(&self, style: &ComputedStyle) -> f32 {
            let Length::Px(font_px) = style.font_size();
            font_px * 1.2
        }
    }

    #[test]
    fn retained_layout_artifact_materializes_with_current_style_tree_refs() {
        let dom = element(
            1,
            "div",
            vec![("display", "block"), ("width", "100px")],
            vec![text(2, "hello")],
        );
        let styled = build_style_tree(&dom, None);
        let layout = layout_document(LayoutPhaseInput::new(&styled, 320.0, &TestMeasurer, None));
        let key = RetainedLayoutKeySeed {
            identity_domain: 1,
            layout_input_generation: 1,
            layout_style_generation: 1,
            text_measurement_generation: 0,
            replaced_metadata_generation: 0,
        }
        .for_viewport_width(320.0);
        let artifact = RetainedLayoutArtifact::from_layout_output(key, &layout);

        let updated_dom = element(
            1,
            "div",
            vec![
                ("display", "block"),
                ("width", "100px"),
                ("background-color", "red"),
            ],
            vec![text(2, "hello")],
        );
        let updated_styled = build_style_tree(&updated_dom, None);
        let materialized = artifact
            .materialize(&updated_styled)
            .expect("retained layout should materialize");

        assert_eq!(materialized.document_rect(), layout.document_rect());
        assert_eq!(
            materialized.root().style.background_color(),
            (255, 0, 0, 255)
        );
        assert!(!artifact.contains_frame_local_box_id_debug_text());
        assert!(artifact.contains_artifact_local_box_ordinal_debug_text());
    }

    #[test]
    fn retained_layout_artifact_rejects_missing_anchors() {
        let dom = element(1, "div", vec![("display", "block")], vec![text(2, "hello")]);
        let styled = build_style_tree(&dom, None);
        let layout = layout_document(LayoutPhaseInput::new(&styled, 320.0, &TestMeasurer, None));
        let key = RetainedLayoutKeySeed {
            identity_domain: 1,
            layout_input_generation: 1,
            layout_style_generation: 1,
            text_measurement_generation: 0,
            replaced_metadata_generation: 0,
        }
        .for_viewport_width(320.0);
        let artifact = RetainedLayoutArtifact::from_layout_output(key, &layout);
        let changed_dom = element(10, "div", vec![("display", "block")], Vec::new());
        let changed_styled = build_style_tree(&changed_dom, None);

        let Err(error) = artifact.materialize(&changed_styled) else {
            panic!("missing retained anchors should reject materialization");
        };
        assert_eq!(
            error,
            RetainedLayoutMaterializationError::MissingAnchor { node_id: Id(1) }
        );
    }

    fn element(id: u32, name: &str, style: Vec<(&str, &str)>, children: Vec<Node>) -> Node {
        html::internal::node_element_from_parts(
            Id(id),
            Arc::from(name),
            Vec::new(),
            style
                .into_iter()
                .map(|(property, value)| (property.to_string(), value.to_string()))
                .collect(),
            children,
        )
    }

    fn text(id: u32, value: &str) -> Node {
        Node::Text {
            id: Id(id),
            text: value.to_string(),
        }
    }
}
