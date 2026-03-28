use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;
use crate::html5::tree_builder::Html5TreeBuilder;

const LIMIT_PARSE_ERROR_SOE_DEPTH: &str = "resource-limit-soe-depth";
const LIMIT_PARSE_ERROR_NODE_COUNT: &str = "resource-limit-node-count";
const LIMIT_PARSE_ERROR_CHILDREN_PER_NODE: &str = "resource-limit-children-per-node";

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn allow_non_self_closing_element(
        &mut self,
        name: AtomId,
    ) -> bool {
        if self.open_elements.len() < self.config.limits.max_open_elements_depth {
            return true;
        }
        self.record_parse_error(
            LIMIT_PARSE_ERROR_SOE_DEPTH,
            Some(name),
            Some(self.insertion_mode),
        );
        false
    }

    pub(in crate::html5::tree_builder) fn allow_node_creation(
        &mut self,
        tag: Option<AtomId>,
    ) -> bool {
        if self.non_document_nodes_created < self.config.limits.max_nodes_created {
            return true;
        }
        self.record_parse_error(LIMIT_PARSE_ERROR_NODE_COUNT, tag, Some(self.insertion_mode));
        false
    }

    pub(in crate::html5::tree_builder) fn note_node_created(&mut self) {
        self.non_document_nodes_created = self.non_document_nodes_created.saturating_add(1);
    }

    pub(in crate::html5::tree_builder) fn allow_new_child(
        &mut self,
        parent: PatchKey,
        tag: Option<AtomId>,
    ) -> bool {
        if self.live_tree.child_count(parent) < self.config.limits.max_children_per_node {
            return true;
        }
        self.record_parse_error(
            LIMIT_PARSE_ERROR_CHILDREN_PER_NODE,
            tag,
            Some(self.insertion_mode),
        );
        false
    }

    pub(in crate::html5::tree_builder) fn allow_existing_child_insertion(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        tag: Option<AtomId>,
    ) -> bool {
        if self.live_tree.parent(child) == Some(parent) {
            return true;
        }
        self.allow_new_child(parent, tag)
    }
}
