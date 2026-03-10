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
                    let Some(parent_node) = self.nodes.get_mut(parent) else {
                        self.nodes.insert(*child, child_node);
                        panic!("missing parent in AppendChild");
                    };
                    match parent_node.kind {
                        PatchKind::Document { .. } | PatchKind::Element { .. } => {}
                        _ => {
                            self.nodes.insert(*child, child_node);
                            panic!("AppendChild parent must be a container");
                        }
                    }
                    assert!(child_node.parent.is_none(), "child already has parent");
                    assert!(
                        !parent_node.children.iter().any(|k| k == child),
                        "child already present in parent"
                    );
                    parent_node.children.push(*child);
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
