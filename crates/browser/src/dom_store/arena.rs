use super::error::DomPatchError;
use html::PatchKey;
use html::internal::{Id, ParserCreatedFragmentKind};
use html::{ExpandedElementName, ParserCreatedAttribute};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(crate) struct DomArena {
    pub(crate) nodes: Vec<NodeRecord>,
    pub(crate) live: HashMap<PatchKey, usize>,
    // Keys allocated since last `clear()`. Keys are intentionally non-reusable
    // until Clear, even after subtree removal.
    allocated: HashSet<PatchKey>,
}

impl DomArena {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            live: HashMap::new(),
            allocated: HashSet::new(),
        }
    }

    #[inline]
    fn debug_check_invariants(&self) {
        debug_assert!(
            self.live
                .keys()
                .all(|live_key| self.allocated.contains(live_key)),
            "arena invariant violated: live keys must be a subset of allocated keys"
        );
        debug_assert!(
            self.live.len() <= self.allocated.len(),
            "arena invariant violated: live set larger than allocated set"
        );
    }

    #[inline]
    pub(crate) fn debug_assert_structural_invariants(&self) {
        self.debug_check_invariants();
        #[cfg(debug_assertions)]
        {
            for (&key, &index) in &self.live {
                let node = &self.nodes[index];
                match &node.kind {
                    NodeKind::Element {
                        name,
                        template_contents: Some(contents),
                        ..
                    } => {
                        debug_assert!(name.is(html::ElementNamespace::Html, "template"));
                        let &contents_index = self.live.get(contents).expect(
                            "arena invariant violated: template contents key missing from live set",
                        );
                        debug_assert!(matches!(
                            self.nodes[contents_index].kind,
                            NodeKind::DocumentFragment {
                                kind: ParserCreatedFragmentKind::TemplateContents,
                                host,
                            } if host == key
                        ));
                    }
                    NodeKind::DocumentFragment { kind, host } => {
                        debug_assert_eq!(
                            *kind,
                            ParserCreatedFragmentKind::TemplateContents,
                            "arena invariant violated: unsupported template fragment kind"
                        );
                        debug_assert!(
                            node.parent.is_none(),
                            "arena invariant violated: template contents has ordinary parent"
                        );
                        let &host_index = self
                            .live
                            .get(host)
                            .expect("arena invariant violated: template host missing");
                        debug_assert!(matches!(
                            self.nodes[host_index].kind,
                            NodeKind::Element { template_contents: Some(contents), .. }
                                if contents == key
                        ));
                    }
                    _ => {}
                }
                if let Some(parent_key) = node.parent {
                    let &parent_index = self
                        .live
                        .get(&parent_key)
                        .expect("arena invariant violated: live parent key missing");
                    let parent = &self.nodes[parent_index];
                    let child_refs = parent
                        .children
                        .iter()
                        .filter(|child_key| **child_key == key)
                        .count();
                    debug_assert_eq!(
                        child_refs, 1,
                        "arena invariant violated: parent must reference child exactly once"
                    );
                }

                let mut seen_children = HashSet::new();
                for &child_key in &node.children {
                    let &child_index = self
                        .live
                        .get(&child_key)
                        .expect("arena invariant violated: child key missing from live set");
                    debug_assert!(
                        seen_children.insert(child_key),
                        "arena invariant violated: duplicate child reference under one parent"
                    );
                    debug_assert_eq!(
                        self.nodes[child_index].parent,
                        Some(key),
                        "arena invariant violated: child parent backref mismatch"
                    );
                }
            }

            for &root_key in self.live.keys() {
                let mut visiting = HashSet::new();
                let mut visited = HashSet::new();
                self.debug_assert_acyclic_from(root_key, &mut visiting, &mut visited);
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        self.debug_check_invariants();
        self.nodes.clear();
        self.live.clear();
        self.allocated.clear();
        self.debug_check_invariants();
    }

    /// Map a live patch-layer node identity to the runtime DOM node identity
    /// materialization exposes today.
    ///
    /// The current bridge is numeric (`PatchKey(n) -> Id(n)`), but that
    /// coupling is owned by the DOM materialization layer, not by rendering or
    /// page invalidation code.
    pub(crate) fn materialized_node_id_for_key(&self, key: PatchKey) -> Result<Id, DomPatchError> {
        self.debug_check_invariants();
        if !self.live.contains_key(&key) {
            return Err(DomPatchError::MissingKey(key));
        }
        Ok(Id(key.0))
    }

    pub(crate) fn insert_node(
        &mut self,
        key: PatchKey,
        kind: NodeKind,
    ) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        if self.allocated.contains(&key) {
            return Err(DomPatchError::DuplicateKey(key));
        }
        let index = self.nodes.len();
        self.nodes.push(NodeRecord {
            kind,
            parent: None,
            children: Vec::new(),
        });
        self.allocated.insert(key);
        self.live.insert(key, index);
        self.debug_check_invariants();
        Ok(())
    }

    pub(crate) fn create_template_contents(
        &mut self,
        host: PatchKey,
        contents: PatchKey,
    ) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        if contents == PatchKey::INVALID {
            return Err(DomPatchError::InvalidKey(contents));
        }
        if self.allocated.contains(&contents) {
            return Err(DomPatchError::DuplicateKey(contents));
        }
        let host_index = *self
            .live
            .get(&host)
            .ok_or(DomPatchError::MissingKey(host))?;
        match &self.nodes[host_index].kind {
            NodeKind::Element {
                name,
                template_contents: None,
                ..
            } if name.is(html::ElementNamespace::Html, "template") => {}
            NodeKind::Element {
                template_contents: Some(_),
                ..
            } => {
                return Err(DomPatchError::Protocol(
                    "template host already has contents",
                ));
            }
            NodeKind::Element { .. } => {
                return Err(DomPatchError::Protocol(
                    "template contents host must have canonical template name",
                ));
            }
            _ => {
                return Err(DomPatchError::WrongNodeKind {
                    key: host,
                    expected: "Element",
                    actual: self.nodes[host_index].kind_name(),
                });
            }
        }
        self.insert_node(
            contents,
            NodeKind::DocumentFragment {
                kind: ParserCreatedFragmentKind::TemplateContents,
                host,
            },
        )?;
        let NodeKind::Element {
            template_contents, ..
        } = &mut self.nodes[host_index].kind
        else {
            unreachable!("validated template host changed kind")
        };
        *template_contents = Some(contents);
        self.debug_check_invariants();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_fragment_kind_for_test(
        &mut self,
        key: PatchKey,
        kind: ParserCreatedFragmentKind,
    ) {
        let index = *self.live.get(&key).expect("test fragment key must be live");
        let NodeKind::DocumentFragment {
            kind: fragment_kind,
            ..
        } = &mut self.nodes[index].kind
        else {
            panic!("test fragment-kind mutation requires a document fragment")
        };
        *fragment_kind = kind;
    }

    pub(crate) fn append_child(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
    ) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        if parent == child {
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        let parent_index = *self
            .live
            .get(&parent)
            .ok_or(DomPatchError::MissingKey(parent))?;
        let child_index = *self
            .live
            .get(&child)
            .ok_or(DomPatchError::MissingKey(child))?;
        if !self.nodes[parent_index].allows_children() {
            return Err(DomPatchError::InvalidParent(parent));
        }
        if self.is_document_node(child_index) {
            return Err(DomPatchError::IllegalMove {
                key: child,
                reason: "document nodes cannot be moved",
            });
        }
        if matches!(
            self.nodes[child_index].kind,
            NodeKind::DocumentFragment { .. }
        ) {
            return Err(DomPatchError::IllegalMove {
                key: child,
                reason: "template contents roots cannot acquire ordinary parents",
            });
        }
        if self.is_document_root_element(child_index) {
            return Err(DomPatchError::IllegalMove {
                key: child,
                reason: "document root element cannot be moved",
            });
        }
        if self.contains_in_subtree(child, parent) {
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        if self.nodes[child_index].parent == Some(parent)
            && self.nodes[parent_index].children.last() == Some(&child)
        {
            self.debug_check_invariants();
            return Ok(());
        }
        self.detach_child(child_index, child);
        self.nodes[parent_index].children.push(child);
        self.nodes[child_index].parent = Some(parent);
        self.debug_check_invariants();
        Ok(())
    }

    pub(crate) fn insert_before(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        if parent == child {
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        let parent_index = *self
            .live
            .get(&parent)
            .ok_or(DomPatchError::MissingKey(parent))?;
        let child_index = *self
            .live
            .get(&child)
            .ok_or(DomPatchError::MissingKey(child))?;
        if !self.nodes[parent_index].allows_children() {
            return Err(DomPatchError::InvalidParent(parent));
        }
        let before_index = *self
            .live
            .get(&before)
            .ok_or(DomPatchError::MissingKey(before))?;
        if self.nodes[before_index].parent != Some(parent) {
            return Err(DomPatchError::InvalidSibling { parent, before });
        }
        if self.is_document_node(child_index) {
            return Err(DomPatchError::IllegalMove {
                key: child,
                reason: "document nodes cannot be moved",
            });
        }
        if matches!(
            self.nodes[child_index].kind,
            NodeKind::DocumentFragment { .. }
        ) {
            return Err(DomPatchError::IllegalMove {
                key: child,
                reason: "template contents roots cannot acquire ordinary parents",
            });
        }
        if self.is_document_root_element(child_index) {
            return Err(DomPatchError::IllegalMove {
                key: child,
                reason: "document root element cannot be moved",
            });
        }
        if self.contains_in_subtree(child, parent) {
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        if self.nodes[child_index].parent == Some(parent) {
            let siblings = &self.nodes[parent_index].children;
            let child_pos = siblings.iter().position(|key| *key == child);
            let before_pos = siblings.iter().position(|key| *key == before);
            if matches!((child_pos, before_pos), (Some(child_pos), Some(before_pos)) if child_pos + 1 == before_pos)
            {
                self.debug_check_invariants();
                return Ok(());
            }
        }
        self.detach_child(child_index, child);
        let siblings = &mut self.nodes[parent_index].children;
        let pos = siblings
            .iter()
            .position(|k| *k == before)
            .ok_or(DomPatchError::InvalidSibling { parent, before })?;
        siblings.insert(pos, child);
        self.nodes[child_index].parent = Some(parent);
        self.debug_check_invariants();
        Ok(())
    }

    #[allow(clippy::collapsible_if)]
    pub(crate) fn remove_subtree(&mut self, key: PatchKey) -> Result<(), DomPatchError> {
        self.remove_subtree_owned(key, false)
    }

    #[allow(clippy::collapsible_if)]
    fn remove_subtree_owned(
        &mut self,
        key: PatchKey,
        association_owned: bool,
    ) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        let index = *self.live.get(&key).ok_or(DomPatchError::MissingKey(key))?;
        let fragment_host = match self.nodes[index].kind {
            NodeKind::DocumentFragment { host, .. } => Some(host),
            _ => None,
        };
        if fragment_host.is_some() && !association_owned {
            return Err(DomPatchError::IllegalMove {
                key,
                reason: "hosted template contents roots cannot be removed directly",
            });
        }
        let template_contents = match self.nodes[index].kind {
            NodeKind::Element {
                template_contents, ..
            } => template_contents,
            _ => None,
        };
        if let Some(parent) = self.nodes[index].parent.take() {
            if let Some(parent_index) = self.live.get(&parent).copied() {
                let siblings = &mut self.nodes[parent_index].children;
                siblings.retain(|k| *k != key);
            }
        }
        let children = self.nodes[index].children.clone();
        self.nodes[index].children.clear();
        self.live.remove(&key);
        for child in children {
            if self.live.contains_key(&child) {
                self.remove_subtree_owned(child, false)?;
            }
        }
        if let Some(contents) = template_contents
            && self.live.contains_key(&contents)
        {
            self.remove_subtree_owned(contents, true)?;
        }
        if let Some(host) = fragment_host
            && let Some(&host_index) = self.live.get(&host)
            && let NodeKind::Element {
                template_contents, ..
            } = &mut self.nodes[host_index].kind
        {
            *template_contents = None;
        }
        self.debug_check_invariants();
        Ok(())
    }

    pub(crate) fn set_attributes(
        &mut self,
        key: PatchKey,
        attributes: &[ParserCreatedAttribute],
    ) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        let index = *self.live.get(&key).ok_or(DomPatchError::MissingKey(key))?;
        let actual = self.nodes[index].kind_name();
        match &mut self.nodes[index].kind {
            NodeKind::Element {
                attributes: attrs, ..
            } => {
                attrs.clear();
                attrs.extend(attributes.iter().cloned());
                self.debug_check_invariants();
                Ok(())
            }
            _ => Err(DomPatchError::WrongNodeKind {
                key,
                expected: "Element",
                actual,
            }),
        }
    }

    pub(crate) fn set_text(&mut self, key: PatchKey, text: &str) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        let index = *self.live.get(&key).ok_or(DomPatchError::MissingKey(key))?;
        let actual = self.nodes[index].kind_name();
        match &mut self.nodes[index].kind {
            NodeKind::Text { text: existing } => {
                existing.clear();
                existing.push_str(text);
                self.debug_check_invariants();
                Ok(())
            }
            _ => Err(DomPatchError::WrongNodeKind {
                key,
                expected: "Text",
                actual,
            }),
        }
    }

    pub(crate) fn append_text(&mut self, key: PatchKey, text: &str) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        let index = *self.live.get(&key).ok_or(DomPatchError::MissingKey(key))?;
        let actual = self.nodes[index].kind_name();
        match &mut self.nodes[index].kind {
            NodeKind::Text { text: existing } => {
                existing.push_str(text);
                self.debug_check_invariants();
                Ok(())
            }
            _ => Err(DomPatchError::WrongNodeKind {
                key,
                expected: "Text",
                actual,
            }),
        }
    }

    fn is_document_node(&self, index: usize) -> bool {
        matches!(self.nodes[index].kind, NodeKind::Document { .. })
    }

    fn is_document_root_element(&self, index: usize) -> bool {
        let Some(parent) = self.nodes[index].parent else {
            return false;
        };
        let Some(&parent_index) = self.live.get(&parent) else {
            return false;
        };
        matches!(
            (&self.nodes[index].kind, &self.nodes[parent_index].kind),
            (NodeKind::Element { .. }, NodeKind::Document { .. })
        )
    }

    fn detach_child(&mut self, child_index: usize, child: PatchKey) {
        if let Some(existing_parent) = self.nodes[child_index].parent
            && let Some(&parent_index) = self.live.get(&existing_parent)
        {
            self.nodes[parent_index]
                .children
                .retain(|key| *key != child);
        }
        self.nodes[child_index].parent = None;
    }

    fn contains_in_subtree(&self, root: PatchKey, needle: PatchKey) -> bool {
        let Some(&index) = self.live.get(&root) else {
            return false;
        };
        let mut stack = Vec::new();
        stack.extend(self.nodes[index].children.iter().copied());
        if let NodeKind::Element {
            template_contents: Some(contents),
            ..
        } = self.nodes[index].kind
        {
            stack.push(contents);
        }
        while let Some(current) = stack.pop() {
            if current == needle {
                return true;
            }
            if let Some(&child_index) = self.live.get(&current) {
                stack.extend(self.nodes[child_index].children.iter().copied());
                if let NodeKind::Element {
                    template_contents: Some(contents),
                    ..
                } = self.nodes[child_index].kind
                {
                    stack.push(contents);
                }
            }
        }
        false
    }

    #[cfg(debug_assertions)]
    fn debug_assert_acyclic_from(
        &self,
        key: PatchKey,
        visiting: &mut HashSet<PatchKey>,
        visited: &mut HashSet<PatchKey>,
    ) {
        if visited.contains(&key) {
            return;
        }
        debug_assert!(
            visiting.insert(key),
            "arena invariant violated: cycle detected while walking live subtree"
        );
        let &index = self
            .live
            .get(&key)
            .expect("arena invariant violated: traversal reached missing live node");
        for &child in &self.nodes[index].children {
            self.debug_assert_acyclic_from(child, visiting, visited);
        }
        if let NodeKind::Element {
            template_contents: Some(contents),
            ..
        } = self.nodes[index].kind
        {
            self.debug_assert_acyclic_from(contents, visiting, visited);
        }
        visiting.remove(&key);
        visited.insert(key);
    }
}

