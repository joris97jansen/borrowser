use super::error::DomPatchError;
use html::PatchKey;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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

    pub(crate) fn clear(&mut self) {
        self.debug_check_invariants();
        self.nodes.clear();
        self.live.clear();
        self.allocated.clear();
        self.debug_check_invariants();
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
        self.debug_check_invariants();
        let index = *self.live.get(&key).ok_or(DomPatchError::MissingKey(key))?;
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
                self.remove_subtree(child)?;
            }
        }
        self.debug_check_invariants();
        Ok(())
    }

    pub(crate) fn set_attributes(
        &mut self,
        key: PatchKey,
        attributes: &[(Arc<str>, Option<String>)],
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
        while let Some(current) = stack.pop() {
            if current == needle {
                return true;
            }
            if let Some(&child_index) = self.live.get(&current) {
                stack.extend(self.nodes[child_index].children.iter().copied());
            }
        }
        false
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
            NodeKind::Document { .. } | NodeKind::Element { .. }
        )
    }

    fn kind_name(&self) -> &'static str {
        match self.kind {
            NodeKind::Document { .. } => "Document",
            NodeKind::Element { .. } => "Element",
            NodeKind::Text { .. } => "Text",
            NodeKind::Comment { .. } => "Comment",
        }
    }
}

#[derive(Clone)]
pub(crate) enum NodeKind {
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
