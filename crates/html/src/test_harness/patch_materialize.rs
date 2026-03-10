use crate::Node;
use crate::dom_patch::PatchKey;
use crate::types::Id;
use std::collections::HashMap;
use std::sync::Arc;

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

#[derive(Clone, Debug)]
struct PatchNode {
    kind: PatchKind,
    parent: Option<PatchKey>,
    children: Vec<PatchKey>,
}

#[derive(Default)]
struct PatchArena {
    nodes: HashMap<PatchKey, PatchNode>,
    root: Option<PatchKey>,
}

impl PatchArena {
    fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
    }

    fn insert(&mut self, key: PatchKey, node: PatchNode) -> Result<(), String> {
        if key == PatchKey::INVALID {
            return Err("patch key must be non-zero".to_string());
        }
        if self.nodes.contains_key(&key) {
            return Err(format!("duplicate patch key {key:?}"));
        }
        self.nodes.insert(key, node);
        Ok(())
    }

    fn ensure_node(&self, key: PatchKey, context: &str) -> Result<(), String> {
        if key == PatchKey::INVALID {
            return Err(format!("invalid patch key in {context}"));
        }
        if !self.nodes.contains_key(&key) {
            return Err(format!("missing node {key:?} in {context}"));
        }
        Ok(())
    }

    fn apply_batch(&mut self, patches: &[crate::DomPatch]) -> Result<(), String> {
        if patches
            .get(1..)
            .is_some_and(|rest| rest.iter().any(|p| matches!(p, crate::DomPatch::Clear)))
        {
            return Err("Clear may only appear as the first patch in a batch".to_string());
        }
        let mut start = 0usize;
        if matches!(patches.first(), Some(crate::DomPatch::Clear)) {
            self.clear();
            start = 1;
        }
        for patch in &patches[start..] {
            match patch {
                crate::DomPatch::Clear => {
                    return Err("Clear must be first patch in a batch".to_string());
                }
                crate::DomPatch::CreateDocument { key, doctype } => {
                    if self.root.is_some() {
                        return Err("document root already exists".to_string());
                    }
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Document {
                                doctype: doctype.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
                    self.root = Some(*key);
                }
                crate::DomPatch::CreateElement {
                    key,
                    name,
                    attributes,
                } => {
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Element {
                                name: Arc::clone(name),
                                attributes: attributes.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
                }
                crate::DomPatch::CreateText { key, text } => {
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Text { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
                }
                crate::DomPatch::CreateComment { key, text } => {
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Comment { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
                }
                crate::DomPatch::AppendChild { parent, child } => {
                    self.ensure_node(*parent, "AppendChild parent")?;
                    self.ensure_node(*child, "AppendChild child")?;
                    let child_parent = self.nodes.get(child).and_then(|node| node.parent);
                    if child_parent.is_some() {
                        return Err("child already has parent".to_string());
                    }
                    let parent_is_container = {
                        let parent_node = self
                            .nodes
                            .get(parent)
                            .ok_or_else(|| "missing parent".to_string())?;
                        matches!(
                            parent_node.kind,
                            PatchKind::Document { .. } | PatchKind::Element { .. }
                        )
                    };
                    if !parent_is_container {
                        return Err("AppendChild parent must be a container".to_string());
                    }
                    self.nodes
                        .get_mut(parent)
                        .ok_or_else(|| "missing parent".to_string())?
                        .children
                        .push(*child);
                    self.nodes
                        .get_mut(child)
                        .ok_or_else(|| "missing child".to_string())?
                        .parent = Some(*parent);
                }
                crate::DomPatch::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    if parent == child {
                        return Err("InsertBefore cannot attach a node to itself".to_string());
                    }
                    if child == before {
                        return Err("InsertBefore cannot insert a node before itself".to_string());
                    }
                    self.ensure_node(*parent, "InsertBefore parent")?;
                    self.ensure_node(*child, "InsertBefore child")?;
                    self.ensure_node(*before, "InsertBefore before")?;
                    let (parent_is_container, before_index) = {
                        let parent_node = self
                            .nodes
                            .get(parent)
                            .ok_or_else(|| "missing parent".to_string())?;
                        let is_container = matches!(
                            parent_node.kind,
                            PatchKind::Document { .. } | PatchKind::Element { .. }
                        );
                        let index = parent_node
                            .children
                            .iter()
                            .position(|k| k == before)
                            .ok_or_else(|| "before child not found in parent".to_string())?;
                        (is_container, index)
                    };
                    if !parent_is_container {
                        return Err("InsertBefore parent must be a container".to_string());
                    }
                    let before_parent = self
                        .nodes
                        .get(before)
                        .and_then(|node| node.parent)
                        .ok_or_else(|| "before child has no parent".to_string())?;
                    if before_parent != *parent {
                        return Err("before child is not attached to parent".to_string());
                    }
                    if self.nodes.get(child).and_then(|node| node.parent).is_some() {
                        return Err("child already has parent".to_string());
                    }
                    self.nodes
                        .get_mut(parent)
                        .ok_or_else(|| "missing parent".to_string())?
                        .children
                        .insert(before_index, *child);
                    self.nodes
                        .get_mut(child)
                        .ok_or_else(|| "missing child".to_string())?
                        .parent = Some(*parent);
                }
                crate::DomPatch::RemoveNode { key } => {
                    self.ensure_node(*key, "RemoveNode")?;
                    let is_root = self.root == Some(*key);
                    let is_attached = self.nodes.get(key).and_then(|node| node.parent).is_some();
                    if !is_root && !is_attached {
                        return Err("RemoveNode applied to detached node".to_string());
                    }
                    self.remove_subtree(*key)?;
                }
                crate::DomPatch::SetAttributes { key, attributes } => {
                    self.ensure_node(*key, "SetAttributes")?;
                    let node = self
                        .nodes
                        .get_mut(key)
                        .ok_or_else(|| "missing node".to_string())?;
                    match &mut node.kind {
                        PatchKind::Element {
                            attributes: slot, ..
                        } => {
                            *slot = attributes.clone();
                        }
                        _ => return Err("SetAttributes applied to non-element".to_string()),
                    }
                }
                crate::DomPatch::SetText { key, text } => {
                    self.ensure_node(*key, "SetText")?;
                    let node = self
                        .nodes
                        .get_mut(key)
                        .ok_or_else(|| "missing node".to_string())?;
                    match &mut node.kind {
                        PatchKind::Text { text: slot } => {
                            *slot = text.clone();
                        }
                        _ => return Err("SetText applied to non-text".to_string()),
                    }
                }
                crate::DomPatch::AppendText { key, text } => {
                    self.ensure_node(*key, "AppendText")?;
                    let node = self
                        .nodes
                        .get_mut(key)
                        .ok_or_else(|| "missing node".to_string())?;
                    match &mut node.kind {
                        PatchKind::Text { text: slot } => {
                            slot.push_str(text);
                        }
                        _ => return Err("AppendText applied to non-text".to_string()),
                    }
                }
            }
        }
        Ok(())
    }

    fn remove_subtree(&mut self, key: PatchKey) -> Result<(), String> {
        let children = {
            let node = self
                .nodes
                .get(&key)
                .ok_or_else(|| "missing node".to_string())?;
            node.children.clone()
        };
        if let Some(parent) = self.nodes.get(&key).and_then(|node| node.parent)
            && let Some(parent_node) = self.nodes.get_mut(&parent)
        {
            parent_node.children.retain(|child| *child != key);
        }
        for child in children {
            self.remove_subtree(child)?;
        }
        self.nodes.remove(&key);
        if self.root == Some(key) {
            self.root = None;
        }
        Ok(())
    }

    fn materialize(&self) -> Result<Node, String> {
        let root = self
            .root
            .ok_or_else(|| "missing document root".to_string())?;
        self.materialize_node(root)
    }

    fn materialize_node(&self, key: PatchKey) -> Result<Node, String> {
        let node = self
            .nodes
            .get(&key)
            .ok_or_else(|| "missing node".to_string())?;
        let children = node
            .children
            .iter()
            .map(|child| self.materialize_node(*child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(match &node.kind {
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
        })
    }
}

/// Materialize a DOM tree from patch batches (test-only helper).
///
/// This is intended for deterministic golden tests; it enforces patch protocol invariants
/// per batch (e.g., `Clear` must be first in a batch). `Clear` resets the arena, allowing
/// a fresh document root in subsequent batches. Returns a simplified DOM with
/// `Id::INVALID` for all nodes.
pub fn materialize_patch_batches(batches: &[Vec<crate::DomPatch>]) -> Result<Node, String> {
    let mut arena = PatchArena::default();
    for batch in batches {
        arena.apply_batch(batch)?;
    }
    arena.materialize()
}

/// Backwards-compatible helper: treat a single vector as one batch.
pub fn materialize_patches(patches: &[crate::DomPatch]) -> Result<Node, String> {
    materialize_patch_batches(&[patches.to_vec()])
}
