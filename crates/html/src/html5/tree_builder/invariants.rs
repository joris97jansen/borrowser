use crate::dom_patch::{DomPatch, PatchKey};
use std::collections::HashSet;

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

    fn clear(&mut self) {
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

    fn insert_created_node(
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

    fn apply_append_child(
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

    fn apply_insert_before(
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

    fn apply_remove_node(
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

    fn apply_kind_checked_patch(
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomInvariantError {
    MissingRootForNonEmptyState,
    RootNodeMissing {
        root: PatchKey,
    },
    RootHasParent {
        root: PatchKey,
        parent: PatchKey,
    },
    RootIsNotDocument {
        root: PatchKey,
        actual: DomInvariantNodeKind,
    },
    DocumentNodeNotRoot {
        key: PatchKey,
        actual_parent: Option<PatchKey>,
    },
    DetachedNonRootNode {
        key: PatchKey,
    },
    DanglingParentReference {
        key: PatchKey,
        parent: PatchKey,
    },
    ParentChildMismatch {
        key: PatchKey,
        parent: PatchKey,
        matches: usize,
    },
    DanglingChildReference {
        parent: PatchKey,
        child: PatchKey,
    },
    DuplicateChildReference {
        parent: PatchKey,
        child: PatchKey,
    },
    ChildParentMismatch {
        parent: PatchKey,
        child: PatchKey,
        actual_parent: Option<PatchKey>,
    },
    CycleDetected {
        key: PatchKey,
    },
}

impl std::fmt::Display for DomInvariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRootForNonEmptyState => {
                f.write_str("DOM invariant failed: non-empty state is missing a root")
            }
            Self::RootNodeMissing { root } => {
                write!(f, "DOM invariant failed: root node {root:?} is missing")
            }
            Self::RootHasParent { root, parent } => write!(
                f,
                "DOM invariant failed: root node {root:?} unexpectedly has parent {parent:?}"
            ),
            Self::RootIsNotDocument { root, actual } => write!(
                f,
                "DOM invariant failed: root node {root:?} must be a document, found {}",
                actual.as_str()
            ),
            Self::DocumentNodeNotRoot { key, actual_parent } => write!(
                f,
                "DOM invariant failed: document node {key:?} is not the declared root (parent={actual_parent:?})"
            ),
            Self::DetachedNonRootNode { key } => {
                write!(
                    f,
                    "DOM invariant failed: node {key:?} is detached but not the root"
                )
            }
            Self::DanglingParentReference { key, parent } => write!(
                f,
                "DOM invariant failed: node {key:?} points to missing parent {parent:?}"
            ),
            Self::ParentChildMismatch {
                key,
                parent,
                matches,
            } => write!(
                f,
                "DOM invariant failed: node {key:?} expected exactly one entry under parent {parent:?}, found {matches}"
            ),
            Self::DanglingChildReference { parent, child } => write!(
                f,
                "DOM invariant failed: parent {parent:?} points to missing child {child:?}"
            ),
            Self::DuplicateChildReference { parent, child } => write!(
                f,
                "DOM invariant failed: parent {parent:?} contains duplicate child {child:?}"
            ),
            Self::ChildParentMismatch {
                parent,
                child,
                actual_parent,
            } => write!(
                f,
                "DOM invariant failed: child {child:?} under parent {parent:?} has parent back-reference {actual_parent:?}"
            ),
            Self::CycleDetected { key } => {
                write!(f, "DOM invariant failed: cycle detected at node {key:?}")
            }
        }
    }
}

impl std::error::Error for DomInvariantError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchInvariantError {
    InvalidBaseline(DomInvariantError),
    InvalidKey {
        patch_index: usize,
        operation: &'static str,
        role: &'static str,
        key: PatchKey,
    },
    DuplicateCreate {
        patch_index: usize,
        key: PatchKey,
    },
    DuplicateDocumentRoot {
        patch_index: usize,
        existing_root: PatchKey,
        new_root: PatchKey,
    },
    MissingNodeReference {
        patch_index: usize,
        operation: &'static str,
        role: &'static str,
        key: PatchKey,
    },
    ContainerRequired {
        patch_index: usize,
        operation: &'static str,
        key: PatchKey,
        actual: DomInvariantNodeKind,
    },
    SelfAttachment {
        patch_index: usize,
        operation: &'static str,
        parent: PatchKey,
        child: PatchKey,
    },
    InsertBeforeSelf {
        patch_index: usize,
        child: PatchKey,
    },
    InsertBeforeParentMismatch {
        patch_index: usize,
        parent: PatchKey,
        before: PatchKey,
        actual_parent: Option<PatchKey>,
    },
    DocumentMove {
        patch_index: usize,
        operation: &'static str,
        key: PatchKey,
    },
    DocumentRootMove {
        patch_index: usize,
        operation: &'static str,
        key: PatchKey,
    },
    CycleCreation {
        patch_index: usize,
        operation: &'static str,
        parent: PatchKey,
        child: PatchKey,
    },
    RemoveDetachedNode {
        patch_index: usize,
        key: PatchKey,
    },
    WrongNodeKind {
        patch_index: usize,
        operation: &'static str,
        key: PatchKey,
        expected: DomInvariantNodeKind,
        actual: DomInvariantNodeKind,
    },
    ClearMustBeFirst {
        patch_index: usize,
    },
    ClearBatchMustReestablishDocument,
    FinalDomInvariantViolation(DomInvariantError),
    Internal(&'static str),
}

impl std::fmt::Display for PatchInvariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBaseline(source) => {
                write!(
                    f,
                    "patch invariant failed: invalid baseline DOM state: {source}"
                )
            }
            Self::InvalidKey {
                patch_index,
                operation,
                role,
                key,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): invalid {role} key {key:?}"
            ),
            Self::DuplicateCreate { patch_index, key } => write!(
                f,
                "patch invariant failed at patch #{patch_index}: duplicate create for {key:?}"
            ),
            Self::DuplicateDocumentRoot {
                patch_index,
                existing_root,
                new_root,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index}: duplicate document roots {existing_root:?} and {new_root:?}"
            ),
            Self::MissingNodeReference {
                patch_index,
                operation,
                role,
                key,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): missing {role} node {key:?}"
            ),
            Self::ContainerRequired {
                patch_index,
                operation,
                key,
                actual,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): node {key:?} must be a container, found {}",
                actual.as_str()
            ),
            Self::SelfAttachment {
                patch_index,
                operation,
                parent,
                child,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): cannot attach {child:?} to itself via parent {parent:?}"
            ),
            Self::InsertBeforeSelf { patch_index, child } => write!(
                f,
                "patch invariant failed at patch #{patch_index} (InsertBefore): child {child:?} cannot be inserted before itself"
            ),
            Self::InsertBeforeParentMismatch {
                patch_index,
                parent,
                before,
                actual_parent,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} (InsertBefore): before={before:?} is under {actual_parent:?}, expected {parent:?}"
            ),
            Self::DocumentMove {
                patch_index,
                operation,
                key,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): document node {key:?} cannot be moved"
            ),
            Self::DocumentRootMove {
                patch_index,
                operation,
                key,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): document root element {key:?} cannot be moved"
            ),
            Self::CycleCreation {
                patch_index,
                operation,
                parent,
                child,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): attaching {child:?} under {parent:?} would create a cycle"
            ),
            Self::RemoveDetachedNode { patch_index, key } => write!(
                f,
                "patch invariant failed at patch #{patch_index} (RemoveNode): node {key:?} is detached"
            ),
            Self::WrongNodeKind {
                patch_index,
                operation,
                key,
                expected,
                actual,
            } => write!(
                f,
                "patch invariant failed at patch #{patch_index} ({operation}): node {key:?} must be {}, found {}",
                expected.as_str(),
                actual.as_str()
            ),
            Self::ClearMustBeFirst { patch_index } => write!(
                f,
                "patch invariant failed at patch #{patch_index}: Clear may only appear as the first patch in a batch"
            ),
            Self::ClearBatchMustReestablishDocument => f.write_str(
                "patch invariant failed: Clear batches must re-establish a rooted document",
            ),
            Self::FinalDomInvariantViolation(source) => {
                write!(
                    f,
                    "patch invariant failed: resulting DOM state is invalid: {source}"
                )
            }
            Self::Internal(message) => {
                write!(f, "patch invariant checker internal failure: {message}")
            }
        }
    }
}

