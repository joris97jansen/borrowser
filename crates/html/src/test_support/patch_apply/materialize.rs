use super::arena::{TestKind, TestNode};
use super::{ArenaResult, TestPatchArena};
use crate::dom_patch::PatchKey;
use crate::types::{Id, Node};
use std::sync::Arc;

impl TestPatchArena {
    pub(crate) fn from_dom(root: &Node) -> ArenaResult<Self> {
        let mut arena = Self::default();
        arena.insert_from_dom(root, None)?;
        Ok(arena)
    }

    fn insert_from_dom(&mut self, node: &Node, parent: Option<PatchKey>) -> ArenaResult<()> {
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

    pub(crate) fn materialize(&self) -> ArenaResult<Node> {
        let root = self.root.ok_or_else(|| "missing root".to_string())?;
        self.materialize_node(root)
    }

    fn materialize_node(&self, key: PatchKey) -> ArenaResult<Node> {
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

fn patch_key(id: Id) -> ArenaResult<PatchKey> {
    if id == Id::INVALID {
        return Err("invalid key".to_string());
    }
    Ok(PatchKey::from_id(id))
}
