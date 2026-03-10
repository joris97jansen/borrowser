use super::arena::{DomArena, NodeKind};
use super::error::DomPatchError;
use html::internal::Id;
use html::{Node, PatchKey};
use std::sync::Arc;

impl DomArena {
    pub(crate) fn materialize(&self, root: PatchKey) -> Result<Node, DomPatchError> {
        let Some(&index) = self.live.get(&root) else {
            return Err(DomPatchError::MissingKey(root));
        };
        self.materialize_node(index)
    }

    fn materialize_node(&self, index: usize) -> Result<Node, DomPatchError> {
        let id = Id::INVALID;
        let children = self.nodes[index]
            .children
            .iter()
            .map(|child_key| {
                let child_index = *self
                    .live
                    .get(child_key)
                    .ok_or(DomPatchError::MissingKey(*child_key))?;
                self.materialize_node(child_index)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let node = match &self.nodes[index].kind {
            NodeKind::Document { doctype } => Node::Document {
                id,
                doctype: doctype.clone(),
                children,
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
