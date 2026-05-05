use crate::{BoxNode, BoxTree};
use css::{ComputedStyle, Length};
use html::{Node, internal::Id};
use std::sync::Arc;

use super::super::{
    BoxGenerationRole, BoxId, ContainingBlockId, FormattingContextId, InlineFormattingContextId,
};

pub(super) struct TestMeasurer;

impl crate::TextMeasurer for TestMeasurer {
    fn measure(&self, text: &str, style: &ComputedStyle) -> f32 {
        let Length::Px(font_px) = style.font_size();
        text.chars().count() as f32 * font_px * 0.5
    }

    fn line_height(&self, style: &ComputedStyle) -> f32 {
        let Length::Px(font_px) = style.font_size();
        font_px * 1.2
    }
}

pub(super) fn element(id: u32, name: &str, style: Vec<(&str, &str)>, children: Vec<Node>) -> Node {
    Node::Element {
        id: Id(id),
        name: Arc::from(name),
        attributes: Vec::new(),
        style: style
            .into_iter()
            .map(|(property, value)| (property.to_string(), value.to_string()))
            .collect(),
        children,
    }
}

pub(super) fn text(id: u32, value: &str) -> Node {
    Node::Text {
        id: Id(id),
        text: value.to_string(),
    }
}

pub(super) fn comment(id: u32, value: &str) -> Node {
    Node::Comment {
        id: Id(id),
        text: value.to_string(),
    }
}

pub(super) fn doc(children: Vec<Node>) -> Node {
    Node::Document {
        id: Id(1),
        doctype: None,
        children,
    }
}

pub(super) fn source_ids(tree: &BoxTree<'_, '_>) -> Vec<Option<Id>> {
    tree.nodes()
        .iter()
        .map(|node| node.direct_node_id())
        .collect()
}

pub(super) fn box_by_node_id<'tree, 'style_tree, 'dom>(
    tree: &'tree BoxTree<'style_tree, 'dom>,
    id: Id,
) -> &'tree BoxNode<'style_tree, 'dom> {
    tree.nodes()
        .iter()
        .find(|node| node.direct_node_id() == Some(id))
        .unwrap_or_else(|| panic!("expected box for node id {id:?}"))
}

pub(super) fn containing_block_box_id(node: &BoxNode<'_, '_>) -> Option<BoxId> {
    node.containing_block().map(ContainingBlockId::box_id)
}

pub(super) fn formatting_context_box_id(node: &BoxNode<'_, '_>) -> Option<BoxId> {
    node.formatting_context().map(FormattingContextId::box_id)
}

pub(super) fn inline_formatting_context_box_id(node: &BoxNode<'_, '_>) -> Option<BoxId> {
    node.inline_formatting_context()
        .map(InlineFormattingContextId::box_id)
}

pub(super) fn anonymous_boxes<'tree, 'style_tree, 'dom>(
    tree: &'tree BoxTree<'style_tree, 'dom>,
) -> Vec<&'tree BoxNode<'style_tree, 'dom>> {
    tree.nodes()
        .iter()
        .filter(|node| matches!(node.role(), BoxGenerationRole::Anonymous(_)))
        .collect()
}

pub(super) fn find_layout_by_direct_node_id<'layout, 'style_tree, 'dom>(
    layout: &'layout crate::LayoutBox<'style_tree, 'dom>,
    id: Id,
) -> Option<&'layout crate::LayoutBox<'style_tree, 'dom>> {
    if layout.direct_node_id() == Some(id) {
        return Some(layout);
    }

    layout
        .children
        .iter()
        .find_map(|child| find_layout_by_direct_node_id(child, id))
}
