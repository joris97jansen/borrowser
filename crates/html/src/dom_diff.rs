//! Deterministic DOM diffing to patch streams (Stage 1 baseline).
//!
//! Contract:
//! - Nodes are matched by stable `Id` values (see `types::Id`).
//! - Output ordering is deterministic (pre-order traversal).
//! - Child lists are append-only; reorders or mid-list inserts trigger a reset.
//! - Attribute order is preserved; changes emit `SetAttributes`.
//! - Text updates emit `SetText`; comment/doctype changes trigger a reset.
//! - Resets are encoded as `DomPatch::Clear` + full create stream.
//! - Stage 1 uses `PatchKey == Id` to avoid a separate mapping layer.
//!   This coupling may change once patch transport stabilizes.
//! - Patch batches are ordered as: removals first, then updates/creates in pre-order.
//!
//! Complexity: O(n) in the number of nodes for both trees, plus set/map storage.

use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::{Id, Node};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug)]
pub enum DomDiffError {
    InvalidKey(Id),
    InvalidRoot(&'static str),
}

pub fn diff_dom(prev: &Node, next: &Node) -> Result<Vec<DomPatch>, DomDiffError> {
    if !root_is_compatible(prev, next) {
        return reset_stream(next);
    }

    let mut prev_map = HashMap::new();
    build_prev_map(prev, &mut prev_map);
    let mut next_ids = HashSet::new();
    collect_ids(next, &mut next_ids);

    let mut patches = Vec::new();
    emit_removals(prev, &next_ids, &mut patches)?;

    let mut need_reset = false;
    emit_updates(
        next,
        None,
        &prev_map,
        &next_ids,
        &mut patches,
        &mut need_reset,
    )?;

    if need_reset {
        return reset_stream(next);
    }

    Ok(patches)
}

fn reset_stream(next: &Node) -> Result<Vec<DomPatch>, DomDiffError> {
    let mut patches = vec![DomPatch::Clear];
    emit_create_subtree(next, None, &mut patches)?;
    Ok(patches)
}

#[derive(Clone, Debug)]
enum PrevNodeInfo {
    Document {
        doctype: Option<String>,
        children: Vec<Id>,
    },
    Element {
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
        children: Vec<Id>,
    },
    Text {
        text: String,
    },
    Comment {
        text: String,
    },
}

fn build_prev_map(node: &Node, map: &mut HashMap<Id, PrevNodeInfo>) {
    match node {
        Node::Document {
            id,
            doctype,
            children,
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Document {
                    doctype: doctype.clone(),
                    children: children.iter().map(Node::id).collect(),
                },
            );
            for child in children {
                build_prev_map(child, map);
            }
        }
        Node::Element {
            id,
            name,
            attributes,
            children,
            ..
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Element {
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                    children: children.iter().map(Node::id).collect(),
                },
            );
            for child in children {
                build_prev_map(child, map);
            }
        }
        Node::Text { id, text } => {
            map.insert(*id, PrevNodeInfo::Text { text: text.clone() });
        }
        Node::Comment { id, text } => {
            map.insert(*id, PrevNodeInfo::Comment { text: text.clone() });
        }
    }
}

