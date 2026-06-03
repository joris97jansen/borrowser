//! Generated box tree data model.

use crate::replaced::intrinsic::IntrinsicSize;
use crate::{
    BoxKind, FlowParticipation, ListMarker, PositioningScheme, ReplacedElementInfoProvider,
    ReplacedKind,
};
use css::{ComputedStyle, Display, StyledNode};
use html::internal::Id;
use std::fmt;

use super::display::{BoxGenerationRole, DisplayBoxBehavior};
use super::formatting::{
    BlockFormattingParticipation, FlexFormattingParticipation, FormattingContextKind,
    InlineFormattingParticipation,
};
use super::ids::{
    BoxId, ContainingBlockId, FormattingContextId, InlineFormattingContextId,
    PositionedContainingBlockId,
};
use super::source::BoxSource;

/// A generated layout box node before final geometry is computed.
pub struct BoxNode<'style_tree, 'dom> {
    pub(super) id: BoxId,
    pub(super) parent: Option<BoxId>,
    pub(super) children: Vec<BoxId>,
    pub(super) role: BoxGenerationRole,
    pub(super) kind: BoxKind,
    pub(super) source: BoxSource<'style_tree, 'dom>,
    pub(super) style: &'style_tree ComputedStyle,
    pub(super) display: Display,
    pub(super) display_behavior: DisplayBoxBehavior,
    pub(super) containing_block: Option<ContainingBlockId>,
    pub(super) establishes_containing_block: bool,
    pub(super) positioning_scheme: PositioningScheme,
    pub(super) flow_participation: FlowParticipation,
    pub(super) positioned_containing_block: Option<PositionedContainingBlockId>,
    pub(super) establishes_positioned_containing_block: bool,
    pub(super) formatting_context: Option<FormattingContextId>,
    pub(super) establishes_formatting_context: Option<FormattingContextKind>,
    pub(super) block_formatting_participation: BlockFormattingParticipation,
    pub(super) flex_formatting_participation: FlexFormattingParticipation,
    pub(super) inline_formatting_context: Option<InlineFormattingContextId>,
    pub(super) establishes_inline_formatting_context: bool,
    pub(super) inline_formatting_participation: InlineFormattingParticipation,
    pub(super) list_marker: Option<ListMarker>,
    pub(super) replaced: Option<ReplacedKind>,
    pub(super) replaced_intrinsic: Option<IntrinsicSize>,
}

impl fmt::Debug for BoxNode<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxNode")
            .field("id", &self.id)
            .field("parent", &self.parent)
            .field("children", &self.children)
            .field("role", &self.role)
            .field("kind", &self.kind)
            .field("source", &self.source)
            .field("display", &self.display)
            .field("display_behavior", &self.display_behavior)
            .field("containing_block", &self.containing_block)
            .field(
                "establishes_containing_block",
                &self.establishes_containing_block,
            )
            .field("positioning_scheme", &self.positioning_scheme)
            .field("flow_participation", &self.flow_participation)
            .field(
                "positioned_containing_block",
                &self.positioned_containing_block,
            )
            .field(
                "establishes_positioned_containing_block",
                &self.establishes_positioned_containing_block,
            )
            .field("formatting_context", &self.formatting_context)
            .field(
                "establishes_formatting_context",
                &self.establishes_formatting_context,
            )
            .field(
                "block_formatting_participation",
                &self.block_formatting_participation,
            )
            .field(
                "flex_formatting_participation",
                &self.flex_formatting_participation,
            )
            .field("inline_formatting_context", &self.inline_formatting_context)
            .field(
                "establishes_inline_formatting_context",
                &self.establishes_inline_formatting_context,
            )
            .field(
                "inline_formatting_participation",
                &self.inline_formatting_participation,
            )
            .field("list_marker", &self.list_marker)
            .field("replaced", &self.replaced)
            .field("replaced_intrinsic", &self.replaced_intrinsic)
            .finish()
    }
}

impl<'style_tree, 'dom> BoxNode<'style_tree, 'dom> {
    pub fn id(&self) -> BoxId {
        self.id
    }

    pub fn parent(&self) -> Option<BoxId> {
        self.parent
    }

    pub fn children(&self) -> &[BoxId] {
        &self.children
    }

    pub fn role(&self) -> BoxGenerationRole {
        self.role
    }

    pub fn kind(&self) -> BoxKind {
        self.kind
    }

    pub fn source(&self) -> BoxSource<'style_tree, 'dom> {
        self.source
    }

    pub fn style(&self) -> &'style_tree ComputedStyle {
        self.style
    }

    pub fn display(&self) -> Display {
        self.display
    }

    pub fn display_behavior(&self) -> DisplayBoxBehavior {
        self.display_behavior
    }

    pub fn containing_block(&self) -> Option<ContainingBlockId> {
        self.containing_block
    }

    pub fn establishes_containing_block(&self) -> bool {
        self.establishes_containing_block
    }

    pub fn positioning_scheme(&self) -> PositioningScheme {
        self.positioning_scheme
    }

    pub fn flow_participation(&self) -> FlowParticipation {
        self.flow_participation
    }

    pub fn positioned_containing_block(&self) -> Option<PositionedContainingBlockId> {
        self.positioned_containing_block
    }

    pub fn establishes_positioned_containing_block(&self) -> bool {
        self.establishes_positioned_containing_block
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

    pub fn flex_formatting_participation(&self) -> FlexFormattingParticipation {
        self.flex_formatting_participation
    }

    pub fn inline_formatting_context(&self) -> Option<InlineFormattingContextId> {
        self.inline_formatting_context
    }

    pub fn establishes_inline_formatting_context(&self) -> bool {
        self.establishes_inline_formatting_context
    }

    pub fn inline_formatting_participation(&self) -> InlineFormattingParticipation {
        self.inline_formatting_participation
    }

    pub fn list_marker(&self) -> Option<ListMarker> {
        self.list_marker
    }

    pub fn replaced(&self) -> Option<ReplacedKind> {
        self.replaced
    }

    pub fn replaced_intrinsic(&self) -> Option<IntrinsicSize> {
        self.replaced_intrinsic
    }

    pub fn direct_node_id(&self) -> Option<Id> {
        self.source.direct_node_id()
    }

    pub fn anchor_node_id(&self) -> Id {
        self.source.anchor_node_id()
    }
}

/// Explicit generated box tree for one layout pass.
pub struct BoxTree<'style_tree, 'dom> {
    pub(super) root: BoxId,
    pub(super) nodes: Vec<BoxNode<'style_tree, 'dom>>,
}

impl fmt::Debug for BoxTree<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxTree")
            .field("root", &self.root)
            .field("nodes", &self.nodes)
            .finish()
    }
}

impl<'style_tree, 'dom> BoxTree<'style_tree, 'dom> {
    pub fn generate(
        root: &'style_tree StyledNode<'dom>,
        replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    ) -> Self {
        super::builder::generate_box_tree(root, replaced_info)
    }

    pub(super) fn new(root: BoxId, nodes: Vec<BoxNode<'style_tree, 'dom>>) -> Self {
        Self { root, nodes }
    }

    pub fn root_id(&self) -> BoxId {
        self.root
    }

    pub fn root(&self) -> &BoxNode<'style_tree, 'dom> {
        self.node(self.root)
    }

    pub fn node(&self, id: BoxId) -> &BoxNode<'style_tree, 'dom> {
        &self.nodes[id.index()]
    }

    pub fn nodes(&self) -> &[BoxNode<'style_tree, 'dom>] {
        &self.nodes
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