impl std::error::Error for PatchInvariantError {}

#[must_use]
fn create_operation_name(kind: DomInvariantNodeKind) -> &'static str {
    match kind {
        DomInvariantNodeKind::Document => "CreateDocument",
        DomInvariantNodeKind::Element => "CreateElement",
        DomInvariantNodeKind::Text => "CreateText",
        DomInvariantNodeKind::Comment => "CreateComment",
    }
}

pub fn check_dom_invariants(dom: &DomInvariantState) -> Result<(), DomInvariantError> {
    if dom.root.is_none() && dom.nodes.iter().any(Option::is_some) {
        return Err(DomInvariantError::MissingRootForNonEmptyState);
    }

    if let Some(root) = dom.root {
        let Some(root_node) = dom.node(root) else {
            return Err(DomInvariantError::RootNodeMissing { root });
        };
        if let Some(parent) = root_node.parent {
            return Err(DomInvariantError::RootHasParent { root, parent });
        }
        if !matches!(root_node.kind, DomInvariantNodeKind::Document) {
            return Err(DomInvariantError::RootIsNotDocument {
                root,
                actual: root_node.kind,
            });
        }
    }

    for (index, maybe_node) in dom.nodes.iter().enumerate() {
        let Some(node) = maybe_node else {
            continue;
        };
        let key = PatchKey(index as u32);

        if matches!(node.kind, DomInvariantNodeKind::Document) && dom.root != Some(key) {
            return Err(DomInvariantError::DocumentNodeNotRoot {
                key,
                actual_parent: node.parent,
            });
        }

        match node.parent {
            Some(parent) => {
                let Some(parent_node) = dom.node(parent) else {
                    return Err(DomInvariantError::DanglingParentReference { key, parent });
                };
                let matches = parent_node
                    .children
                    .iter()
                    .filter(|child| **child == key)
                    .count();
                if matches != 1 {
                    return Err(DomInvariantError::ParentChildMismatch {
                        key,
                        parent,
                        matches,
                    });
                }
            }
            None if dom.root != Some(key) => {
                return Err(DomInvariantError::DetachedNonRootNode { key });
            }
            None => {}
        }

        let mut unique_children = HashSet::new();
        for child in &node.children {
            let Some(child_node) = dom.node(*child) else {
                return Err(DomInvariantError::DanglingChildReference {
                    parent: key,
                    child: *child,
                });
            };
            if !unique_children.insert(*child) {
                return Err(DomInvariantError::DuplicateChildReference {
                    parent: key,
                    child: *child,
                });
            }
            if child_node.parent != Some(key) {
                return Err(DomInvariantError::ChildParentMismatch {
                    parent: key,
                    child: *child,
                    actual_parent: child_node.parent,
                });
            }
        }
    }

    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    for (index, maybe_node) in dom.nodes.iter().enumerate() {
        if maybe_node.is_none() {
            continue;
        }
        assert_acyclic_from(dom, PatchKey(index as u32), &mut visited, &mut visiting)?;
    }

    Ok(())
}

