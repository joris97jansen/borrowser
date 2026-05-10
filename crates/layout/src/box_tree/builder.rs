//! Styled-tree to generated box-tree construction.

use crate::{
    BoxKind, ListMarker, ReplacedElementInfoProvider, ReplacedKind, classify_replaced_kind,
};
use css::{Display, StyledNode};
use html::Node;

use super::display::{
    AnonymousBoxKind, BoxGenerationRole, DisplayBoxBehavior, DisplayBoxGeneration, PrincipalBox,
    display_box_generation, principal_participates_inline,
};
use super::formatting::{
    BlockFormattingParticipation, FormattingContextKind, InlineFormattingParticipation,
    principal_block_formatting_participation, principal_establishes_containing_block,
    principal_establishes_formatting_context, principal_establishes_inline_formatting_context,
    principal_inline_formatting_participation, principal_participates_in_inline_formatting_context,
};
use super::ids::{BoxId, ContainingBlockId, FormattingContextId, InlineFormattingContextId};
use super::model::{BoxNode, BoxTree};
use super::source::BoxSource;

pub(super) fn generate_box_tree<'style_tree, 'dom>(
    root: &'style_tree StyledNode<'dom>,
    replaced_info: Option<&dyn ReplacedElementInfoProvider>,
) -> BoxTree<'style_tree, 'dom> {
    let mut builder = BoxTreeBuilder { nodes: Vec::new() };
    let root = builder
        .build_styled_subtree(root, None, replaced_info)
        .unwrap_or_else(|| builder.push_fallback_root(root));

    BoxTree::new(root, builder.nodes)
}

struct BoxTreeBuilder<'style_tree, 'dom> {
    nodes: Vec<BoxNode<'style_tree, 'dom>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnonymousChildClass {
    InlineLevel,
    BlockLevel,
}

impl<'style_tree, 'dom> BoxTreeBuilder<'style_tree, 'dom> {
    fn build_styled_subtree(
        &mut self,
        styled: &'style_tree StyledNode<'dom>,
        parent: Option<BoxId>,
        replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    ) -> Option<BoxId> {
        let replaced_kind = classify_replaced_kind(styled.node);
        let generation =
            display_box_generation(styled, parent.map(|id| self.node(id).role()), replaced_kind);
        let principal = match generation {
            DisplayBoxGeneration::SuppressSubtree(_) => return None,
            DisplayBoxGeneration::GeneratePrincipalBox(principal) => principal,
        };
        let id = self.push_dom_backed_box(styled, parent, principal, replaced_kind, replaced_info);

        if matches!(styled.node, Node::Document { .. } | Node::Element { .. }) {
            self.build_generated_children(id, styled, replaced_info);
        }

        Some(id)
    }

    fn build_generated_children(
        &mut self,
        parent_id: BoxId,
        styled: &'style_tree StyledNode<'dom>,
        replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    ) {
        let (is_ul, is_ol) = list_container_kind(styled.node);
        let mut next_ol_index: u32 = 1;
        let wrap_inline_runs = self.requires_anonymous_block_wrapping(parent_id, styled);
        let mut current_anonymous_block: Option<BoxId> = None;

        for child in &styled.children {
            let child_class = if wrap_inline_runs {
                self.anonymous_child_class(child, parent_id)
            } else {
                None
            };

            let target_parent = match child_class {
                Some(AnonymousChildClass::InlineLevel) => *current_anonymous_block
                    .get_or_insert_with(|| self.push_anonymous_block_box(parent_id, styled)),
                Some(AnonymousChildClass::BlockLevel) => {
                    current_anonymous_block = None;
                    parent_id
                }
                None => parent_id,
            };

            let Some(child_id) =
                self.build_styled_subtree(child, Some(target_parent), replaced_info)
            else {
                continue;
            };

            if target_parent == parent_id
                && matches!(child.node, Node::Element { .. })
                && self.node(child_id).display_behavior() == DisplayBoxBehavior::ListItem
            {
                if is_ul {
                    self.node_mut(child_id).list_marker = Some(ListMarker::Unordered);
                } else if is_ol {
                    self.node_mut(child_id).list_marker = Some(ListMarker::Ordered(next_ol_index));
                    next_ol_index += 1;
                }
            }

            self.node_mut(target_parent).children.push(child_id);
        }
    }

    fn requires_anonymous_block_wrapping(
        &self,
        parent_id: BoxId,
        styled: &'style_tree StyledNode<'dom>,
    ) -> bool {
        if !supports_anonymous_block_children(self.node(parent_id)) {
            return false;
        }

        let mut has_inline_level = false;
        let mut has_block_level = false;

        for child in &styled.children {
            match self.anonymous_child_class(child, parent_id) {
                Some(AnonymousChildClass::InlineLevel) => has_inline_level = true,
                Some(AnonymousChildClass::BlockLevel) => has_block_level = true,
                None => {}
            }

            if has_inline_level && has_block_level {
                return true;
            }
        }

        false
    }

    fn anonymous_child_class(
        &self,
        child: &'style_tree StyledNode<'dom>,
        parent_id: BoxId,
    ) -> Option<AnonymousChildClass> {
        let replaced_kind = classify_replaced_kind(child.node);
        let generation =
            display_box_generation(child, Some(self.node(parent_id).role()), replaced_kind);
        let principal = match generation {
            DisplayBoxGeneration::SuppressSubtree(_) => return None,
            DisplayBoxGeneration::GeneratePrincipalBox(principal) => principal,
        };

        if principal_participates_inline(principal) {
            Some(AnonymousChildClass::InlineLevel)
        } else {
            Some(AnonymousChildClass::BlockLevel)
        }
    }

