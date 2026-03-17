use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::{Id, Node};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct PatchNode {
    kind: PatchKind,
    parent: Option<PatchKey>,
    children: Vec<PatchKey>,
}

#[derive(Clone, Debug)]
enum PatchKind {
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

#[derive(Default)]
pub(super) struct PatchArena {
    nodes: HashMap<PatchKey, PatchNode>,
    root: Option<PatchKey>,
}

impl PatchArena {
    pub(super) fn apply(&mut self, patches: &[DomPatch]) {
        for patch in patches {
            match patch {
                DomPatch::CreateDocument { key, doctype } => {
                    assert!(self.root.is_none(), "document root already exists");
                    assert!(
                        !self.nodes.contains_key(key),
                        "duplicate key in CreateDocument"
                    );
                    self.nodes.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Document {
                                doctype: doctype.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                    self.root = Some(*key);
                }
                DomPatch::CreateElement {
                    key,
                    name,
                    attributes,
                } => {
                    assert!(
                        !self.nodes.contains_key(key),
                        "duplicate key in CreateElement"
                    );
                    self.nodes.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Element {
                                name: Arc::clone(name),
                                attributes: attributes.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                }
                DomPatch::CreateText { key, text } => {
                    assert!(!self.nodes.contains_key(key), "duplicate key in CreateText");
                    self.nodes.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Text { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                }
                DomPatch::CreateComment { key, text } => {
                    assert!(
                        !self.nodes.contains_key(key),
                        "duplicate key in CreateComment"
                    );
                    self.nodes.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Comment { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                }
                DomPatch::AppendChild { parent, child } => {
                    assert_ne!(parent, child, "AppendChild cannot attach a node to itself");
                    let Some(mut child_node) = self.nodes.remove(child) else {
                        panic!("missing child in AppendChild");
                    };
                    let parent_is_container = matches!(
                        self.nodes.get(parent).map(|node| &node.kind),
                        Some(PatchKind::Document { .. } | PatchKind::Element { .. })
                    );
                    if !self.nodes.contains_key(parent) {
                        self.nodes.insert(*child, child_node);
                        panic!("missing parent in AppendChild");
                    }
                    if !parent_is_container {
                        self.nodes.insert(*child, child_node);
                        panic!("AppendChild parent must be a container");
                    }
                    if let Some(existing_parent) = child_node.parent {
                        let already_last = existing_parent == *parent
                            && self.nodes.get(parent).and_then(|node| node.children.last())
                                == Some(child);
                        if already_last {
                            self.nodes.insert(*child, child_node);
                            continue;
                        }
                        if let Some(previous_parent) = self.nodes.get_mut(&existing_parent) {
                            previous_parent.children.retain(|key| key != child);
                        }
                    }
                    let Some(parent_node) = self.nodes.get_mut(parent) else {
                        self.nodes.insert(*child, child_node);
                        panic!("missing parent in AppendChild");
                    };
                    assert!(
                        !parent_node.children.iter().any(|k| k == child),
                        "child already present in parent after detach"
                    );
                    parent_node.children.push(*child);
                    child_node.parent = Some(*parent);
                    self.nodes.insert(*child, child_node);
                }
                DomPatch::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    assert_ne!(parent, child, "InsertBefore cannot attach a node to itself");
                    assert_ne!(
                        child, before,
                        "InsertBefore cannot insert a node before itself"
                    );
                    let Some(mut child_node) = self.nodes.remove(child) else {
                        panic!("missing child in InsertBefore");
                    };
                    let parent_is_container = matches!(
                        self.nodes.get(parent).map(|node| &node.kind),
                        Some(PatchKind::Document { .. } | PatchKind::Element { .. })
                    );
                    if !self.nodes.contains_key(parent) {
                        self.nodes.insert(*child, child_node);
                        panic!("missing parent in InsertBefore");
                    }
                    if !parent_is_container {
                        self.nodes.insert(*child, child_node);
                        panic!("InsertBefore parent must be a container");
                    }
                    if let Some(existing_parent) = child_node.parent {
                        if existing_parent == *parent {
                            let child_index = self
                                .nodes
                                .get(parent)
                                .and_then(|node| node.children.iter().position(|key| key == child));
                            let before_index = self.nodes.get(parent).and_then(|node| {
                                node.children.iter().position(|key| key == before)
                            });
                            if matches!((child_index, before_index), (Some(child_index), Some(before_index)) if child_index + 1 == before_index)
                            {
                                self.nodes.insert(*child, child_node);
                                continue;
                            }
                        }
                        if let Some(previous_parent) = self.nodes.get_mut(&existing_parent) {
                            previous_parent.children.retain(|key| key != child);
                        }
                    }
                    let Some(parent_node) = self.nodes.get_mut(parent) else {
                        self.nodes.insert(*child, child_node);
                        panic!("missing parent in InsertBefore");
                    };
                    let before_index = parent_node
                        .children
                        .iter()
                        .position(|key| key == before)
                        .expect("before child missing in InsertBefore");
                    parent_node.children.insert(before_index, *child);
                    child_node.parent = Some(*parent);
                    self.nodes.insert(*child, child_node);
                }
                DomPatch::SetText { key, text } => {
                    let Some(node) = self.nodes.get_mut(key) else {
                        panic!("missing node in SetText");
                    };
                    match &mut node.kind {
                        PatchKind::Text { text: slot } => *slot = text.clone(),
                        _ => panic!("SetText applied to non-text node"),
                    }
                }
                DomPatch::AppendText { key, text } => {
                    let Some(node) = self.nodes.get_mut(key) else {
                        panic!("missing node in AppendText");
                    };
                    match &mut node.kind {
                        PatchKind::Text { text: slot } => slot.push_str(text),
                        _ => panic!("AppendText applied to non-text node"),
                    }
                }
                _ => panic!("unexpected patch in core emission test: {patch:?}"),
            }
        }
    }

    pub(super) fn materialize(&self) -> Node {
        let root = self.root.expect("missing root in patch arena");
        self.materialize_node(root)
    }

    fn materialize_node(&self, key: PatchKey) -> Node {
        let node = self.nodes.get(&key).expect("missing node");
        let children = node
            .children
            .iter()
            .map(|child| self.materialize_node(*child))
            .collect();
        match &node.kind {
            PatchKind::Document { doctype } => Node::Document {
                id: Id::INVALID,
                doctype: doctype.clone(),
                children,
            },
            PatchKind::Element { name, attributes } => Node::Element {
                id: Id::INVALID,
                name: Arc::clone(name),
                attributes: attributes.clone(),
                style: Vec::new(),
                children,
            },
            PatchKind::Text { text } => Node::Text {
                id: Id::INVALID,
                text: text.clone(),
            },
            PatchKind::Comment { text } => Node::Comment {
                id: Id::INVALID,
                text: text.clone(),
            },
        }
    }
}
