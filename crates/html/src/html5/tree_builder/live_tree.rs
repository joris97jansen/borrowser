//! Internal structural mirror for the HTML5 tree builder.
//!
//! `LiveTree` is not the public typed-validation surface for parser hardening.
//! It mirrors already-authoritative structural edits emitted by the tree builder
//! and uses assertions intentionally: if a structural patch violates these
//! assumptions, that indicates an engine bug in tree-builder logic rather than a
//! recoverable malformed-HTML condition.
//!
//! Typed, reusable invariant validation for tests and fuzzing lives in
//! `tree_builder::invariants` via `check_dom_invariants` and
//! `check_patch_invariants`.

use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::tree_builder::invariants::{
    DomInvariantNode, DomInvariantNodeKind, DomInvariantState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LiveNodeKind {
    Document,
    Element,
    Text,
    Comment,
}

impl LiveNodeKind {
    #[inline]
    fn is_container(self) -> bool {
        matches!(self, Self::Document | Self::Element)
    }
}

#[derive(Clone, Debug)]
struct LiveNode {
    kind: LiveNodeKind,
    parent: Option<PatchKey>,
    children: Vec<PatchKey>,
}

impl LiveNode {
    #[inline]
    fn new(kind: LiveNodeKind) -> Self {
        Self {
            kind,
            parent: None,
            children: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::html5::tree_builder) struct LiveTree {
    nodes: Vec<Option<LiveNode>>,
    root: Option<PatchKey>,
}

impl LiveTree {
    pub(in crate::html5::tree_builder) fn apply_structural_patch(&mut self, patch: &DomPatch) {
        match patch {
            DomPatch::CreateDocument { key, .. } => self.insert_node(*key, LiveNodeKind::Document),
            DomPatch::CreateElement { key, .. } => self.insert_node(*key, LiveNodeKind::Element),
            DomPatch::CreateText { key, .. } => self.insert_node(*key, LiveNodeKind::Text),
            DomPatch::CreateComment { key, .. } => self.insert_node(*key, LiveNodeKind::Comment),
            DomPatch::AppendChild { parent, child } => self.append_child(*parent, *child),
            DomPatch::InsertBefore {
                parent,
                child,
                before,
            } => self.insert_before(*parent, *child, *before),
            DomPatch::RemoveNode { key } => self.remove_node(*key),
            DomPatch::Clear
            | DomPatch::SetAttributes { .. }
            | DomPatch::SetText { .. }
            | DomPatch::AppendText { .. } => {}
        }
    }

    pub(in crate::html5::tree_builder) fn parent(&self, key: PatchKey) -> Option<PatchKey> {
        self.node(key).parent
    }

    pub(in crate::html5::tree_builder) fn children_snapshot(&self, key: PatchKey) -> Vec<PatchKey> {
        self.node(key).children.clone()
    }

    pub(in crate::html5::tree_builder) fn invariant_state(&self) -> DomInvariantState {
        let mut state = DomInvariantState {
            nodes: vec![None; self.nodes.len()],
            root: self.root,
        };
        for (index, maybe_node) in self.nodes.iter().enumerate() {
            let Some(node) = maybe_node else {
                continue;
            };
            state.nodes[index] = Some(DomInvariantNode {
                kind: match node.kind {
                    LiveNodeKind::Document => DomInvariantNodeKind::Document,
                    LiveNodeKind::Element => DomInvariantNodeKind::Element,
                    LiveNodeKind::Text => DomInvariantNodeKind::Text,
                    LiveNodeKind::Comment => DomInvariantNodeKind::Comment,
                },
                parent: node.parent,
                children: node.children.clone(),
            });
        }
        state
    }

    fn insert_node(&mut self, key: PatchKey, kind: LiveNodeKind) {
        assert_ne!(key, PatchKey::INVALID, "live tree keys must be non-zero");
        self.ensure_slot(key);
        assert!(
            self.nodes[key.0 as usize].is_none(),
            "duplicate live tree key {key:?}"
        );
        if matches!(kind, LiveNodeKind::Document) {
            assert!(self.root.is_none(), "live tree document already exists");
            self.root = Some(key);
        }
        self.nodes[key.0 as usize] = Some(LiveNode::new(kind));
    }

    fn append_child(&mut self, parent: PatchKey, child: PatchKey) {
        assert_ne!(parent, child, "AppendChild cannot attach a node to itself");
        self.ensure_container(parent, "AppendChild parent");
        self.ensure_node(child, "AppendChild child");
        self.assert_move_allowed(child, "AppendChild child");
        self.assert_no_cycle(parent, child, "AppendChild");

        let already_last = self.parent(child) == Some(parent)
            && self
                .node(parent)
                .children
                .last()
                .copied()
                .is_some_and(|last| last == child);
        if already_last {
            return;
        }

        self.detach_child(child);
        self.node_mut(parent).children.push(child);
        self.node_mut(child).parent = Some(parent);
    }

    fn insert_before(&mut self, parent: PatchKey, child: PatchKey, before: PatchKey) {
        assert_ne!(parent, child, "InsertBefore cannot attach a node to itself");
        assert_ne!(
            child, before,
            "InsertBefore cannot insert a node before itself"
        );
        self.ensure_container(parent, "InsertBefore parent");
        self.ensure_node(child, "InsertBefore child");
        self.ensure_node(before, "InsertBefore before");
        self.assert_move_allowed(child, "InsertBefore child");
        self.assert_no_cycle(parent, child, "InsertBefore");
        assert_eq!(
            self.parent(before),
            Some(parent),
            "InsertBefore before child must already be attached to the requested parent"
        );

        let already_in_place = if self.parent(child) == Some(parent) {
            let siblings = &self.node(parent).children;
            let child_index = siblings.iter().position(|key| *key == child);
            let before_index = siblings.iter().position(|key| *key == before);
            matches!((child_index, before_index), (Some(child_index), Some(before_index)) if child_index + 1 == before_index)
        } else {
            false
        };
        if already_in_place {
            return;
        }

        self.detach_child(child);
        let before_index = self
            .node(parent)
            .children
            .iter()
            .position(|key| *key == before)
            .expect("InsertBefore before child must remain present");
        self.node_mut(parent).children.insert(before_index, child);
        self.node_mut(child).parent = Some(parent);
    }

    fn remove_node(&mut self, key: PatchKey) {
        self.ensure_node(key, "RemoveNode target");
        self.detach_child(key);

        let mut stack = vec![key];
        while let Some(current) = stack.pop() {
            let children = self.node(current).children.clone();
            for child in children {
                stack.push(child);
            }
            if self.root == Some(current) {
                self.root = None;
            }
            self.nodes[current.0 as usize] = None;
        }
    }

    fn ensure_slot(&mut self, key: PatchKey) {
        let index = key.0 as usize;
        if self.nodes.len() <= index {
            self.nodes.resize_with(index + 1, || None);
        }
    }

    fn ensure_node(&self, key: PatchKey, context: &str) {
        assert_ne!(key, PatchKey::INVALID, "{context}: invalid patch key");
        assert!(
            (key.0 as usize) < self.nodes.len() && self.nodes[key.0 as usize].is_some(),
            "{context}: missing node {key:?}"
        );
    }

    fn ensure_container(&self, key: PatchKey, context: &str) {
        self.ensure_node(key, context);
        assert!(
            self.node(key).kind.is_container(),
            "{context}: container required"
        );
    }

    fn assert_move_allowed(&self, key: PatchKey, context: &str) {
        let node = self.node(key);
        assert!(
            !matches!(node.kind, LiveNodeKind::Document),
            "{context}: cannot move a document node"
        );
        let is_root_element = matches!(node.kind, LiveNodeKind::Element)
            && self.root.is_some_and(|root| node.parent == Some(root));
        assert!(
            !is_root_element,
            "{context}: cannot move the document root element"
        );
    }

    fn assert_no_cycle(&self, parent: PatchKey, child: PatchKey, context: &str) {
        let mut cursor = Some(parent);
        while let Some(current) = cursor {
            assert_ne!(current, child, "{context}: cannot create an ancestor cycle");
            cursor = self.parent(current);
        }
    }

    fn detach_child(&mut self, child: PatchKey) {
        let parent = self.parent(child);
        if let Some(parent) = parent {
            let siblings = &self.node(parent).children;
            let mut first_index = None;
            let mut match_count = 0usize;
            for (index, existing) in siblings.iter().copied().enumerate() {
                if existing == child {
                    match_count += 1;
                    if first_index.is_none() {
                        first_index = Some(index);
                    }
                }
            }
            let index = first_index.expect("live tree detach requires child to appear in parent");
            assert_eq!(
                match_count, 1,
                "live tree invariant violated: sibling list contains duplicate child entries"
            );
            self.node_mut(parent).children.remove(index);
        }
        self.node_mut(child).parent = None;
    }

    fn node(&self, key: PatchKey) -> &LiveNode {
        self.nodes
            .get(key.0 as usize)
            .and_then(Option::as_ref)
            .expect("live tree node missing")
    }

    fn node_mut(&mut self, key: PatchKey) -> &mut LiveNode {
        self.nodes
            .get_mut(key.0 as usize)
            .and_then(Option::as_mut)
            .expect("live tree node missing")
    }
}

#[cfg(test)]
mod tests {
    use super::LiveTree;
    use crate::dom_patch::{DomPatch, PatchKey};

    fn create_document(tree: &mut LiveTree, key: u32) {
        tree.apply_structural_patch(&DomPatch::CreateDocument {
            key: PatchKey(key),
            doctype: None,
        });
    }

    fn create_element(tree: &mut LiveTree, key: u32, name: &'static str) {
        tree.apply_structural_patch(&DomPatch::CreateElement {
            key: PatchKey(key),
            name: std::sync::Arc::from(name),
            attributes: Vec::new(),
        });
    }

    fn create_text(tree: &mut LiveTree, key: u32, text: &'static str) {
        tree.apply_structural_patch(&DomPatch::CreateText {
            key: PatchKey(key),
            text: text.to_string(),
        });
    }

    #[test]
    fn live_tree_append_child_reparents_between_parents() {
        let mut tree = LiveTree::default();
        create_document(&mut tree, 1);
        create_element(&mut tree, 2, "div");
        create_element(&mut tree, 3, "p");
        create_element(&mut tree, 4, "span");

        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(3),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        });

        assert_eq!(tree.children_snapshot(PatchKey(2)), Vec::<PatchKey>::new());
        assert_eq!(tree.children_snapshot(PatchKey(3)), vec![PatchKey(4)]);
        assert_eq!(tree.parent(PatchKey(4)), Some(PatchKey(3)));
    }

    #[test]
    fn live_tree_insert_before_reorders_and_moves_across_parents() {
        let mut tree = LiveTree::default();
        create_document(&mut tree, 1);
        create_element(&mut tree, 2, "ul");
        create_element(&mut tree, 3, "li");
        create_element(&mut tree, 4, "li");
        create_element(&mut tree, 5, "li");
        create_element(&mut tree, 6, "ol");
        create_element(&mut tree, 7, "li");

        for child in [2, 6] {
            tree.apply_structural_patch(&DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(child),
            });
        }
        for child in [3, 4, 5] {
            tree.apply_structural_patch(&DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(child),
            });
        }
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(6),
            child: PatchKey(7),
        });
        tree.apply_structural_patch(&DomPatch::InsertBefore {
            parent: PatchKey(2),
            child: PatchKey(5),
            before: PatchKey(3),
        });
        tree.apply_structural_patch(&DomPatch::InsertBefore {
            parent: PatchKey(2),
            child: PatchKey(7),
            before: PatchKey(4),
        });

        assert_eq!(
            tree.children_snapshot(PatchKey(2)),
            vec![PatchKey(5), PatchKey(3), PatchKey(7), PatchKey(4)]
        );
        assert_eq!(tree.parent(PatchKey(5)), Some(PatchKey(2)));
        assert_eq!(tree.parent(PatchKey(7)), Some(PatchKey(2)));
    }

    #[test]
    fn live_tree_remove_node_removes_entire_subtree() {
        let mut tree = LiveTree::default();
        create_document(&mut tree, 1);
        create_element(&mut tree, 2, "div");
        create_element(&mut tree, 3, "span");
        create_text(&mut tree, 4, "x");

        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        });

        tree.apply_structural_patch(&DomPatch::RemoveNode { key: PatchKey(2) });

        assert_eq!(tree.children_snapshot(PatchKey(1)), Vec::<PatchKey>::new());
        let removed = std::panic::catch_unwind(|| tree.children_snapshot(PatchKey(2)));
        assert!(
            removed.is_err(),
            "removed subtree root should no longer exist"
        );
    }

    #[test]
    fn live_tree_rejects_ancestor_cycles() {
        let mut tree = LiveTree::default();
        create_document(&mut tree, 1);
        create_element(&mut tree, 2, "html");
        create_element(&mut tree, 3, "body");
        create_element(&mut tree, 4, "div");

        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        });

        let result = std::panic::catch_unwind(move || {
            tree.apply_structural_patch(&DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(3),
            });
        });
        assert!(result.is_err(), "cycle-creating moves must panic");
    }

    #[test]
    fn live_tree_rejects_document_root_element_moves() {
        let mut tree = LiveTree::default();
        create_document(&mut tree, 1);
        create_element(&mut tree, 2, "html");
        create_element(&mut tree, 3, "body");

        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        });

        let result = std::panic::catch_unwind(move || {
            tree.apply_structural_patch(&DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(2),
            });
        });
        assert!(result.is_err(), "document root element moves must panic");
    }

    #[test]
    fn live_tree_rejects_duplicate_sibling_entries_on_detach() {
        let mut tree = LiveTree::default();
        create_document(&mut tree, 1);
        create_element(&mut tree, 2, "div");
        create_element(&mut tree, 3, "p");
        create_element(&mut tree, 4, "span");

        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(3),
        });
        tree.apply_structural_patch(&DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        });

        tree.nodes[2] = Some(super::LiveNode {
            kind: super::LiveNodeKind::Element,
            parent: Some(PatchKey(1)),
            children: vec![PatchKey(4), PatchKey(4)],
        });
        tree.nodes[4]
            .as_mut()
            .expect("child should remain present")
            .parent = Some(PatchKey(2));

        let result = std::panic::catch_unwind(move || {
            tree.apply_structural_patch(&DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(4),
            });
        });
        assert!(
            result.is_err(),
            "duplicate sibling entries must be treated as a live-tree invariant violation"
        );
    }
}
