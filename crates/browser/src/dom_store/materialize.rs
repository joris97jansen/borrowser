use super::arena::{DomArena, NodeKind};
use super::error::DomPatchError;
use html::{Node, PatchKey};
use std::sync::Arc;

impl DomArena {
    pub(crate) fn materialize(&self, root: PatchKey) -> Result<Node, DomPatchError> {
        let Some(&index) = self.live.get(&root) else {
            return Err(DomPatchError::MissingKey(root));
        };
        self.materialize_node(root, index)
    }

    fn materialize_node(&self, key: PatchKey, index: usize) -> Result<Node, DomPatchError> {
        let id = self.materialized_node_id_for_key(key)?;
        let children = self.nodes[index]
            .children
            .iter()
            .map(|child_key| {
                let child_index = *self
                    .live
                    .get(child_key)
                    .ok_or(DomPatchError::MissingKey(*child_key))?;
                self.materialize_node(*child_key, child_index)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let node = match &self.nodes[index].kind {
            NodeKind::Document { doctype } => Node::Document {
                id,
                doctype: doctype.clone(),
                children,
            },
            NodeKind::DocumentType {
                name,
                public_id,
                system_id,
            } => Node::DocumentType {
                id,
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            },
            NodeKind::Element { name, attributes } => Node::Element {
                id,
                name: Arc::clone(name),
                attributes: attributes.clone(),
                style: Vec::new(),
                children,
            },
            NodeKind::Text { text } => Node::Text {
                id,
                text: text.clone(),
            },
            NodeKind::Comment { text } => Node::Comment {
                id,
                text: text.clone(),
            },
        };
        Ok(node)
    }
}