    fn push_dom_backed_box(
        &mut self,
        styled: &'style_tree StyledNode<'dom>,
        parent: Option<BoxId>,
        principal: PrincipalBox,
        replaced_kind: Option<ReplacedKind>,
        replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    ) -> BoxId {
        let role = principal.role();
        let kind = principal.kind();
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

        let id = BoxId(self.nodes.len());
        self.nodes.push(BoxNode {
            id,
            parent,
            children: Vec::new(),
            role,
            kind,
            source: BoxSource::DomNode(styled),
            style: &styled.style,
            display: styled.style.display(),
            display_behavior: principal.behavior(),
            containing_block: self.containing_block_for_child(parent),
            establishes_containing_block: principal_establishes_containing_block(principal),
            formatting_context: self.formatting_context_for_child(parent),
            establishes_formatting_context: principal_establishes_formatting_context(
                principal, styled,
            ),
            block_formatting_participation: principal_block_formatting_participation(principal),
            inline_formatting_context: self.inline_formatting_context_for_child(parent, principal),
            establishes_inline_formatting_context: principal_establishes_inline_formatting_context(
                principal, styled,
            ),
            inline_formatting_participation: principal_inline_formatting_participation(principal),
            list_marker: None,
            replaced,
            replaced_intrinsic,
        });
        id
    }

    fn push_anonymous_block_box(
        &mut self,
        parent: BoxId,
        parent_styled: &'style_tree StyledNode<'dom>,
    ) -> BoxId {
        let id = BoxId(self.nodes.len());
        self.nodes.push(BoxNode {
            id,
            parent: Some(parent),
            children: Vec::new(),
            role: BoxGenerationRole::Anonymous(AnonymousBoxKind::Block),
            kind: BoxKind::Block,
            source: BoxSource::Anonymous {
                parent: parent_styled,
                kind: AnonymousBoxKind::Block,
            },
            style: &parent_styled.style,
            display: Display::Block,
            display_behavior: DisplayBoxBehavior::Anonymous,
            containing_block: self.containing_block_for_child(Some(parent)),
            establishes_containing_block: true,
            formatting_context: self.formatting_context_for_child(Some(parent)),
            establishes_formatting_context: None,
            block_formatting_participation: BlockFormattingParticipation::BlockLevel,
            inline_formatting_context: None,
            establishes_inline_formatting_context: true,
            inline_formatting_participation: InlineFormattingParticipation::None,
            list_marker: None,
            replaced: None,
            replaced_intrinsic: None,
        });
        self.node_mut(parent).children.push(id);
        id
    }

    fn push_fallback_root(&mut self, styled: &'style_tree StyledNode<'dom>) -> BoxId {
        let id = BoxId(self.nodes.len());
        self.nodes.push(BoxNode {
            id,
            parent: None,
            children: Vec::new(),
            role: BoxGenerationRole::DocumentRoot,
            kind: BoxKind::Block,
            source: BoxSource::DomNode(styled),
            style: &styled.style,
            display: styled.style.display(),
            display_behavior: DisplayBoxBehavior::DocumentRoot,
            containing_block: None,
            establishes_containing_block: true,
            formatting_context: None,
            establishes_formatting_context: Some(FormattingContextKind::Block),
            block_formatting_participation: BlockFormattingParticipation::Root,
            inline_formatting_context: None,
            establishes_inline_formatting_context: false,
            inline_formatting_participation: InlineFormattingParticipation::None,
            list_marker: None,
            replaced: None,
            replaced_intrinsic: None,
        });
        id
    }

    fn node(&self, id: BoxId) -> &BoxNode<'style_tree, 'dom> {
        &self.nodes[id.index()]
    }

    fn node_mut(&mut self, id: BoxId) -> &mut BoxNode<'style_tree, 'dom> {
        &mut self.nodes[id.index()]
    }

    fn containing_block_for_child(&self, parent: Option<BoxId>) -> Option<ContainingBlockId> {
        let parent = parent?;
        let parent_node = self.node(parent);

        if parent_node.establishes_containing_block() {
            Some(ContainingBlockId(parent))
        } else {
            parent_node.containing_block()
        }
    }

    fn formatting_context_for_child(&self, parent: Option<BoxId>) -> Option<FormattingContextId> {
        let parent = parent?;
        let parent_node = self.node(parent);

        if parent_node.establishes_formatting_context().is_some() {
            Some(FormattingContextId(parent))
        } else {
            parent_node.formatting_context()
        }
    }

    fn inline_formatting_context_for_child(
        &self,
        parent: Option<BoxId>,
        principal: PrincipalBox,
    ) -> Option<InlineFormattingContextId> {
        if !principal_participates_in_inline_formatting_context(principal) {
            return None;
        }

        let parent = parent?;
        let parent_node = self.node(parent);

        if parent_node.establishes_inline_formatting_context() {
            Some(InlineFormattingContextId(parent))
        } else if parent_node.block_formatting_participation()
            == BlockFormattingParticipation::AtomicInline
        {
            None
        } else {
            parent_node.inline_formatting_context()
        }
    }
}

fn supports_anonymous_block_children(node: &BoxNode<'_, '_>) -> bool {
    matches!(node.kind(), BoxKind::Block)
        && matches!(
            node.display_behavior(),
            DisplayBoxBehavior::DocumentElement
                | DisplayBoxBehavior::Block
                | DisplayBoxBehavior::ListItem
                | DisplayBoxBehavior::Anonymous
        )
}

fn list_container_kind(node: &Node) -> (bool, bool) {
    match node {
        Node::Element { name, .. } => {
            let is_ul = name.eq_ignore_ascii_case("ul");
            let is_ol = name.eq_ignore_ascii_case("ol");
            (is_ul, is_ol)
        }
        _ => (false, false),
    }
}