#[derive(Clone)]
pub(crate) struct NodeRecord {
    pub(crate) kind: NodeKind,
    pub(crate) parent: Option<PatchKey>,
    pub(crate) children: Vec<PatchKey>,
}

impl NodeRecord {
    fn allows_children(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::Document { .. }
                | NodeKind::Element { .. }
                | NodeKind::DocumentFragment { .. }
        )
    }

    pub(crate) fn kind_name(&self) -> &'static str {
        match self.kind {
            NodeKind::Document { .. } => "Document",
            NodeKind::DocumentType { .. } => "DocumentType",
            NodeKind::Element { .. } => "Element",
            NodeKind::DocumentFragment { .. } => "DocumentFragment",
            NodeKind::Text { .. } => "Text",
            NodeKind::Comment { .. } => "Comment",
            NodeKind::ProcessingInstruction { .. } => "ProcessingInstruction",
        }
    }
}

#[derive(Clone)]
pub(crate) enum NodeKind {
    Document {
        doctype: Option<String>,
    },
    DocumentType {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
    },
    Element {
        name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        template_contents: Option<PatchKey>,
    },
    DocumentFragment {
        kind: ParserCreatedFragmentKind,
        host: PatchKey,
    },
    Text {
        text: String,
    },
    Comment {
        text: String,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
}
