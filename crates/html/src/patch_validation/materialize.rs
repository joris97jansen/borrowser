use crate::Node;
use crate::dom_patch::PatchKey;
use crate::types::{DocumentFragmentNode, Id, ParserCreatedFragmentKind};

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
            PatchKind::DocumentType {
                name,
                public_id,
                system_id,
            } => Node::DocumentType {
                id: Id::INVALID,
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            },
            PatchKind::Element {
                name,
                attributes,
                template_contents,
            } => crate::Node::from_element_parts(
                Id::INVALID,
                name.clone(),
                attributes.clone(),
                Vec::new(),
                template_contents
                    .map(|contents| self.materialize_fragment(contents))
                    .transpose()?
                    .map(Box::new),
                children,
            ),
            PatchKind::DocumentFragment { .. } => {
                return Err(PatchValidationError::new(
                    "materialize",
                    "document fragments materialize only through their template host",
                ));
            }
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

    fn materialize_fragment(&self, key: PatchKey) -> ArenaResult<DocumentFragmentNode> {
        let node = self.nodes.get(&key).ok_or_else(|| {
            PatchValidationError::new("materialize fragment", format!("missing node {key:?}"))
        })?;
        let PatchKind::DocumentFragment { kind, .. } = node.kind else {
            return Err(PatchValidationError::new(
                "materialize fragment",
                "associated contents key is not a document fragment",
            ));
        };
        if kind != ParserCreatedFragmentKind::TemplateContents {
            return Err(PatchValidationError::new(
                "materialize fragment",
                "associated fragment is not template contents",
            ));
        }
        let children = node
            .children
            .iter()
            .map(|child| self.materialize_node(*child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DocumentFragmentNode::new_template_contents(
            Id::INVALID,
            children,
        ))
    }
}