fn collect_ids(node: &Node, out: &mut HashSet<Id>) {
    out.insert(node.id());
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                collect_ids(child, out);
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn emit_removals(
    node: &Node,
    next_ids: &HashSet<Id>,
    patches: &mut Vec<DomPatch>,
) -> Result<(), DomDiffError> {
    if !next_ids.contains(&node.id()) {
        patches.push(DomPatch::RemoveNode {
            key: patch_key(node.id())?,
        });
        return Ok(());
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_removals(child, next_ids, patches)?;
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
    Ok(())
}

fn emit_updates(
    node: &Node,
    parent_key: Option<PatchKey>,
    prev_map: &HashMap<Id, PrevNodeInfo>,
    next_ids: &HashSet<Id>,
    patches: &mut Vec<DomPatch>,
    need_reset: &mut bool,
) -> Result<(), DomDiffError> {
    let id = node.id();
    let key = patch_key(id)?;
    let is_new = !prev_map.contains_key(&id);

    if is_new {
        emit_create_node(node, key, patches)?;
        if let Some(parent) = parent_key {
            patches.push(DomPatch::AppendChild { parent, child: key });
        } else if !matches!(node, Node::Document { .. }) {
            return Err(DomDiffError::InvalidRoot("root must be Document"));
        }
    } else if let Some(prev) = prev_map.get(&id) {
        match (prev, node) {
            (
                PrevNodeInfo::Document { doctype, .. },
                Node::Document {
                    doctype: next_doctype,
                    ..
                },
            ) => {
                if doctype != next_doctype {
                    *need_reset = true;
                    return Ok(());
                }
            }
            (
                PrevNodeInfo::Element {
                    name, attributes, ..
                },
                Node::Element {
                    name: next_name,
                    attributes: next_attrs,
                    ..
                },
            ) => {
                if name != next_name {
                    *need_reset = true;
                    return Ok(());
                }
                if attributes != next_attrs {
                    patches.push(DomPatch::SetAttributes {
                        key,
                        attributes: next_attrs.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Text { text },
                Node::Text {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    patches.push(DomPatch::SetText {
                        key,
                        text: next_text.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Comment { text },
                Node::Comment {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    *need_reset = true;
                    return Ok(());
                }
            }
            _ => {
                *need_reset = true;
                return Ok(());
            }
        }
    }

    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            if !is_new {
                let prev_children_live = match prev_map.get(&id) {
                    Some(PrevNodeInfo::Document { children, .. })
                    | Some(PrevNodeInfo::Element { children, .. }) => children
                        .iter()
                        .copied()
                        .filter(|child| next_ids.contains(child))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                let next_children = children.iter().map(Node::id).collect::<Vec<_>>();
                if next_children.len() < prev_children_live.len() {
                    *need_reset = true;
                    return Ok(());
                }
                if next_children[..prev_children_live.len()] != prev_children_live[..] {
                    *need_reset = true;
                    return Ok(());
                }
            }
            for child in children {
                emit_updates(child, Some(key), prev_map, next_ids, patches, need_reset)?;
                if *need_reset {
                    return Ok(());
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
    Ok(())
}

fn emit_create_node(
    node: &Node,
    key: PatchKey,
    patches: &mut Vec<DomPatch>,
) -> Result<(), DomDiffError> {
    match node {
        Node::Document { doctype, .. } => {
            patches.push(DomPatch::CreateDocument {
                key,
                doctype: doctype.clone(),
            });
        }
        Node::Element {
            name, attributes, ..
        } => {
            patches.push(DomPatch::CreateElement {
                key,
                name: Arc::clone(name),
                attributes: attributes.clone(),
            });
        }
        Node::Text { text, .. } => {
            patches.push(DomPatch::CreateText {
                key,
                text: text.clone(),
            });
        }
        Node::Comment { text, .. } => {
            patches.push(DomPatch::CreateComment {
                key,
                text: text.clone(),
            });
        }
    }
    Ok(())
}

fn emit_create_subtree(
    node: &Node,
    parent_key: Option<PatchKey>,
    patches: &mut Vec<DomPatch>,
) -> Result<(), DomDiffError> {
    let key = patch_key(node.id())?;
    emit_create_node(node, key, patches)?;
    if let Some(parent) = parent_key {
        patches.push(DomPatch::AppendChild { parent, child: key });
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_create_subtree(child, Some(key), patches)?;
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
    Ok(())
}

fn patch_key(id: Id) -> Result<PatchKey, DomDiffError> {
    if id == Id::INVALID {
        return Err(DomDiffError::InvalidKey(id));
    }
    Ok(PatchKey(id.0))
}

fn root_is_compatible(prev: &Node, next: &Node) -> bool {
    match (prev, next) {
        (Node::Document { .. }, Node::Document { .. }) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
    use crate::golden_corpus::fixtures;
    use crate::{build_dom, tokenize};

    fn build(input: &str) -> Node {
        let stream = tokenize(input);
        build_dom(&stream)
    }

    #[derive(Default)]
    struct TestArena {
        nodes: HashMap<PatchKey, TestNode>,
        allocated: HashSet<PatchKey>,
        root: Option<PatchKey>,
    }

    #[derive(Clone)]
    struct TestNode {
        kind: TestKind,
        parent: Option<PatchKey>,
        children: Vec<PatchKey>,
    }

    #[derive(Clone)]
    enum TestKind {
        Document {
            doctype: Option<String>,
        },
        Element {
            name: Arc<str>,
            attributes: Vec<(Arc<str>, Option<String>)>,
        },
        Text {
            text: String,
        },
        Comment {
            text: String,
        },
    }

    impl TestArena {
        fn from_dom(root: &Node) -> Result<Self, DomDiffError> {
            let mut arena = Self::default();
            arena.insert_from_dom(root, None)?;
            Ok(arena)
        }

        fn insert_from_dom(
            &mut self,
            node: &Node,
            parent: Option<PatchKey>,
        ) -> Result<(), DomDiffError> {
            let key = patch_key(node.id())?;
            let kind = match node {
                Node::Document { doctype, .. } => TestKind::Document {
                    doctype: doctype.clone(),
                },
                Node::Element {
                    name, attributes, ..
                } => TestKind::Element {
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                },
                Node::Text { text, .. } => TestKind::Text { text: text.clone() },
                Node::Comment { text, .. } => TestKind::Comment { text: text.clone() },
            };
            if self.nodes.contains_key(&key) {
                return Err(DomDiffError::InvalidKey(node.id()));
            }
            if self.allocated.contains(&key) {
                return Err(DomDiffError::InvalidKey(node.id()));
            }
            self.nodes.insert(
                key,
                TestNode {
                    kind,
                    parent,
                    children: Vec::new(),
                },
            );
            self.allocated.insert(key);
            if parent.is_none() {
                self.root = Some(key);
            }
            match node {
                Node::Document { children, .. } | Node::Element { children, .. } => {
                    for child in children {
                        self.insert_from_dom(child, Some(key))?;
                        if let Some(entry) = self.nodes.get_mut(&key) {
                            entry.children.push(patch_key(child.id())?);
                        }
                    }
                }
                Node::Text { .. } | Node::Comment { .. } => {}
            }
            Ok(())
        }

        fn apply(&mut self, patches: &[DomPatch]) -> Result<(), DomDiffError> {
            for patch in patches {
                match patch {
                    DomPatch::Clear => {
                        self.nodes.clear();
                        self.allocated.clear();
                        self.root = None;
                    }
                    DomPatch::CreateDocument { key, doctype } => {
                        if self.root.is_some() {
                            return Err(DomDiffError::InvalidRoot("root already set"));
                        }
                        if self.allocated.contains(key) {
                            return Err(DomDiffError::InvalidRoot("duplicate key"));
                        }
                        self.nodes.insert(
                            *key,
                            TestNode {
                                kind: TestKind::Document {
                                    doctype: doctype.clone(),
                                },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                        self.allocated.insert(*key);
                        self.root = Some(*key);
                    }
                    DomPatch::CreateElement {
                        key,
                        name,
                        attributes,
                    } => {
                        if self.allocated.contains(key) {
                            return Err(DomDiffError::InvalidRoot("duplicate key"));
                        }
                        self.nodes.insert(
                            *key,
                            TestNode {
                                kind: TestKind::Element {
                                    name: Arc::clone(name),
                                    attributes: attributes.clone(),
                                },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                        self.allocated.insert(*key);
                    }
                    DomPatch::CreateText { key, text } => {
                        if self.allocated.contains(key) {
                            return Err(DomDiffError::InvalidRoot("duplicate key"));
                        }
                        self.nodes.insert(
                            *key,
                            TestNode {
                                kind: TestKind::Text { text: text.clone() },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                        self.allocated.insert(*key);
                    }
                    DomPatch::CreateComment { key, text } => {
                        if self.allocated.contains(key) {
                            return Err(DomDiffError::InvalidRoot("duplicate key"));
                        }
                        self.nodes.insert(
                            *key,
                            TestNode {
                                kind: TestKind::Comment { text: text.clone() },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                        self.allocated.insert(*key);
                    }
                    DomPatch::AppendChild { parent, child } => {
                        let Some(parent_node) = self.nodes.get_mut(parent) else {
                            return Err(DomDiffError::InvalidRoot("missing parent"));
                        };
                        let Some(child_node) = self.nodes.get_mut(child) else {
                            return Err(DomDiffError::InvalidRoot("missing child"));
                        };
                        if child_node.parent.is_some() {
                            return Err(DomDiffError::InvalidRoot("child already has parent"));
                        }
                        parent_node.children.push(*child);
                        child_node.parent = Some(*parent);
                    }
                    DomPatch::InsertBefore {
                        parent,
                        child,
                        before,
                    } => {
                        let Some(parent_node) = self.nodes.get_mut(parent) else {
                            return Err(DomDiffError::InvalidRoot("missing parent"));
                        };
                        let Some(child_node) = self.nodes.get_mut(child) else {
                            return Err(DomDiffError::InvalidRoot("missing child"));
                        };
                        if child_node.parent.is_some() {
                            return Err(DomDiffError::InvalidRoot("child already has parent"));
                        }
                        let pos = parent_node
                            .children
                            .iter()
                            .position(|k| k == before)
                            .ok_or(DomDiffError::InvalidRoot("missing before"))?;
                        parent_node.children.insert(pos, *child);
                        child_node.parent = Some(*parent);
                    }
                    DomPatch::RemoveNode { key } => {
                        self.remove_subtree(*key);
                    }
                    DomPatch::SetAttributes { key, attributes } => {
                        let Some(node) = self.nodes.get_mut(key) else {
                            return Err(DomDiffError::InvalidRoot("missing node"));
                        };
                        if let TestKind::Element {
                            attributes: attrs, ..
                        } = &mut node.kind
                        {
                            *attrs = attributes.clone();
                        }
                    }
                    DomPatch::SetText { key, text } => {
                        let Some(node) = self.nodes.get_mut(key) else {
                            return Err(DomDiffError::InvalidRoot("missing node"));
                        };
                        match &mut node.kind {
                            TestKind::Text { text: existing } => *existing = text.clone(),
                            TestKind::Comment { .. }
                            | TestKind::Document { .. }
                            | TestKind::Element { .. } => {}
                        }
                    }
                }
            }
            Ok(())
        }

        fn remove_subtree(&mut self, key: PatchKey) {
            let Some(node) = self.nodes.remove(&key) else {
                return;
            };
            if let Some(parent) = node.parent {
                if let Some(parent_node) = self.nodes.get_mut(&parent) {
                    parent_node.children.retain(|k| *k != key);
                }
            } else if self.root == Some(key) {
                self.root = None;
            }
            for child in node.children {
                self.remove_subtree(child);
            }
        }

        fn materialize(&self) -> Result<Node, DomDiffError> {
            let root = self.root.ok_or(DomDiffError::InvalidRoot("missing root"))?;
            self.materialize_node(root)
        }

        fn materialize_node(&self, key: PatchKey) -> Result<Node, DomDiffError> {
            let Some(node) = self.nodes.get(&key) else {
                return Err(DomDiffError::InvalidRoot("missing node"));
            };
            let children = node
                .children
                .iter()
                .map(|child| self.materialize_node(*child))
                .collect::<Result<Vec<_>, _>>()?;
            let id = Id::INVALID;
            let result = match &node.kind {
                TestKind::Document { doctype } => Node::Document {
                    id,
                    doctype: doctype.clone(),
                    children,
                },
                TestKind::Element { name, attributes } => Node::Element {
                    id,
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                    style: Vec::new(),
                    children,
                },
                TestKind::Text { text } => Node::Text {
                    id,
                    text: text.clone(),
                },
                TestKind::Comment { text } => Node::Comment {
                    id,
                    text: text.clone(),
                },
            };
            Ok(result)
        }
    }

    #[test]
    fn diff_roundtrip_golden_fixtures() {
        let base = build("");
        let opts = DomSnapshotOptions {
            ignore_ids: true,
            ignore_empty_style: true,
        };
        for fixture in fixtures() {
            let next = build(fixture.input);
            let patches = diff_dom(&base, &next).expect("diff failed");
            let mut arena = TestArena::from_dom(&base).expect("arena init failed");
            arena.apply(&patches).expect("apply failed");
            let materialized = arena.materialize().expect("materialize failed");
            assert_dom_eq(&next, &materialized, opts);
        }
    }

    #[test]
    fn diff_is_deterministic() {
        let prev = build("<div><span>hi</span></div>");
        let next = build("<div><span>hi</span><em>ok</em></div>");
        let a = diff_dom(&prev, &next).expect("diff a failed");
        let b = diff_dom(&prev, &next).expect("diff b failed");
        assert_eq!(a, b, "expected deterministic patch output");
    }

    #[test]
    fn diff_triggers_reset_on_midlist_insert() {
        let prev = build("<div><span>hi</span></div>");
        let next = build("<div><em>yo</em><span>hi</span></div>");
        let patches = diff_dom(&prev, &next).expect("diff failed");
        assert!(matches!(patches.first(), Some(DomPatch::Clear)));
    }

    #[test]
    fn diff_reset_clears_allocation_state() {
        let prev = build("<div><span>hi</span></div>");
        let next = build("<div><em>yo</em><span>hi</span></div>");
        let mut arena = TestArena::from_dom(&prev).expect("arena init failed");
        let patches = diff_dom(&prev, &next).expect("diff failed");
        arena.apply(&patches).expect("apply failed");
        let materialized = arena.materialize().expect("materialize failed");
        assert_dom_eq(
            &next,
            &materialized,
            DomSnapshotOptions {
                ignore_ids: true,
                ignore_empty_style: true,
            },
        );
    }
}
