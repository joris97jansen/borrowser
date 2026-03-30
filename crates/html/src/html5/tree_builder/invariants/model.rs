use crate::dom_patch::PatchKey;

use super::errors::PatchInvariantError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomInvariantNodeKind {
    Document,
    Element,
    Text,
    Comment,
}

impl DomInvariantNodeKind {
    #[must_use]
    pub fn is_container(self) -> bool {
        matches!(self, Self::Document | Self::Element)
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Element => "element",
            Self::Text => "text",
            Self::Comment => "comment",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomInvariantNode {
    pub(in crate::html5::tree_builder) kind: DomInvariantNodeKind,
    pub(in crate::html5::tree_builder) parent: Option<PatchKey>,
    pub(in crate::html5::tree_builder) children: Vec<PatchKey>,
}

impl DomInvariantNode {
    #[must_use]
    pub fn kind(&self) -> DomInvariantNodeKind {
        self.kind
    }

    #[must_use]
    pub fn parent(&self) -> Option<PatchKey> {
        self.parent
    }

    #[must_use]
    pub fn children(&self) -> &[PatchKey] {
        self.children.as_slice()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomInvariantState {
    pub(in crate::html5::tree_builder) nodes: Vec<Option<DomInvariantNode>>,
    pub(in crate::html5::tree_builder) root: Option<PatchKey>,
}

impl DomInvariantState {
    #[must_use]
    pub fn root(&self) -> Option<PatchKey> {
        self.root
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.root.is_none() && self.nodes.iter().all(Option::is_none)
    }

    #[must_use]
    pub fn contains(&self, key: PatchKey) -> bool {
        self.node(key).is_some()
    }

    #[must_use]
    pub fn node(&self, key: PatchKey) -> Option<&DomInvariantNode> {
        self.nodes.get(key.0 as usize).and_then(Option::as_ref)
    }

    pub(super) fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
    }

    fn ensure_slot(&mut self, key: PatchKey) {
        let index = key.0 as usize;
        if self.nodes.len() <= index {
            self.nodes.resize_with(index + 1, || None);
        }
    }

    fn node_kind(&self, key: PatchKey) -> Option<DomInvariantNodeKind> {
        self.node(key).map(DomInvariantNode::kind)
    }

    fn node_parent(&self, key: PatchKey) -> Option<Option<PatchKey>> {
        self.node(key).map(DomInvariantNode::parent)
    }

    pub(super) fn insert_created_node(
        &mut self,
        key: PatchKey,
        kind: DomInvariantNodeKind,
        patch_index: usize,
    ) -> Result<(), PatchInvariantError> {
        if key == PatchKey::INVALID {
            return Err(PatchInvariantError::InvalidKey {
                patch_index,
                operation: create_operation_name(kind),
                role: "key",
                key,
            });
        }
        if self.contains(key) {
            return Err(PatchInvariantError::DuplicateCreate { patch_index, key });
        }
        if matches!(kind, DomInvariantNodeKind::Document) && self.root.is_some() {
            return Err(PatchInvariantError::DuplicateDocumentRoot {
                patch_index,
                existing_root: self.root.expect("checked is_some above"),
                new_root: key,
            });
        }
        self.ensure_slot(key);
        self.nodes[key.0 as usize] = Some(DomInvariantNode {
            kind,
            parent: None,
            children: Vec::new(),
        });
        if matches!(kind, DomInvariantNodeKind::Document) {
            self.root = Some(key);
        }
        Ok(())
    }

    fn ensure_patch_node(
        &self,
        key: PatchKey,
        patch_index: usize,
        operation: &'static str,
        role: &'static str,
    ) -> Result<DomInvariantNodeKind, PatchInvariantError> {
        if key == PatchKey::INVALID {
            return Err(PatchInvariantError::InvalidKey {
                patch_index,
                operation,
                role,
                key,
            });
        }
        self.node_kind(key)
            .ok_or(PatchInvariantError::MissingNodeReference {
                patch_index,
                operation,
                role,
                key,
            })
    }

    fn ensure_patch_container(
        &self,
        key: PatchKey,
        patch_index: usize,
        operation: &'static str,
        role: &'static str,
    ) -> Result<(), PatchInvariantError> {
        let kind = self.ensure_patch_node(key, patch_index, operation, role)?;
        if kind.is_container() {
            return Ok(());
        }
        Err(PatchInvariantError::ContainerRequired {
            patch_index,
            operation,
            key,
            actual: kind,
        })
    }

    fn is_document_node(&self, key: PatchKey) -> bool {
        self.node_kind(key)
            .is_some_and(|kind| matches!(kind, DomInvariantNodeKind::Document))
    }

    fn is_document_root_element(&self, key: PatchKey) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        self.node(key).is_some_and(|node| {
            node.parent == Some(root) && matches!(node.kind, DomInvariantNodeKind::Element)
        })
    }

    fn would_create_cycle(&self, parent: PatchKey, child: PatchKey) -> bool {
        let mut cursor = Some(parent);
        while let Some(current) = cursor {
            if current == child {
                return true;
            }
            cursor = self.node_parent(current).flatten();
        }
        false
    }

    fn detach_child(&mut self, child: PatchKey) -> Result<(), PatchInvariantError> {
        let parent = self
            .node_parent(child)
            .flatten()
            .expect("detach_child requires an existing child node");
        let Some(parent_node) = self
            .nodes
            .get_mut(parent.0 as usize)
            .and_then(Option::as_mut)
        else {
            return Err(PatchInvariantError::Internal(
                "detach_child parent disappeared during patch checking",
            ));
        };
        let mut first_index = None;
        let mut match_count = 0usize;
        for (index, existing) in parent_node.children.iter().copied().enumerate() {
            if existing == child {
                match_count += 1;
                if first_index.is_none() {
                    first_index = Some(index);
                }
            }
        }
        let Some(index) = first_index else {
            return Err(PatchInvariantError::Internal(
                "detach_child child missing from parent children",
            ));
        };
        if match_count != 1 {
            return Err(PatchInvariantError::Internal(
                "detach_child found duplicate child references in parent",
            ));
        }
        parent_node.children.remove(index);
        let Some(child_node) = self
            .nodes
            .get_mut(child.0 as usize)
            .and_then(Option::as_mut)
        else {
            return Err(PatchInvariantError::Internal(
                "detach_child target disappeared during patch checking",
            ));
        };
        child_node.parent = None;
        Ok(())
    }

    pub(super) fn apply_append_child(
        &mut self,
        patch_index: usize,
        parent: PatchKey,
        child: PatchKey,
    ) -> Result<(), PatchInvariantError> {
        const OPERATION: &str = "AppendChild";

        if parent == child {
            return Err(PatchInvariantError::SelfAttachment {
                patch_index,
                operation: OPERATION,
                parent,
                child,
            });
        }
        self.ensure_patch_container(parent, patch_index, OPERATION, "parent")?;
        self.ensure_patch_node(child, patch_index, OPERATION, "child")?;
        if self.is_document_node(child) {
            return Err(PatchInvariantError::DocumentMove {
                patch_index,
                operation: OPERATION,
                key: child,
            });
        }
        if self.is_document_root_element(child) {
            return Err(PatchInvariantError::DocumentRootMove {
                patch_index,
                operation: OPERATION,
                key: child,
            });
        }
        if self.would_create_cycle(parent, child) {
            return Err(PatchInvariantError::CycleCreation {
                patch_index,
                operation: OPERATION,
                parent,
                child,
            });
        }

        let already_last = self.node_parent(child) == Some(Some(parent))
            && self
                .node(parent)
                .and_then(|node| node.children.last().copied())
                .is_some_and(|last| last == child);
        if already_last {
            return Ok(());
        }

        if self.node_parent(child).flatten().is_some() {
            self.detach_child(child)?;
        }
        let Some(parent_node) = self
            .nodes
            .get_mut(parent.0 as usize)
            .and_then(Option::as_mut)
        else {
            return Err(PatchInvariantError::Internal(
                "AppendChild parent disappeared",
            ));
        };
        parent_node.children.push(child);
        let Some(child_node) = self
            .nodes
            .get_mut(child.0 as usize)
            .and_then(Option::as_mut)
        else {
            return Err(PatchInvariantError::Internal(
                "AppendChild child disappeared",
            ));
        };
        child_node.parent = Some(parent);
        Ok(())
    }

    pub(super) fn apply_insert_before(
        &mut self,
        patch_index: usize,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) -> Result<(), PatchInvariantError> {
        const OPERATION: &str = "InsertBefore";

        if parent == child {
            return Err(PatchInvariantError::SelfAttachment {
                patch_index,
                operation: OPERATION,
                parent,
                child,
            });
        }
        if child == before {
            return Err(PatchInvariantError::InsertBeforeSelf { patch_index, child });
        }
        self.ensure_patch_container(parent, patch_index, OPERATION, "parent")?;
        self.ensure_patch_node(child, patch_index, OPERATION, "child")?;
        self.ensure_patch_node(before, patch_index, OPERATION, "before")?;
        if self.is_document_node(child) {
            return Err(PatchInvariantError::DocumentMove {
                patch_index,
                operation: OPERATION,
                key: child,
            });
        }
        if self.is_document_root_element(child) {
            return Err(PatchInvariantError::DocumentRootMove {
                patch_index,
                operation: OPERATION,
                key: child,
            });
        }
        if self.node_parent(before) != Some(Some(parent)) {
            return Err(PatchInvariantError::InsertBeforeParentMismatch {
                patch_index,
                parent,
                before,
                actual_parent: self.node_parent(before).flatten(),
            });
        }
        if self.would_create_cycle(parent, child) {
            return Err(PatchInvariantError::CycleCreation {
                patch_index,
                operation: OPERATION,
                parent,
                child,
            });
        }

        let already_in_place = if self.node_parent(child) == Some(Some(parent)) {
            let Some(siblings) = self.node(parent).map(DomInvariantNode::children) else {
                return Err(PatchInvariantError::Internal(
                    "InsertBefore parent disappeared",
                ));
            };
            let child_index = siblings.iter().position(|key| *key == child);
            let before_index = siblings.iter().position(|key| *key == before);
            matches!((child_index, before_index), (Some(child_index), Some(before_index)) if child_index + 1 == before_index)
        } else {
            false
        };
        if already_in_place {
            return Ok(());
        }

        if self.node_parent(child).flatten().is_some() {
            self.detach_child(child)?;
        }
        let before_index = self
            .node(parent)
            .and_then(|node| node.children.iter().position(|key| *key == before))
            .ok_or(PatchInvariantError::Internal(
                "InsertBefore before child disappeared from parent",
            ))?;
        let Some(parent_node) = self
            .nodes
            .get_mut(parent.0 as usize)
            .and_then(Option::as_mut)
        else {
            return Err(PatchInvariantError::Internal(
                "InsertBefore parent disappeared",
            ));
        };
        parent_node.children.insert(before_index, child);
        let Some(child_node) = self
            .nodes
            .get_mut(child.0 as usize)
            .and_then(Option::as_mut)
        else {
            return Err(PatchInvariantError::Internal(
                "InsertBefore child disappeared",
            ));
        };
        child_node.parent = Some(parent);
        Ok(())
    }

    pub(super) fn apply_remove_node(
        &mut self,
        patch_index: usize,
        key: PatchKey,
    ) -> Result<(), PatchInvariantError> {
        const OPERATION: &str = "RemoveNode";

        self.ensure_patch_node(key, patch_index, OPERATION, "key")?;
        let is_root = self.root == Some(key);
        let is_attached = self.node_parent(key).flatten().is_some();
        if !is_root && !is_attached {
            return Err(PatchInvariantError::RemoveDetachedNode { patch_index, key });
        }
        if is_attached {
            self.detach_child(key)?;
        }

        let mut stack = vec![key];
        while let Some(current) = stack.pop() {
            let children = self.node(current).map(|node| node.children.clone()).ok_or(
                PatchInvariantError::Internal("RemoveNode target disappeared during subtree walk"),
            )?;
            stack.extend(children.into_iter());
            if self.root == Some(current) {
                self.root = None;
            }
            let Some(slot) = self.nodes.get_mut(current.0 as usize) else {
                return Err(PatchInvariantError::Internal(
                    "RemoveNode slot disappeared during subtree walk",
                ));
            };
            *slot = None;
        }
        Ok(())
    }

    pub(super) fn apply_kind_checked_patch(
        &self,
        patch_index: usize,
        key: PatchKey,
        operation: &'static str,
        expected: DomInvariantNodeKind,
    ) -> Result<(), PatchInvariantError> {
        let actual = self.ensure_patch_node(key, patch_index, operation, "key")?;
        if actual == expected {
            return Ok(());
        }
        Err(PatchInvariantError::WrongNodeKind {
            patch_index,
            operation,
            key,
            expected,
            actual,
        })
    }
}

#[must_use]
fn create_operation_name(kind: DomInvariantNodeKind) -> &'static str {
    match kind {
        DomInvariantNodeKind::Document => "CreateDocument",
        DomInvariantNodeKind::Element => "CreateElement",
        DomInvariantNodeKind::Text => "CreateText",
        DomInvariantNodeKind::Comment => "CreateComment",
    }
}
