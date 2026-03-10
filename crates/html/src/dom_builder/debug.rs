use crate::dom_patch::DomPatch;
use crate::types::NodeKey;

use super::TreeBuilder;
use super::arena::ArenaNode;
use super::patches::patch_has_invalid_key;

#[cfg(feature = "debug-stats")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DebugArenaStats {
    pub nodes: usize,
    pub edges: usize,
    pub text_bytes: usize,
}

impl TreeBuilder {
    #[cfg(test)]
    pub(crate) fn debug_root_key(&self) -> NodeKey {
        self.root_key
    }

    #[cfg(test)]
    pub(crate) fn debug_next_key(&self) -> NodeKey {
        NodeKey(self.arena.next_key)
    }

    #[cfg(test)]
    pub(crate) fn debug_node_count(&self) -> u32 {
        self.arena.nodes.len() as u32
    }

    #[cfg(feature = "debug-stats")]
    pub(crate) fn debug_arena_stats(&self) -> DebugArenaStats {
        self.arena.debug_stats()
    }

    #[cfg(any(test, debug_assertions))]
    pub(super) fn debug_assert_invariants(&self) {
        debug_assert!(
            self.root_index < self.arena.nodes.len(),
            "root index must be within arena bounds"
        );
        debug_assert!(
            matches!(
                self.arena.nodes[self.root_index],
                ArenaNode::Document { .. }
            ),
            "root node must be a document"
        );
        debug_assert_ne!(self.root_key, NodeKey::INVALID, "root key must be valid");
        if let ArenaNode::Document { key, .. } = self.arena.nodes[self.root_index] {
            debug_assert_eq!(
                key, self.root_key,
                "root key must match the document node key"
            );
        }
        debug_assert!(
            !self.open_elements.contains(&self.root_index),
            "open elements must not include the document node"
        );
        debug_assert!(
            self.open_elements
                .iter()
                .all(|&idx| idx < self.arena.nodes.len()),
            "open element indices must be within arena bounds"
        );
        debug_assert!(
            self.open_elements
                .iter()
                .all(|&idx| matches!(self.arena.nodes[idx], ArenaNode::Element { .. })),
            "open elements must only contain element nodes"
        );
        if let Some(pending) = &self.pending_text {
            debug_assert!(
                pending.parent_index < self.arena.nodes.len(),
                "pending text parent must be within arena bounds"
            );
            debug_assert!(
                pending.node_index < self.arena.nodes.len(),
                "pending text node must be within arena bounds"
            );
            debug_assert!(
                matches!(self.arena.nodes[pending.node_index], ArenaNode::Text { .. }),
                "pending text node must be a text node"
            );
            debug_assert_ne!(
                pending.key,
                NodeKey::INVALID,
                "pending text key must be valid"
            );
            debug_assert_eq!(
                self.arena.node_key(pending.node_index),
                pending.key,
                "pending text key must match arena node key"
            );
        }
        self.arena.debug_validate();
    }
}

#[cfg(any(test, debug_assertions))]
#[allow(dead_code, reason = "kept close to patch invariants for debugging")]
fn _debug_patch_sanity(patch: &DomPatch) {
    debug_assert!(
        !patch_has_invalid_key(patch),
        "patch emission must not use invalid keys"
    );
}
