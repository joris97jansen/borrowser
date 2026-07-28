use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, ParserResourceLimit};
use crate::html5::tree_builder::Html5TreeBuilder;

const LIMIT_DIAGNOSTIC_SOE_DEPTH: &str = "resource-limit-soe-depth";
const LIMIT_DIAGNOSTIC_NODE_COUNT: &str = "resource-limit-node-count";
const LIMIT_DIAGNOSTIC_CHILDREN_PER_NODE: &str = "resource-limit-children-per-node";

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn allow_non_self_closing_element(
        &mut self,
        _name: AtomId,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if self.open_elements.len() < self.config.limits.max_open_elements_depth {
            return true;
        }
        self.record_tree_resource_limit(
            context,
            ParserResourceLimit::TreeOpenElementsDepth,
            self.config.limits.max_open_elements_depth,
            Some(LIMIT_DIAGNOSTIC_SOE_DEPTH),
        );
        false
    }

    pub(in crate::html5::tree_builder) fn allow_node_creation(
        &mut self,
        _tag: Option<AtomId>,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if self.non_document_nodes_created < self.config.limits.max_nodes_created {
            return true;
        }
        self.record_tree_resource_limit(
            context,
            ParserResourceLimit::TreeNodeCount,
            self.config.limits.max_nodes_created,
            Some(LIMIT_DIAGNOSTIC_NODE_COUNT),
        );
        false
    }

    pub(in crate::html5::tree_builder) fn allow_node_creation_count(
        &mut self,
        count: usize,
        _tag: Option<AtomId>,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if self
            .non_document_nodes_created
            .checked_add(count)
            .is_some_and(|total| total <= self.config.limits.max_nodes_created)
        {
            return true;
        }
        self.record_tree_resource_limit(
            context,
            ParserResourceLimit::TreeNodeCount,
            self.config.limits.max_nodes_created,
            Some(LIMIT_DIAGNOSTIC_NODE_COUNT),
        );
        false
    }

    pub(in crate::html5::tree_builder) fn note_node_created(&mut self) {
        self.non_document_nodes_created = self.non_document_nodes_created.saturating_add(1);
    }

    pub(in crate::html5::tree_builder) fn allow_new_child(
        &mut self,
        parent: PatchKey,
        _tag: Option<AtomId>,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if self.live_tree.child_count(parent) < self.config.limits.max_children_per_node {
            return true;
        }
        self.record_tree_resource_limit(
            context,
            ParserResourceLimit::TreeChildrenPerNode,
            self.config.limits.max_children_per_node,
            Some(LIMIT_DIAGNOSTIC_CHILDREN_PER_NODE),
        );
        false
    }

    pub(in crate::html5::tree_builder) fn allow_existing_child_insertion(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        tag: Option<AtomId>,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if self.live_tree.parent(child) == Some(parent) {
            return true;
        }
        self.allow_new_child(parent, tag, context)
    }
}