pub fn check_patch_invariants(
    patches: &[DomPatch],
    dom_state: &DomInvariantState,
) -> Result<DomInvariantState, PatchInvariantError> {
    check_dom_invariants(dom_state).map_err(PatchInvariantError::InvalidBaseline)?;

    let mut staged = dom_state.clone();
    let clear_batch = matches!(patches.first(), Some(DomPatch::Clear));

    for (patch_index, patch) in patches.iter().enumerate() {
        match patch {
            DomPatch::Clear => {
                if patch_index != 0 {
                    return Err(PatchInvariantError::ClearMustBeFirst { patch_index });
                }
                staged.clear();
            }
            DomPatch::CreateDocument { key, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Document, patch_index)?;
            }
            DomPatch::CreateElement { key, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Element, patch_index)?;
            }
            DomPatch::CreateText { key, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Text, patch_index)?;
            }
            DomPatch::CreateComment { key, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Comment, patch_index)?;
            }
            DomPatch::AppendChild { parent, child } => {
                staged.apply_append_child(patch_index, *parent, *child)?;
            }
            DomPatch::InsertBefore {
                parent,
                child,
                before,
            } => {
                staged.apply_insert_before(patch_index, *parent, *child, *before)?;
            }
            DomPatch::RemoveNode { key } => {
                staged.apply_remove_node(patch_index, *key)?;
            }
            DomPatch::SetAttributes { key, .. } => {
                staged.apply_kind_checked_patch(
                    patch_index,
                    *key,
                    "SetAttributes",
                    DomInvariantNodeKind::Element,
                )?;
            }
            DomPatch::SetText { key, .. } => {
                staged.apply_kind_checked_patch(
                    patch_index,
                    *key,
                    "SetText",
                    DomInvariantNodeKind::Text,
                )?;
            }
            DomPatch::AppendText { key, .. } => {
                staged.apply_kind_checked_patch(
                    patch_index,
                    *key,
                    "AppendText",
                    DomInvariantNodeKind::Text,
                )?;
            }
        }
    }

    if clear_batch && staged.root.is_none() {
        return Err(PatchInvariantError::ClearBatchMustReestablishDocument);
    }
    check_dom_invariants(&staged).map_err(PatchInvariantError::FinalDomInvariantViolation)?;
    Ok(staged)
}

