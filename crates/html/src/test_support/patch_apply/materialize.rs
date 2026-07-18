use super::arena::{TestKind, TestNode};
use super::{ArenaResult, TestPatchArena};
use crate::dom_patch::PatchKey;
use crate::types::{DocumentFragmentNode, Id, Node};
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
            Node::DocumentType {
                name,
                public_id,
                system_id,
                ..
            } => TestKind::DocumentType {
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            },
            Node::Element { element } => TestKind::Element {
                name: Arc::clone(element.name()),
                attributes: element.attributes().to_vec(),
                template_contents: element
                    .template_contents()
                    .map(|fragment| patch_key(fragment.id()))
                    .transpose()?,
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
        if let Node::Element { element } = node
            && let Some(fragment) = element.template_contents()
        {
            let contents = patch_key(fragment.id())?;
            if self.nodes.contains_key(&contents) || self.allocated.contains(&contents) {
                return Err("duplicate key".to_string());
            }
            self.nodes.insert(
                contents,
                TestNode {
                    kind: TestKind::DocumentFragment {
                        kind: fragment.kind(),
                        host: key,
                    },
                    parent: None,
                    children: Vec::new(),
                },
            );
            self.allocated.insert(contents);
            for child in fragment.children() {
                self.insert_from_dom(child, Some(contents))?;
                if let Some(entry) = self.nodes.get_mut(&contents) {
                    entry.children.push(patch_key(child.id())?);
                }
            }
        }
        if let Some(children) = node.children() {
            for child in children {
                self.insert_from_dom(child, Some(key))?;
                if let Some(entry) = self.nodes.get_mut(&key) {
                    entry.children.push(patch_key(child.id())?);
                }
            }
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
            TestKind::DocumentType {
                name,
                public_id,
                system_id,
            } => Node::DocumentType {
                id,
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            },
            TestKind::Element {
                name,
                attributes,
                template_contents,
            } => crate::Node::from_element_parts(
                id,
                Arc::clone(name),
                attributes.clone(),
                Vec::new(),
                template_contents
                    .map(|contents| self.materialize_fragment(contents))
                    .transpose()?
                    .map(Box::new),
                children,
            ),
            TestKind::DocumentFragment { .. } => {
                return Err("template contents must materialize through its host".to_string());
            }
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

    fn materialize_fragment(&self, key: PatchKey) -> ArenaResult<DocumentFragmentNode> {
        let Some(node) = self.nodes.get(&key) else {
            return Err("missing template contents".to_string());
        };
        let TestKind::DocumentFragment { kind, .. } = &node.kind else {
            return Err("template contents association targets a non-fragment".to_string());
        };
        let children = node
            .children
            .iter()
            .map(|child| self.materialize_node(*child))
            .collect::<Result<Vec<_>, _>>()?;
        if *kind != crate::types::ParserCreatedFragmentKind::TemplateContents {
            return Err("unsupported parser-created fragment kind".to_string());
        }
        Ok(DocumentFragmentNode::new_template_contents(
            Id::INVALID,
            children,
        ))
    }
}

fn patch_key(id: Id) -> ArenaResult<PatchKey> {
    if id == Id::INVALID {
        return Err("invalid key".to_string());
    }
    Ok(PatchKey::from_id(id))
}
