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
use crate::names::ElementNamespace;
use crate::types::ParserCreatedFragmentKind;
use crate::{ExpandedElementName, ParserCreatedAttribute};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LiveNodeKind {
    Document,
    DocumentType,
    Element,
    DocumentFragment(ParserCreatedFragmentKind),
    Text,
    Comment,
    ProcessingInstruction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum ChildInsertionReservationError {
    InvalidParent,
    InvalidBeforeSibling,
    InvalidChild,
    ArithmeticOverflow,
    AllocationFailure,
}

impl LiveNodeKind {
    #[inline]
    fn is_container(self) -> bool {
        matches!(
            self,
            Self::Document | Self::Element | Self::DocumentFragment(_)
        )
    }
}

#[derive(Clone, Debug)]
struct LiveNode {
    kind: LiveNodeKind,
    parent: Option<PatchKey>,
    children: Vec<PatchKey>,
    template_contents: Option<PatchKey>,
    fragment_host: Option<PatchKey>,
    is_template_element: bool,
    expanded_name: Option<ExpandedElementName>,
    attributes: Vec<ParserCreatedAttribute>,
    processing_instruction: Option<(String, String)>,
}

impl LiveNode {
    #[inline]
    fn new(kind: LiveNodeKind) -> Self {
        Self {
            kind,
            parent: None,
            children: Vec::new(),
            template_contents: None,
            fragment_host: None,
            is_template_element: false,
            expanded_name: None,
            attributes: Vec::new(),
            processing_instruction: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::html5::tree_builder) struct LiveTree {
    nodes: Vec<Option<LiveNode>>,
    root: Option<PatchKey>,
    #[cfg(test)]
    fail_next_child_reservation: Option<ChildInsertionReservationError>,
}

impl LiveTree {
    pub(in crate::html5::tree_builder) fn try_reserve_through_key(
        &mut self,
        key: PatchKey,
    ) -> Result<(), ()> {
        let required_len = (key.0 as usize).checked_add(1).ok_or(())?;
        if required_len > self.nodes.len() {
            self.nodes
                .try_reserve(required_len - self.nodes.len())
                .map_err(|_| ())?;
        }
        Ok(())
    }

    /// Reserve the exact parent child-list capacity required by a future
    /// insertion before a parser-owned structural transaction begins.
    ///
    /// `child` is `None` for a not-yet-created node. An existing child already
    /// attached to `parent` does not increase the final child count, including
    /// insert-before moves within the same parent.
    pub(in crate::html5::tree_builder) fn try_reserve_child_insertion(
        &mut self,
        parent: PatchKey,
        child: Option<PatchKey>,
        before: Option<PatchKey>,
    ) -> Result<(), ChildInsertionReservationError> {
        let parent_node = self
            .nodes
            .get(parent.0 as usize)
            .and_then(Option::as_ref)
            .filter(|node| node.kind.is_container())
            .ok_or(ChildInsertionReservationError::InvalidParent)?;
        if let Some(before) = before {
            let before_node = self
                .nodes
                .get(before.0 as usize)
                .and_then(Option::as_ref)
                .ok_or(ChildInsertionReservationError::InvalidBeforeSibling)?;
            if before_node.parent != Some(parent) {
                return Err(ChildInsertionReservationError::InvalidBeforeSibling);
            }
        }

        let additional = match child {
            Some(child) => {
                let child_node = self
                    .nodes
                    .get(child.0 as usize)
                    .and_then(Option::as_ref)
                    .ok_or(ChildInsertionReservationError::InvalidChild)?;
                usize::from(child_node.parent != Some(parent))
            }
            None => 1,
        };
        checked_child_insertion_len(parent_node.children.len(), additional)?;

        #[cfg(test)]
        if additional > 0
            && let Some(error) = self.fail_next_child_reservation.take()
        {
            return Err(error);
        }

        self.node_mut(parent)
            .children
            .try_reserve(additional)
            .map_err(|_| ChildInsertionReservationError::AllocationFailure)
    }

    #[cfg(test)]
    pub(in crate::html5::tree_builder) fn fail_next_child_reservation_for_test(
        &mut self,
        error: ChildInsertionReservationError,
    ) {
        self.fail_next_child_reservation = Some(error);
    }

    pub(in crate::html5::tree_builder) fn apply_structural_patch(&mut self, patch: &DomPatch) {
        match patch {
            DomPatch::CreateDocument { key, .. } => self.insert_node(*key, LiveNodeKind::Document),
            DomPatch::CreateDocumentType { key, .. } => {
                self.insert_node(*key, LiveNodeKind::DocumentType)
            }
            DomPatch::CreateElement {
                key,
                name,
                attributes,
            } => {
                self.insert_node(*key, LiveNodeKind::Element);
                self.node_mut(*key).is_template_element =
                    name.is(ElementNamespace::Html, "template");
                self.node_mut(*key).expanded_name = Some(name.clone());
                self.node_mut(*key).attributes.clone_from(attributes);
            }
            DomPatch::CreateTemplateContents { host, contents } => {
                self.create_template_contents(*host, *contents);
            }
            DomPatch::CreateText { key, .. } => self.insert_node(*key, LiveNodeKind::Text),
            DomPatch::CreateComment { key, .. } => self.insert_node(*key, LiveNodeKind::Comment),
            DomPatch::CreateProcessingInstruction { key, target, data } => {
                self.insert_node(*key, LiveNodeKind::ProcessingInstruction);
                self.node_mut(*key).processing_instruction = Some((target.clone(), data.clone()));
            }
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

    pub(in crate::html5::tree_builder) fn child_count(&self, key: PatchKey) -> usize {
        self.node(key).children.len()
    }

    pub(in crate::html5::tree_builder) fn template_contents(
        &self,
        host: PatchKey,
    ) -> Option<PatchKey> {
        self.node(host).template_contents
    }

    #[cfg(test)]
    pub(in crate::html5::tree_builder) fn corrupt_template_association_for_test(
        &mut self,
        host: PatchKey,
    ) {
        self.node_mut(host).template_contents = None;
    }

    pub(in crate::html5::tree_builder) fn is_template_element(&self, key: PatchKey) -> bool {
        self.node(key).is_template_element
    }

    pub(in crate::html5::tree_builder) fn element_semantics(
        &self,
        key: PatchKey,
    ) -> Option<(&ExpandedElementName, &[ParserCreatedAttribute])> {
        let node = self.node(key);
        node.expanded_name
            .as_ref()
            .map(|name| (name, node.attributes.as_slice()))
    }

    pub(in crate::html5::tree_builder) fn contains(&self, key: PatchKey) -> bool {
        key != PatchKey::INVALID && self.nodes.get(key.0 as usize).is_some_and(Option::is_some)
    }

    pub(in crate::html5::tree_builder) fn has_document_type_child(&self, key: PatchKey) -> bool {
        self.node(key)
            .children
            .iter()
            .any(|child| matches!(self.node(*child).kind, LiveNodeKind::DocumentType))
    }

    pub(in crate::html5::tree_builder) fn children_snapshot(&self, key: PatchKey) -> Vec<PatchKey> {
        self.node(key).children.clone()
    }

    #[cfg(any(test, feature = "html5-fuzzing", feature = "parser_invariants"))]
    pub(in crate::html5::tree_builder) fn template_hosts_for_full_audit(&self) -> Vec<PatchKey> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                node.as_ref()
                    .filter(|node| node.is_template_element)
                    .map(|_| PatchKey(index as u32))
            })
            .collect()
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
                    LiveNodeKind::DocumentType => DomInvariantNodeKind::DocumentType,
                    LiveNodeKind::Element => DomInvariantNodeKind::Element,
                    LiveNodeKind::DocumentFragment(kind) => {
                        DomInvariantNodeKind::DocumentFragment(kind)
                    }
                    LiveNodeKind::Text => DomInvariantNodeKind::Text,
                    LiveNodeKind::Comment => DomInvariantNodeKind::Comment,
                    LiveNodeKind::ProcessingInstruction => {
                        DomInvariantNodeKind::ProcessingInstruction
                    }
                },
                parent: node.parent,
                children: node.children.clone(),
                template_contents: node.template_contents,
                fragment_host: node.fragment_host,
                is_template_element: node.is_template_element,
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

    fn create_template_contents(&mut self, host: PatchKey, contents: PatchKey) {
        self.ensure_node(host, "CreateTemplateContents host");
        let host_node = self.node(host);
        assert_eq!(host_node.kind, LiveNodeKind::Element);
        assert!(host_node.is_template_element);
        assert!(host_node.template_contents.is_none());
        self.insert_node(
            contents,
            LiveNodeKind::DocumentFragment(ParserCreatedFragmentKind::TemplateContents),
        );
        self.node_mut(contents).fragment_host = Some(host);
        self.node_mut(host).template_contents = Some(contents);
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
        assert!(
            !matches!(self.node(key).kind, LiveNodeKind::DocumentFragment(_)),
            "hosted template contents roots cannot be removed directly"
        );
        self.detach_child(key);

        let mut stack = vec![key];
        while let Some(current) = stack.pop() {
            let node = self.node(current);
            let mut children = node.children.clone();
            if let Some(contents) = node.template_contents {
                children.push(contents);
            }
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
        assert!(
            !matches!(node.kind, LiveNodeKind::DocumentFragment(_)),
            "{context}: template contents roots cannot acquire ordinary parents"
        );
        let is_root_element = matches!(node.kind, LiveNodeKind::Element)
            && self.root.is_some_and(|root| node.parent == Some(root));
        assert!(
            !is_root_element,
            "{context}: cannot move the document root element"
        );
    }

    fn assert_no_cycle(&self, parent: PatchKey, child: PatchKey, context: &str) {
        let mut stack = vec![child];
        let mut visited = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            assert_ne!(
                current, parent,
                "{context}: cannot create an ancestor cycle"
            );
            let node = self.node(current);
            stack.extend(node.children.iter().copied());
            if let Some(contents) = node.template_contents {
                stack.push(contents);
            }
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

fn checked_child_insertion_len(
    len: usize,
    additional: usize,
) -> Result<usize, ChildInsertionReservationError> {
    len.checked_add(additional)
        .ok_or(ChildInsertionReservationError::ArithmeticOverflow)
}

#[cfg(test)]
mod reservation_tests {
    use super::{ChildInsertionReservationError, LiveTree, checked_child_insertion_len};
    use crate::{DomPatch, PatchKey};

    fn apply(tree: &mut LiveTree, patch: DomPatch) {
        tree.apply_structural_patch(&patch);
    }

    #[test]
    fn child_reservation_accounts_for_same_parent_insertions_and_fragment_parents() {
        let mut tree = LiveTree::default();
        for patch in [
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: crate::test_support::html_name("template"),
                attributes: Vec::new(),
            },
            DomPatch::CreateTemplateContents {
                host: PatchKey(2),
                contents: PatchKey(3),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: crate::test_support::html_name("div"),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: crate::test_support::html_name("span"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(4),
            },
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(5),
            },
        ] {
            apply(&mut tree, patch);
        }

        tree.fail_next_child_reservation_for_test(
            ChildInsertionReservationError::AllocationFailure,
        );
        tree.try_reserve_child_insertion(PatchKey(3), Some(PatchKey(5)), Some(PatchKey(4)))
            .expect("same-parent insert-before does not increase final child count");
        assert!(
            tree.try_reserve_child_insertion(PatchKey(3), None, Some(PatchKey(4)))
                .is_err(),
            "fresh insertion under a template fragment must consume the reservation seam"
        );

        assert_eq!(
            tree.try_reserve_child_insertion(PatchKey(99), None, None),
            Err(ChildInsertionReservationError::InvalidParent)
        );
        assert_eq!(
            tree.try_reserve_child_insertion(PatchKey(3), None, Some(PatchKey(2))),
            Err(ChildInsertionReservationError::InvalidBeforeSibling)
        );
        assert_eq!(
            tree.try_reserve_child_insertion(PatchKey(3), Some(PatchKey(99)), None),
            Err(ChildInsertionReservationError::InvalidChild)
        );
        assert_eq!(
            checked_child_insertion_len(usize::MAX, 1),
            Err(ChildInsertionReservationError::ArithmeticOverflow)
        );
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
            name: crate::test_support::html_name(name),
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
            template_contents: None,
            fragment_host: None,
            is_template_element: false,
            expanded_name: Some(crate::test_support::html_name("div")),
            attributes: Vec::new(),
            processing_instruction: None,
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
