use std::sync::Arc;

use crate::Node;
use crate::dom_patch::PatchKey;
use crate::types::Id;

use super::error::{ArenaResult, PatchValidationError};
use super::model::{PatchKind, PatchValidationArena};

impl PatchValidationArena {
    pub fn materialize(&self) -> Result<Node, PatchValidationError> {
        let root = self.root.ok_or_else(|| {
            PatchValidationError::new("materialize", "missing document root after patch apply")
        })?;
        self.materialize_node(root)
    }

    fn materialize_node(&self, key: PatchKey) -> ArenaResult<Node> {
        let node = self.nodes.get(&key).ok_or_else(|| {
            PatchValidationError::new("materialize", format!("missing node {key:?}"))
        })?;

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
