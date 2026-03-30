use crate::dom_patch::PatchKey;

use super::model::DomInvariantNodeKind;

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
