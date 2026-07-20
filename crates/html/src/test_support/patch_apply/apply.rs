use super::arena::{TestKind, TestNode};
use super::{ArenaResult, TestPatchArena};
use crate::dom_patch::DomPatch;
use crate::types::ParserCreatedFragmentKind;

impl TestPatchArena {
    pub(crate) fn apply(&mut self, patches: &[DomPatch]) -> ArenaResult<()> {
        let mut staged = self.clone();
        staged.apply_in_place(patches)?;
        staged.assert_invariants()?;
        *self = staged;
        Ok(())
    }

    fn apply_in_place(&mut self, patches: &[DomPatch]) -> ArenaResult<()> {
        for patch in patches {
            match patch {
                DomPatch::Clear => {
                    self.nodes.clear();
                    self.allocated.clear();
                    self.root = None;
                }
                DomPatch::CreateDocument { key, doctype } => {
                    if self.root.is_some() {
                        return Err("root already set".to_string());
                    }
                    if self.allocated.contains(key) {
                        return Err("duplicate key".to_string());
                    }
                    self.nodes.insert(
                        *key,
                        TestNode {
                            kind: TestKind::Document {
                                doctype: doctype.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                    self.allocated.insert(*key);
                    self.root = Some(*key);
                }
                DomPatch::CreateDocumentType {
                    key,
                    name,
                    public_id,
                    system_id,
                } => {
                    if self.allocated.contains(key) {
                        return Err("duplicate key".to_string());
                    }
                    self.nodes.insert(
                        *key,
                        TestNode {
                            kind: TestKind::DocumentType {
                                name: name.clone(),
                                public_id: public_id.clone(),
                                system_id: system_id.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                    self.allocated.insert(*key);
                }
                DomPatch::CreateElement {
                    key,
                    name,
                    attributes,
                } => {
                    if self.allocated.contains(key) {
                        return Err("duplicate key".to_string());
                    }
                    self.nodes.insert(
                        *key,
                        TestNode {
                            kind: TestKind::Element {
                                name: name.clone(),
                                attributes: attributes.clone(),
                                template_contents: None,
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                    self.allocated.insert(*key);
                }
                DomPatch::CreateTemplateContents { host, contents } => {
                    self.create_template_contents(*host, *contents)?;
                }
                DomPatch::CreateText { key, text } => {
                    if self.allocated.contains(key) {
                        return Err("duplicate key".to_string());
                    }
                    self.nodes.insert(
                        *key,
                        TestNode {
                            kind: TestKind::Text { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                    self.allocated.insert(*key);
                }
                DomPatch::CreateComment { key, text } => {
                    if self.allocated.contains(key) {
                        return Err("duplicate key".to_string());
                    }
                    self.nodes.insert(
                        *key,
                        TestNode {
                            kind: TestKind::Comment { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    );
                    self.allocated.insert(*key);
                }
                DomPatch::AppendChild { parent, child } => {
                    self.append_child(*parent, *child)?;
                }
                DomPatch::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    self.insert_before(*parent, *child, *before)?;
                }
                DomPatch::RemoveNode { key } => {
                    self.remove_subtree(*key)?;
                }
                DomPatch::SetAttributes { key, attributes } => {
                    let Some(node) = self.nodes.get_mut(key) else {
                        return Err("missing node".to_string());
                    };
                    if let TestKind::Element {
                        attributes: attrs, ..
                    } = &mut node.kind
                    {
                        *attrs = attributes.clone();
                    }
                }
                DomPatch::SetText { key, text } => {
                    let Some(node) = self.nodes.get_mut(key) else {
                        return Err("missing node".to_string());
                    };
                    match &mut node.kind {
                        TestKind::Text { text: existing } => *existing = text.clone(),
                        TestKind::Comment { .. }
                        | TestKind::Document { .. }
                        | TestKind::DocumentType { .. }
                        | TestKind::Element { .. }
                        | TestKind::DocumentFragment { .. } => {
                            return Err("SetText applied to non-text node".to_string());
                        }
                    }
                }
                DomPatch::AppendText { key, text } => {
                    let Some(node) = self.nodes.get_mut(key) else {
                        return Err("missing node".to_string());
                    };
                    match &mut node.kind {
                        TestKind::Text { text: existing } => existing.push_str(text),
                        TestKind::Comment { .. }
                        | TestKind::Document { .. }
                        | TestKind::DocumentType { .. }
                        | TestKind::Element { .. }
                        | TestKind::DocumentFragment { .. } => {
                            return Err("AppendText applied to non-text node".to_string());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn detach_child(&mut self, child: crate::dom_patch::PatchKey) -> ArenaResult<()> {
        let parent = self
            .nodes
            .get(&child)
            .ok_or_else(|| "missing child".to_string())?
            .parent;
        if let Some(parent) = parent
            && let Some(parent_node) = self.nodes.get_mut(&parent)
        {
            parent_node.children.retain(|key| *key != child);
        }
        let Some(child_node) = self.nodes.get_mut(&child) else {
            return Err("missing child".to_string());
        };
        child_node.parent = None;
        Ok(())
    }

    fn append_child(
        &mut self,
        parent: crate::dom_patch::PatchKey,
        child: crate::dom_patch::PatchKey,
    ) -> ArenaResult<()> {
        if parent == child {
            return Err("AppendChild cannot attach a node to itself".to_string());
        }
        self.ensure_container(parent, "AppendChild parent")?;
        if !self.nodes.contains_key(&child) {
            return Err("missing child".to_string());
        }
        if matches!(
            self.nodes.get(&child).map(|node| &node.kind),
            Some(TestKind::DocumentFragment { .. })
        ) {
            return Err("template contents cannot acquire an ordinary parent".to_string());
        }
        if self.is_document_node(child)? {
            return Err("AppendChild cannot move a document node".to_string());
        }
        if self.is_document_root_element(child)? {
            return Err("AppendChild cannot move the document root element".to_string());
        }
        if self.would_create_cycle(parent, child)? {
            return Err("AppendChild cannot create an ancestor cycle".to_string());
        }
        let already_last = self.node_parent(child)? == Some(parent)
            && self
                .nodes
                .get(&parent)
                .is_some_and(|node| node.children.last() == Some(&child));
        if already_last {
            return Ok(());
        }
        self.detach_child(child)?;
        let Some(parent_node) = self.nodes.get_mut(&parent) else {
            return Err("missing parent".to_string());
        };
        parent_node.children.push(child);
        let Some(child_node) = self.nodes.get_mut(&child) else {
            return Err("missing child".to_string());
        };
        child_node.parent = Some(parent);
        Ok(())
    }

    fn insert_before(
        &mut self,
        parent: crate::dom_patch::PatchKey,
        child: crate::dom_patch::PatchKey,
        before: crate::dom_patch::PatchKey,
    ) -> ArenaResult<()> {
        if parent == child {
            return Err("InsertBefore cannot attach a node to itself".to_string());
        }
        if child == before {
            return Err("InsertBefore cannot insert a node before itself".to_string());
        }
        self.ensure_container(parent, "InsertBefore parent")?;
        if !self.nodes.contains_key(&child) {
            return Err("missing child".to_string());
        }
        if matches!(
            self.nodes.get(&child).map(|node| &node.kind),
            Some(TestKind::DocumentFragment { .. })
        ) {
            return Err("template contents cannot acquire an ordinary parent".to_string());
        }
        if !self.nodes.contains_key(&before) {
            return Err("missing before".to_string());
        }
        if self.is_document_node(child)? {
            return Err("InsertBefore cannot move a document node".to_string());
        }
        if self.is_document_root_element(child)? {
            return Err("InsertBefore cannot move the document root element".to_string());
        }
        if self.node_parent(before)? != Some(parent) {
            return Err("missing before".to_string());
        }
        if self.would_create_cycle(parent, child)? {
            return Err("InsertBefore cannot create an ancestor cycle".to_string());
        }
        let already_in_place = if self.node_parent(child)? == Some(parent) {
            let siblings = &self
                .nodes
                .get(&parent)
                .ok_or_else(|| "missing parent".to_string())?
                .children;
            let child_index = siblings.iter().position(|key| *key == child);
            let before_index = siblings.iter().position(|key| *key == before);
            matches!((child_index, before_index), (Some(child_index), Some(before_index)) if child_index + 1 == before_index)
        } else {
            false
        };
        if already_in_place {
            return Ok(());
        }
        self.detach_child(child)?;
        let pos = {
            let Some(parent_node) = self.nodes.get(&parent) else {
                return Err("missing parent".to_string());
            };
            parent_node
                .children
                .iter()
                .position(|key| *key == before)
                .ok_or_else(|| "missing before".to_string())?
        };
        let Some(parent_node) = self.nodes.get_mut(&parent) else {
            return Err("missing parent".to_string());
        };
        parent_node.children.insert(pos, child);
        let Some(child_node) = self.nodes.get_mut(&child) else {
            return Err("missing child".to_string());
        };
        child_node.parent = Some(parent);
        Ok(())
    }

    fn create_template_contents(
        &mut self,
        host: crate::dom_patch::PatchKey,
        contents: crate::dom_patch::PatchKey,
    ) -> ArenaResult<()> {
        if host == contents || self.allocated.contains(&contents) {
            return Err("template contents key must be fresh and distinct".to_string());
        }
        let Some(host_node) = self.nodes.get(&host) else {
            return Err("missing template host".to_string());
        };
        match &host_node.kind {
            TestKind::Element {
                name,
                template_contents: None,
                ..
            } if name.is(crate::ElementNamespace::Html, "template") => {}
            TestKind::Element {
                template_contents: Some(_),
                ..
            } => return Err("template host already has contents".to_string()),
            _ => return Err("template contents host must be a template element".to_string()),
        }

        self.nodes.insert(
            contents,
            TestNode {
                kind: TestKind::DocumentFragment {
                    kind: ParserCreatedFragmentKind::TemplateContents,
                    host,
                },
                parent: None,
                children: Vec::new(),
            },
        );
        self.allocated.insert(contents);
        let Some(TestNode {
            kind: TestKind::Element {
                template_contents, ..
            },
            ..
        }) = self.nodes.get_mut(&host)
        else {
            unreachable!("validated template host disappeared during atomic association")
        };
        *template_contents = Some(contents);
        Ok(())
    }

    fn remove_subtree(&mut self, key: crate::dom_patch::PatchKey) -> ArenaResult<()> {
        self.remove_subtree_owned(key, false)
    }

    fn remove_subtree_owned(
        &mut self,
        key: crate::dom_patch::PatchKey,
        association_owned: bool,
    ) -> ArenaResult<()> {
        if matches!(
            self.nodes.get(&key).map(|node| &node.kind),
            Some(TestKind::DocumentFragment { .. })
        ) && !association_owned
        {
            return Err("hosted template contents cannot be removed directly".to_string());
        }
        let Some(node) = self.nodes.remove(&key) else {
            return Ok(());
        };
        if let Some(parent) = node.parent {
            if let Some(parent_node) = self.nodes.get_mut(&parent) {
                parent_node.children.retain(|k| *k != key);
            }
        } else if self.root == Some(key) {
            self.root = None;
        }
        let associated = match node.kind {
            TestKind::Element {
                template_contents, ..
            } => template_contents,
            TestKind::DocumentFragment { host, .. } => {
                if let Some(TestNode {
                    kind:
                        TestKind::Element {
                            template_contents, ..
                        },
                    ..
                }) = self.nodes.get_mut(&host)
                    && *template_contents == Some(key)
                {
                    *template_contents = None;
                }
                None
            }
            _ => None,
        };
        if let Some(contents) = associated {
            self.remove_subtree_owned(contents, true)?;
        }
        for child in node.children {
            self.remove_subtree_owned(child, false)?;
        }
        Ok(())
    }
}
