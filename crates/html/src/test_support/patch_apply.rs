use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::{Id, Node};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Default)]
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
                    let child_has_parent = self
                        .nodes
                        .get(child)
                        .ok_or_else(|| "missing child".to_string())?
                        .parent
                        .is_some();
                    if child_has_parent {
                        return Err("child already has parent".to_string());
                    }
                    let Some(parent_node) = self.nodes.get_mut(parent) else {
                        return Err("missing parent".to_string());
                    };
                    parent_node.children.push(*child);
                    let Some(child_node) = self.nodes.get_mut(child) else {
                        return Err("missing child".to_string());
                    };
                    child_node.parent = Some(*parent);
                }
                DomPatch::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    let child_has_parent = self
                        .nodes
                        .get(child)
                        .ok_or_else(|| "missing child".to_string())?
                        .parent
                        .is_some();
                    if child_has_parent {
                        return Err("child already has parent".to_string());
                    }
                    let pos = {
                        let Some(parent_node) = self.nodes.get(parent) else {
                            return Err("missing parent".to_string());
                        };
                        parent_node
                            .children
                            .iter()
                            .position(|k| k == before)
                            .ok_or_else(|| "missing before".to_string())?
                    };
                    let Some(parent_node) = self.nodes.get_mut(parent) else {
                        return Err("missing parent".to_string());
                    };
                    parent_node.children.insert(pos, *child);
                    let Some(child_node) = self.nodes.get_mut(child) else {
                        return Err("missing child".to_string());
                    };
                    child_node.parent = Some(*parent);
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
}

fn patch_key(id: Id) -> Result<PatchKey, String> {
    if id == Id::INVALID {
        return Err("invalid key".to_string());
    }
    Ok(PatchKey::from_id(id))
}
