use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::{Id, Node};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct TestPatchArena {
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

impl TestPatchArena {
    pub(crate) fn from_dom(root: &Node) -> Result<Self, String> {
        let mut arena = Self::default();
        arena.insert_from_dom(root, None)?;
        Ok(arena)
    }

    fn insert_from_dom(&mut self, node: &Node, parent: Option<PatchKey>) -> Result<(), String> {
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
        if self.nodes.contains_key(&key) || self.allocated.contains(&key) {
            return Err("duplicate key".to_string());
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

    pub(crate) fn apply(&mut self, patches: &[DomPatch]) -> Result<(), String> {
        let mut staged = self.clone();
        staged.apply_in_place(patches)?;
        staged.assert_invariants()?;
        *self = staged;
        Ok(())
    }

    fn apply_in_place(&mut self, patches: &[DomPatch]) -> Result<(), String> {
        for patch in patches {
            match patch {
                DomPatch::Clear => {
                    self.nodes.clear();
                    self.allocated.clear();
                    self.root = None;
                }
                DomPatch::CreateDocument { key, doctype } => {
                    if self.root.is_some() {
                        return Err("root already set".to_string());
                    }
                    if self.allocated.contains(key) {
                        return Err("duplicate key".to_string());
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
                        return Err("duplicate key".to_string());
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
                        return Err("duplicate key".to_string());
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
                        return Err("duplicate key".to_string());
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
                    self.append_child(*parent, *child)?;
                }
                DomPatch::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    self.insert_before(*parent, *child, *before)?;
                }
                DomPatch::RemoveNode { key } => {
                    self.remove_subtree(*key);
                }
                DomPatch::SetAttributes { key, attributes } => {
                    let Some(node) = self.nodes.get_mut(key) else {
                        return Err("missing node".to_string());
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
                        return Err("missing node".to_string());
                    };
                    match &mut node.kind {
                        TestKind::Text { text: existing } => *existing = text.clone(),
                        TestKind::Comment { .. }
                        | TestKind::Document { .. }
                        | TestKind::Element { .. } => {
                            return Err("SetText applied to non-text node".to_string());
                        }
                    }
                }
                DomPatch::AppendText { key, text } => {
                    let Some(node) = self.nodes.get_mut(key) else {
                        return Err("missing node".to_string());
                    };
                    match &mut node.kind {
                        TestKind::Text { text: existing } => existing.push_str(text),
                        TestKind::Comment { .. }
                        | TestKind::Document { .. }
                        | TestKind::Element { .. } => {
                            return Err("AppendText applied to non-text node".to_string());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn ensure_container(&self, key: PatchKey, context: &str) -> Result<(), String> {
        let Some(node) = self.nodes.get(&key) else {
            return Err(format!("missing node in {context}"));
        };
        match node.kind {
            TestKind::Document { .. } | TestKind::Element { .. } => Ok(()),
            TestKind::Text { .. } | TestKind::Comment { .. } => {
                Err(format!("{context} must be a container"))
            }
        }
    }

    fn node_parent(&self, key: PatchKey) -> Result<Option<PatchKey>, String> {
        self.nodes
            .get(&key)
            .map(|node| node.parent)
            .ok_or_else(|| "missing node".to_string())
    }

    fn is_document_node(&self, key: PatchKey) -> Result<bool, String> {
        self.nodes
            .get(&key)
            .map(|node| matches!(node.kind, TestKind::Document { .. }))
            .ok_or_else(|| "missing node".to_string())
    }

    fn is_document_root_element(&self, key: PatchKey) -> Result<bool, String> {
        let Some(root) = self.root else {
            return Ok(false);
        };
        let Some(node) = self.nodes.get(&key) else {
            return Err("missing node".to_string());
        };
        Ok(node.parent == Some(root) && matches!(node.kind, TestKind::Element { .. }))
    }

    fn would_create_cycle(&self, parent: PatchKey, child: PatchKey) -> Result<bool, String> {
        let mut cursor = Some(parent);
        while let Some(current) = cursor {
            if current == child {
                return Ok(true);
            }
            cursor = self.node_parent(current)?;
        }
        Ok(false)
    }

    fn detach_child(&mut self, child: PatchKey) -> Result<(), String> {
        let parent = self
            .nodes
            .get(&child)
            .ok_or_else(|| "missing child".to_string())?
            .parent;
        if let Some(parent) = parent
            && let Some(parent_node) = self.nodes.get_mut(&parent)
        {
            parent_node.children.retain(|key| *key != child);
        }
        let Some(child_node) = self.nodes.get_mut(&child) else {
            return Err("missing child".to_string());
        };
        child_node.parent = None;
        Ok(())
    }

    fn append_child(&mut self, parent: PatchKey, child: PatchKey) -> Result<(), String> {
        if parent == child {
            return Err("AppendChild cannot attach a node to itself".to_string());
        }
        self.ensure_container(parent, "AppendChild parent")?;
        if !self.nodes.contains_key(&child) {
            return Err("missing child".to_string());
        }
        if self.is_document_node(child)? {
            return Err("AppendChild cannot move a document node".to_string());
        }
        if self.is_document_root_element(child)? {
            return Err("AppendChild cannot move the document root element".to_string());
        }
        if self.would_create_cycle(parent, child)? {
            return Err("AppendChild cannot create an ancestor cycle".to_string());
        }
        let already_last = self.node_parent(child)? == Some(parent)
            && self
                .nodes
                .get(&parent)
                .is_some_and(|node| node.children.last() == Some(&child));
        if already_last {
            return Ok(());
        }
        self.detach_child(child)?;
        let Some(parent_node) = self.nodes.get_mut(&parent) else {
            return Err("missing parent".to_string());
        };
        parent_node.children.push(child);
        let Some(child_node) = self.nodes.get_mut(&child) else {
            return Err("missing child".to_string());
        };
        child_node.parent = Some(parent);
        Ok(())
    }

    fn insert_before(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) -> Result<(), String> {
        if parent == child {
            return Err("InsertBefore cannot attach a node to itself".to_string());
        }
        if child == before {
            return Err("InsertBefore cannot insert a node before itself".to_string());
        }
        self.ensure_container(parent, "InsertBefore parent")?;
        if !self.nodes.contains_key(&child) {
            return Err("missing child".to_string());
        }
        if !self.nodes.contains_key(&before) {
            return Err("missing before".to_string());
        }
        if self.is_document_node(child)? {
            return Err("InsertBefore cannot move a document node".to_string());
        }
        if self.is_document_root_element(child)? {
            return Err("InsertBefore cannot move the document root element".to_string());
        }
        if self.node_parent(before)? != Some(parent) {
            return Err("missing before".to_string());
        }
        if self.would_create_cycle(parent, child)? {
            return Err("InsertBefore cannot create an ancestor cycle".to_string());
        }
        let already_in_place = if self.node_parent(child)? == Some(parent) {
            let siblings = &self
                .nodes
                .get(&parent)
                .ok_or_else(|| "missing parent".to_string())?
                .children;
            let child_index = siblings.iter().position(|key| *key == child);
            let before_index = siblings.iter().position(|key| *key == before);
            matches!((child_index, before_index), (Some(child_index), Some(before_index)) if child_index + 1 == before_index)
        } else {
            false
        };
        if already_in_place {
            return Ok(());
        }
        self.detach_child(child)?;
        let pos = {
            let Some(parent_node) = self.nodes.get(&parent) else {
                return Err("missing parent".to_string());
            };
            parent_node
                .children
                .iter()
                .position(|key| *key == before)
                .ok_or_else(|| "missing before".to_string())?
        };
        let Some(parent_node) = self.nodes.get_mut(&parent) else {
            return Err("missing parent".to_string());
        };
        parent_node.children.insert(pos, child);
        let Some(child_node) = self.nodes.get_mut(&child) else {
            return Err("missing child".to_string());
        };
        child_node.parent = Some(parent);
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

    pub(crate) fn materialize(&self) -> Result<Node, String> {
        let root = self.root.ok_or_else(|| "missing root".to_string())?;
        self.materialize_node(root)
    }

    fn materialize_node(&self, key: PatchKey) -> Result<Node, String> {
        let Some(node) = self.nodes.get(&key) else {
            return Err("missing node".to_string());
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

    fn assert_invariants(&self) -> Result<(), String> {
        if let Some(root) = self.root {
            let Some(root_node) = self.nodes.get(&root) else {
                return Err("missing root node".to_string());
            };
            if root_node.parent.is_some() {
                return Err("root node must not have a parent".to_string());
            }
        }

        for (key, node) in &self.nodes {
            if let Some(parent) = node.parent {
                let Some(parent_node) = self.nodes.get(&parent) else {
                    return Err(format!("dangling parent reference for {key:?}"));
                };
                let matches = parent_node
                    .children
                    .iter()
                    .filter(|child| **child == *key)
                    .count();
                if matches != 1 {
                    return Err(format!(
                        "parent/child mismatch for {key:?}: expected exactly one reference from {parent:?}, found {matches}"
                    ));
                }
            }

            let mut unique_children = HashSet::new();
            for child in &node.children {
                if !self.nodes.contains_key(child) {
                    return Err(format!("dangling child reference {child:?} under {key:?}"));
                }
                if !unique_children.insert(*child) {
                    return Err(format!("duplicate child reference {child:?} under {key:?}"));
                }
                let child_parent = self
                    .nodes
                    .get(child)
                    .and_then(|child_node| child_node.parent)
                    .ok_or_else(|| format!("child {child:?} missing parent back-reference"))?;
                if child_parent != *key {
                    return Err(format!(
                        "child {child:?} parent mismatch: expected {key:?}, found {child_parent:?}"
                    ));
                }
            }
        }

        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        for key in self.nodes.keys().copied() {
            self.assert_acyclic_from(key, &mut visited, &mut visiting)?;
        }

        Ok(())
    }

    fn assert_acyclic_from(
        &self,
        key: PatchKey,
        visited: &mut HashSet<PatchKey>,
        visiting: &mut HashSet<PatchKey>,
    ) -> Result<(), String> {
        if visited.contains(&key) {
            return Ok(());
        }
        if !visiting.insert(key) {
            return Err(format!("cycle detected at {key:?}"));
        }
        let node = self
            .nodes
            .get(&key)
            .ok_or_else(|| format!("missing node during cycle check: {key:?}"))?;
        for child in &node.children {
            self.assert_acyclic_from(*child, visited, visiting)?;
        }
        visiting.remove(&key);
        visited.insert(key);
        Ok(())
    }
}

fn patch_key(id: Id) -> Result<PatchKey, String> {
    if id == Id::INVALID {
        return Err("invalid key".to_string());
    }
    Ok(PatchKey::from_id(id))
}

#[cfg(test)]
mod tests {
    use super::TestPatchArena;
    use crate::DomPatch;
    use crate::dom_patch::PatchKey;

    #[test]
    fn test_patch_arena_supports_cross_parent_reparenting() {
        let mut arena = TestPatchArena::default();
        arena
            .apply(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "div".into(),
                    attributes: Vec::new(),
                },
                DomPatch::CreateElement {
                    key: PatchKey(3),
                    name: "p".into(),
                    attributes: Vec::new(),
                },
                DomPatch::CreateElement {
                    key: PatchKey(4),
                    name: "span".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(3),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(4),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(3),
                    child: PatchKey(4),
                },
            ])
            .expect("cross-parent reparenting should apply");

        assert_eq!(
            arena.nodes.get(&PatchKey(4)).and_then(|node| node.parent),
            Some(PatchKey(3))
        );
        assert_eq!(
            arena
                .nodes
                .get(&PatchKey(2))
                .map(|node| node.children.clone())
                .unwrap_or_default(),
            Vec::<PatchKey>::new()
        );
    }

    #[test]
    fn test_patch_arena_rolls_back_failed_batches() {
        let mut arena = TestPatchArena::default();
        arena
            .apply(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "div".into(),
                    attributes: Vec::new(),
                },
                DomPatch::CreateElement {
                    key: PatchKey(3),
                    name: "p".into(),
                    attributes: Vec::new(),
                },
                DomPatch::CreateElement {
                    key: PatchKey(4),
                    name: "span".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(3),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(4),
                },
            ])
            .expect("seed batch should apply");

        let error = arena
            .apply(&[
                DomPatch::AppendChild {
                    parent: PatchKey(3),
                    child: PatchKey(4),
                },
                DomPatch::InsertBefore {
                    parent: PatchKey(99),
                    child: PatchKey(2),
                    before: PatchKey(4),
                },
            ])
            .expect_err("invalid second patch should fail");
        assert!(
            error.contains("missing node in InsertBefore parent")
                || error.contains("missing parent")
                || error.contains("missing before"),
            "unexpected rollback error: {error}"
        );
        assert_eq!(
            arena.nodes.get(&PatchKey(4)).and_then(|node| node.parent),
            Some(PatchKey(2)),
            "failed batch must preserve original parentage"
        );
    }

    #[test]
    fn test_patch_arena_supports_aaa_furthest_block_move_sequence() {
        let mut arena = TestPatchArena::default();
        arena
            .apply(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: Some("html".to_string()),
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "html".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::CreateElement {
                    key: PatchKey(3),
                    name: "head".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(3),
                },
                DomPatch::CreateElement {
                    key: PatchKey(4),
                    name: "body".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(4),
                },
                DomPatch::CreateElement {
                    key: PatchKey(5),
                    name: "a".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(4),
                    child: PatchKey(5),
                },
                DomPatch::CreateElement {
                    key: PatchKey(6),
                    name: "p".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(5),
                    child: PatchKey(6),
                },
                DomPatch::CreateText {
                    key: PatchKey(7),
                    text: "one".to_string(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(6),
                    child: PatchKey(7),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(4),
                    child: PatchKey(6),
                },
                DomPatch::CreateElement {
                    key: PatchKey(8),
                    name: "a".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(8),
                    child: PatchKey(7),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(6),
                    child: PatchKey(8),
                },
            ])
            .expect("AAA furthest-block move sequence should apply");

        assert_eq!(
            arena.nodes.get(&PatchKey(6)).and_then(|node| node.parent),
            Some(PatchKey(4)),
            "furthest block should move under the common ancestor"
        );
        assert_eq!(
            arena.nodes.get(&PatchKey(7)).and_then(|node| node.parent),
            Some(PatchKey(8)),
            "moved text node should retain its original key under the recreated formatting element"
        );
        assert_eq!(
            arena
                .nodes
                .get(&PatchKey(4))
                .map(|node| node.children.clone())
                .unwrap_or_default(),
            vec![PatchKey(5), PatchKey(6)],
            "unaffected and moved siblings must keep deterministic ordering under body"
        );
    }

    #[test]
    fn test_patch_arena_supports_aaa_foster_parent_insert_before_sequence() {
        let mut arena = TestPatchArena::default();
        arena
            .apply(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: Some("html".to_string()),
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "html".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::CreateElement {
                    key: PatchKey(3),
                    name: "head".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(3),
                },
                DomPatch::CreateElement {
                    key: PatchKey(4),
                    name: "body".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(4),
                },
                DomPatch::CreateElement {
                    key: PatchKey(5),
                    name: "table".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(4),
                    child: PatchKey(5),
                },
                DomPatch::CreateElement {
                    key: PatchKey(6),
                    name: "a".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(5),
                    child: PatchKey(6),
                },
                DomPatch::CreateElement {
                    key: PatchKey(7),
                    name: "tr".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(6),
                    child: PatchKey(7),
                },
                DomPatch::CreateText {
                    key: PatchKey(8),
                    text: "x".to_string(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(7),
                    child: PatchKey(8),
                },
                DomPatch::InsertBefore {
                    parent: PatchKey(4),
                    child: PatchKey(7),
                    before: PatchKey(5),
                },
                DomPatch::CreateElement {
                    key: PatchKey(9),
                    name: "a".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(9),
                    child: PatchKey(8),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(7),
                    child: PatchKey(9),
                },
            ])
            .expect("AAA foster-parent move sequence should apply");

        assert_eq!(
            arena.nodes.get(&PatchKey(7)).and_then(|node| node.parent),
            Some(PatchKey(4)),
            "foster-parented furthest block should move before the table without losing identity"
        );
        assert_eq!(
            arena.nodes.get(&PatchKey(8)).and_then(|node| node.parent),
            Some(PatchKey(9)),
            "moved text node should retain its original key under the recreated formatting element"
        );
        assert_eq!(
            arena
                .nodes
                .get(&PatchKey(4))
                .map(|node| node.children.clone())
                .unwrap_or_default(),
            vec![PatchKey(7), PatchKey(5)],
            "foster-parent InsertBefore must leave the moved node immediately before the table"
        );
    }
}