fn assert_acyclic_from(
    dom: &DomInvariantState,
    key: PatchKey,
    visited: &mut HashSet<PatchKey>,
    visiting: &mut HashSet<PatchKey>,
) -> Result<(), DomInvariantError> {
    if visited.contains(&key) {
        return Ok(());
    }
    if !visiting.insert(key) {
        return Err(DomInvariantError::CycleDetected { key });
    }
    let Some(node) = dom.node(key) else {
        return Err(DomInvariantError::DanglingChildReference {
            parent: key,
            child: key,
        });
    };
    for child in &node.children {
        assert_acyclic_from(dom, *child, visited, visiting)?;
    }
    visiting.remove(&key);
    visited.insert(key);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DomInvariantError, DomInvariantNode, DomInvariantNodeKind, DomInvariantState};
    use super::{PatchInvariantError, check_dom_invariants, check_patch_invariants};
    use crate::dom_patch::{DomPatch, PatchKey};
    use std::sync::Arc;

    fn element(name: &'static str, key: u32) -> DomPatch {
        DomPatch::CreateElement {
            key: PatchKey(key),
            name: Arc::from(name),
            attributes: Vec::new(),
        }
    }

    #[test]
    fn dom_checker_accepts_minimal_document_tree() {
        let state = check_patch_invariants(
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                element("html", 2),
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
            ],
            &DomInvariantState::default(),
        )
        .expect("minimal document tree should satisfy invariants");

        check_dom_invariants(&state).expect("resulting state should remain valid");
    }

    #[test]
    fn dom_checker_rejects_detached_non_root_nodes() {
        let state = DomInvariantState {
            root: Some(PatchKey(1)),
            nodes: vec![
                None,
                Some(DomInvariantNode {
                    kind: DomInvariantNodeKind::Document,
                    parent: None,
                    children: vec![PatchKey(2)],
                }),
                Some(DomInvariantNode {
                    kind: DomInvariantNodeKind::Element,
                    parent: Some(PatchKey(1)),
                    children: Vec::new(),
                }),
                Some(DomInvariantNode {
                    kind: DomInvariantNodeKind::Element,
                    parent: None,
                    children: Vec::new(),
                }),
            ],
        };

        let err = check_dom_invariants(&state).expect_err("detached node must be rejected");
        assert_eq!(
            err,
            DomInvariantError::DetachedNonRootNode { key: PatchKey(3) }
        );
    }

    #[test]
    fn patch_checker_rejects_clear_not_first() {
        let err = check_patch_invariants(
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::Clear,
            ],
            &DomInvariantState::default(),
        )
        .expect_err("Clear must only appear first");

        assert_eq!(
            err,
            PatchInvariantError::ClearMustBeFirst { patch_index: 1 }
        );
    }

    #[test]
    fn patch_checker_rejects_invalid_baseline_state() {
        let invalid_baseline = DomInvariantState {
            root: Some(PatchKey(1)),
            nodes: vec![
                None,
                Some(DomInvariantNode {
                    kind: DomInvariantNodeKind::Document,
                    parent: None,
                    children: Vec::new(),
                }),
                Some(DomInvariantNode {
                    kind: DomInvariantNodeKind::Element,
                    parent: None,
                    children: Vec::new(),
                }),
            ],
        };

        let err = check_patch_invariants(
            &[DomPatch::CreateText {
                key: PatchKey(3),
                text: "x".to_string(),
            }],
            &invalid_baseline,
        )
        .expect_err("invalid baseline DOM state must be rejected");

        assert_eq!(
            err,
            PatchInvariantError::InvalidBaseline(DomInvariantError::DetachedNonRootNode {
                key: PatchKey(2)
            })
        );
    }

    #[test]
    fn patch_checker_rejects_clear_batch_without_root_restoration() {
        let baseline = check_patch_invariants(
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                element("html", 2),
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
            ],
            &DomInvariantState::default(),
        )
        .expect("baseline should be valid");

        let err = check_patch_invariants(&[DomPatch::Clear], &baseline)
            .expect_err("Clear batches must restore a rooted document");

        assert_eq!(err, PatchInvariantError::ClearBatchMustReestablishDocument);
    }

    #[test]
    fn patch_checker_rejects_duplicate_document_creation() {
        let err = check_patch_invariants(
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateDocument {
                    key: PatchKey(2),
                    doctype: None,
                },
            ],
            &DomInvariantState::default(),
        )
        .expect_err("multiple document roots in one state must be rejected");

        assert_eq!(
            err,
            PatchInvariantError::DuplicateDocumentRoot {
                patch_index: 1,
                existing_root: PatchKey(1),
                new_root: PatchKey(2),
            }
        );
    }

    #[test]
    fn patch_checker_rejects_cycle_creating_move() {
        let baseline = check_patch_invariants(
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                element("html", 2),
                element("body", 3),
                element("div", 4),
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(3),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(3),
                    child: PatchKey(4),
                },
            ],
            &DomInvariantState::default(),
        )
        .expect("baseline should be valid");

        let err = check_patch_invariants(
            &[DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(3),
            }],
            &baseline,
        )
        .expect_err("cycle-creating move must be rejected");

        assert_eq!(
            err,
            PatchInvariantError::CycleCreation {
                patch_index: 0,
                operation: "AppendChild",
                parent: PatchKey(4),
                child: PatchKey(3),
            }
        );
    }

    #[test]
    fn patch_checker_rejects_insert_before_with_wrong_parent() {
        let baseline = check_patch_invariants(
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                element("html", 2),
                element("body", 3),
                element("div", 4),
                element("p", 5),
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(3),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(3),
                    child: PatchKey(4),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(5),
                },
            ],
            &DomInvariantState::default(),
        )
        .expect("baseline should be valid");

        let err = check_patch_invariants(
            &[DomPatch::InsertBefore {
                parent: PatchKey(3),
                child: PatchKey(4),
                before: PatchKey(5),
            }],
            &baseline,
        )
        .expect_err("before node parent mismatch must be rejected");

        assert_eq!(
            err,
            PatchInvariantError::InsertBeforeParentMismatch {
                patch_index: 0,
                parent: PatchKey(3),
                before: PatchKey(5),
                actual_parent: Some(PatchKey(2)),
            }
        );
    }

    #[test]
    fn patch_checker_rejects_wrong_node_kind_operations() {
        let baseline = check_patch_invariants(
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                element("html", 2),
                DomPatch::CreateComment {
                    key: PatchKey(3),
                    text: "x".to_string(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(3),
                },
            ],
            &DomInvariantState::default(),
        )
        .expect("baseline should be valid");

        let err = check_patch_invariants(
            &[DomPatch::AppendText {
                key: PatchKey(3),
                text: "y".to_string(),
            }],
            &baseline,
        )
        .expect_err("AppendText on a comment must be rejected");

        assert_eq!(
            err,
            PatchInvariantError::WrongNodeKind {
                patch_index: 0,
                operation: "AppendText",
                key: PatchKey(3),
                expected: DomInvariantNodeKind::Text,
                actual: DomInvariantNodeKind::Comment,
            }
        );
    }
}
