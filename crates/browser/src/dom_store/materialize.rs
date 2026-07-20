use super::arena::{DomArena, NodeKind};
use super::error::DomPatchError;
use html::internal::ParserCreatedFragmentKind;
use html::{Node, PatchKey};

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
            NodeKind::Element {
                name,
                attributes,
                template_contents,
            } => {
                if let Some(contents) = template_contents {
                    let (contents_id, contents_children) = self.materialize_fragment(*contents)?;
                    if !name.is(html::ElementNamespace::Html, "template") {
                        return Err(DomPatchError::Protocol(
                            "template contents association targeted a non-template host",
                        ));
                    }
                    html::internal::template_element_from_parts(
                        id,
                        name.clone(),
                        attributes.clone(),
                        Vec::new(),
                        contents_id,
                        contents_children,
                        children,
                    )
                } else {
                    html::internal::node_element_from_parts(
                        id,
                        name.clone(),
                        attributes.clone(),
                        Vec::new(),
                        children,
                    )
                }
            }
            NodeKind::DocumentFragment { .. } => {
                return Err(DomPatchError::Protocol(
                    "document fragments materialize only through template hosts",
                ));
            }
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

    fn materialize_fragment(
        &self,
        key: PatchKey,
    ) -> Result<(html::internal::Id, Vec<Node>), DomPatchError> {
        let &index = self.live.get(&key).ok_or(DomPatchError::MissingKey(key))?;
        let NodeKind::DocumentFragment { kind, .. } = self.nodes[index].kind else {
            return Err(DomPatchError::WrongNodeKind {
                key,
                expected: "DocumentFragment",
                actual: self.nodes[index].kind_name(),
            });
        };
        if kind != ParserCreatedFragmentKind::TemplateContents {
            return Err(DomPatchError::Protocol(
                "template contents association targeted the wrong fragment kind",
            ));
        }
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
        Ok((self.materialized_node_id_for_key(key)?, children))
    }
}
