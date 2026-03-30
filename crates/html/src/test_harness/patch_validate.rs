use crate::Node;
use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::Id;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchValidationError {
    context: &'static str,
    detail: String,
}

impl PatchValidationError {
    fn new(context: &'static str, detail: impl Into<String>) -> Self {
        Self {
            context,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for PatchValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context, self.detail)
    }
}

impl std::error::Error for PatchValidationError {}

type ArenaResult<T> = Result<T, PatchValidationError>;

#[derive(Clone, Debug)]
enum PatchKind {
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

#[derive(Clone, Debug)]
struct PatchNode {
    kind: PatchKind,
    parent: Option<PatchKey>,
    children: Vec<PatchKey>,
}

/// Minimal patch-applier/validator for fuzzing and test lanes.
///
/// The arena applies batches atomically, validates the resulting structure after
/// every batch, and can materialize the final simplified DOM when needed.
///
/// Allocation policy:
/// - `Clear` resets the live tree state
/// - `Clear` does not release historically allocated patch keys
/// - recreated content must therefore use fresh keys across the whole session
#[derive(Clone, Default)]
pub struct PatchValidationArena {
    nodes: HashMap<PatchKey, PatchNode>,
    allocated: HashSet<PatchKey>,
    root: Option<PatchKey>,
}

impl PatchValidationArena {
    pub fn apply_batch(&mut self, patches: &[DomPatch]) -> Result<(), PatchValidationError> {
        let mut staged = self.clone();
        staged.apply_batch_in_place(patches)?;
        staged.assert_invariants()?;
        *self = staged;
        Ok(())
    }

    pub fn materialize(&self) -> Result<Node, PatchValidationError> {
        let root = self.root.ok_or_else(|| {
            PatchValidationError::new("materialize", "missing document root after patch apply")
        })?;
        self.materialize_node(root)
    }

    fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
    }

    fn insert(&mut self, key: PatchKey, node: PatchNode) -> ArenaResult<()> {
        if key == PatchKey::INVALID {
            return Err(PatchValidationError::new(
                "create",
                "patch key must be non-zero",
            ));
        }
        if self.allocated.contains(&key) {
            return Err(PatchValidationError::new(
                "create",
                format!("duplicate patch key {key:?}"),
            ));
        }
        self.nodes.insert(key, node);
        self.allocated.insert(key);
        Ok(())
    }

    fn ensure_node(&self, key: PatchKey, context: &'static str) -> ArenaResult<()> {
        if key == PatchKey::INVALID {
            return Err(PatchValidationError::new(
                context,
                "invalid patch key PatchKey(0)",
            ));
        }
        if !self.nodes.contains_key(&key) {
            return Err(PatchValidationError::new(
                context,
                format!("missing node {key:?}"),
            ));
        }
        Ok(())
    }

    fn ensure_container(&self, key: PatchKey, context: &'static str) -> ArenaResult<()> {
        self.ensure_node(key, context)?;
        let node = self.nodes.get(&key).ok_or_else(|| {
            PatchValidationError::new(context, format!("missing node {key:?} after lookup"))
        })?;
        match node.kind {
            PatchKind::Document { .. } | PatchKind::Element { .. } => Ok(()),
            PatchKind::Text { .. } | PatchKind::Comment { .. } => Err(PatchValidationError::new(
                context,
                "target must be a container node",
            )),
        }
    }

    fn node_parent(&self, key: PatchKey) -> ArenaResult<Option<PatchKey>> {
        self.nodes.get(&key).map(|node| node.parent).ok_or_else(|| {
            PatchValidationError::new("parent lookup", format!("missing node {key:?}"))
        })
    }

    fn is_document_node(&self, key: PatchKey) -> ArenaResult<bool> {
        self.nodes
            .get(&key)
            .map(|node| matches!(node.kind, PatchKind::Document { .. }))
            .ok_or_else(|| {
                PatchValidationError::new("document check", format!("missing node {key:?}"))
            })
    }

    fn is_document_root_element(&self, key: PatchKey) -> ArenaResult<bool> {
        let Some(root) = self.root else {
            return Ok(false);
        };
        let Some(node) = self.nodes.get(&key) else {
            return Err(PatchValidationError::new(
                "document root check",
                format!("missing node {key:?}"),
            ));
        };
        Ok(node.parent == Some(root) && matches!(node.kind, PatchKind::Element { .. }))
    }

    fn would_create_cycle(&self, parent: PatchKey, child: PatchKey) -> ArenaResult<bool> {
        let mut cursor = Some(parent);
        while let Some(current) = cursor {
            if current == child {
                return Ok(true);
            }
            cursor = self.node_parent(current)?;
        }
        Ok(false)
    }

    fn detach_child(&mut self, child: PatchKey) -> ArenaResult<()> {
        let parent = self
            .nodes
            .get(&child)
            .ok_or_else(|| PatchValidationError::new("detach", format!("missing child {child:?}")))?
            .parent;
        if let Some(parent) = parent
            && let Some(parent_node) = self.nodes.get_mut(&parent)
        {
            parent_node.children.retain(|existing| *existing != child);
        }
        self.nodes
            .get_mut(&child)
            .ok_or_else(|| PatchValidationError::new("detach", format!("missing child {child:?}")))?
            .parent = None;
        Ok(())
    }

    fn append_child(&mut self, parent: PatchKey, child: PatchKey) -> ArenaResult<()> {
        if parent == child {
            return Err(PatchValidationError::new(
                "AppendChild",
                "cannot attach a node to itself",
            ));
        }
        self.ensure_container(parent, "AppendChild parent")?;
        self.ensure_node(child, "AppendChild child")?;
        if self.is_document_node(child)? {
            return Err(PatchValidationError::new(
                "AppendChild child",
                "cannot move a document node",
            ));
        }
        if self.is_document_root_element(child)? {
            return Err(PatchValidationError::new(
                "AppendChild child",
                "cannot move the document root element",
            ));
        }
        if self.would_create_cycle(parent, child)? {
            return Err(PatchValidationError::new(
                "AppendChild",
                "cannot create an ancestor cycle",
            ));
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
        self.nodes
            .get_mut(&parent)
            .ok_or_else(|| PatchValidationError::new("AppendChild parent", "missing parent"))?
            .children
            .push(child);
        self.nodes
            .get_mut(&child)
            .ok_or_else(|| PatchValidationError::new("AppendChild child", "missing child"))?
            .parent = Some(parent);
        Ok(())
    }

    fn insert_before(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) -> ArenaResult<()> {
        if parent == child {
            return Err(PatchValidationError::new(
                "InsertBefore",
                "cannot attach a node to itself",
            ));
        }
        if child == before {
            return Err(PatchValidationError::new(
                "InsertBefore",
                "cannot insert a node before itself",
            ));
        }
        self.ensure_container(parent, "InsertBefore parent")?;
        self.ensure_node(child, "InsertBefore child")?;
        self.ensure_node(before, "InsertBefore before")?;
        if self.is_document_node(child)? {
            return Err(PatchValidationError::new(
                "InsertBefore child",
                "cannot move a document node",
            ));
        }
        if self.is_document_root_element(child)? {
            return Err(PatchValidationError::new(
                "InsertBefore child",
                "cannot move the document root element",
            ));
        }
        let before_parent = self.node_parent(before)?;
        if before_parent != Some(parent) {
            return Err(PatchValidationError::new(
                "InsertBefore before",
                format!("{before:?} is not attached under {parent:?}"),
            ));
        }
        if self.would_create_cycle(parent, child)? {
            return Err(PatchValidationError::new(
                "InsertBefore",
                "cannot create an ancestor cycle",
            ));
        }
        let already_in_place = if self.node_parent(child)? == Some(parent) {
            let siblings = &self
                .nodes
                .get(&parent)
                .ok_or_else(|| PatchValidationError::new("InsertBefore parent", "missing parent"))?
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
        let before_index = self
            .nodes
            .get(&parent)
            .ok_or_else(|| PatchValidationError::new("InsertBefore parent", "missing parent"))?
            .children
            .iter()
            .position(|key| *key == before)
            .ok_or_else(|| {
                PatchValidationError::new("InsertBefore before", "before child not found in parent")
            })?;
        self.nodes
            .get_mut(&parent)
            .ok_or_else(|| PatchValidationError::new("InsertBefore parent", "missing parent"))?
            .children
            .insert(before_index, child);
        self.nodes
            .get_mut(&child)
            .ok_or_else(|| PatchValidationError::new("InsertBefore child", "missing child"))?
            .parent = Some(parent);
        Ok(())
    }

    fn remove_subtree(&mut self, key: PatchKey) -> ArenaResult<()> {
        let children = {
            let node = self.nodes.get(&key).ok_or_else(|| {
                PatchValidationError::new("RemoveNode", format!("missing node {key:?}"))
            })?;
            node.children.clone()
        };
        if let Some(parent) = self.nodes.get(&key).and_then(|node| node.parent)
            && let Some(parent_node) = self.nodes.get_mut(&parent)
        {
            parent_node.children.retain(|child| *child != key);
        }
        for child in children {
            self.remove_subtree(child)?;
        }
        self.nodes.remove(&key);
        if self.root == Some(key) {
            self.root = None;
        }
        Ok(())
    }

    fn apply_batch_in_place(&mut self, patches: &[DomPatch]) -> ArenaResult<()> {
        if patches
            .get(1..)
            .is_some_and(|rest| rest.iter().any(|p| matches!(p, DomPatch::Clear)))
        {
            return Err(PatchValidationError::new(
                "batch order",
                "Clear may only appear as the first patch in a batch",
            ));
        }

        let mut start = 0usize;
        if matches!(patches.first(), Some(DomPatch::Clear)) {
            self.clear();
            start = 1;
        }

        for patch in &patches[start..] {
            match patch {
                DomPatch::Clear => {
                    return Err(PatchValidationError::new(
                        "batch order",
                        "Clear must be first patch in a batch",
                    ));
                }
                DomPatch::CreateDocument { key, doctype } => {
                    if self.root.is_some() {
                        return Err(PatchValidationError::new(
                            "CreateDocument",
                            "document root already exists",
                        ));
                    }
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Document {
                                doctype: doctype.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
                    self.root = Some(*key);
                }
                DomPatch::CreateElement {
                    key,
                    name,
                    attributes,
                } => {
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Element {
                                name: Arc::clone(name),
                                attributes: attributes.clone(),
                            },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
                }
                DomPatch::CreateText { key, text } => {
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Text { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
                }
                DomPatch::CreateComment { key, text } => {
                    self.insert(
                        *key,
                        PatchNode {
                            kind: PatchKind::Comment { text: text.clone() },
                            parent: None,
                            children: Vec::new(),
                        },
                    )?;
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
                    self.ensure_node(*key, "RemoveNode target")?;
                    let is_root = self.root == Some(*key);
                    let is_attached = self.nodes.get(key).and_then(|node| node.parent).is_some();
                    if !is_root && !is_attached {
                        return Err(PatchValidationError::new(
                            "RemoveNode target",
                            "cannot remove a detached node",
                        ));
                    }
                    self.remove_subtree(*key)?;
                }
                DomPatch::SetAttributes { key, attributes } => {
                    self.ensure_node(*key, "SetAttributes target")?;
                    let node = self.nodes.get_mut(key).ok_or_else(|| {
                        PatchValidationError::new("SetAttributes target", "missing node")
                    })?;
                    match &mut node.kind {
                        PatchKind::Element {
                            attributes: slot, ..
                        } => *slot = attributes.clone(),
                        _ => {
                            return Err(PatchValidationError::new(
                                "SetAttributes target",
                                "applied to non-element",
                            ));
                        }
                    }
                }
                DomPatch::SetText { key, text } => {
                    self.ensure_node(*key, "SetText target")?;
                    let node = self.nodes.get_mut(key).ok_or_else(|| {
                        PatchValidationError::new("SetText target", "missing node")
                    })?;
                    match &mut node.kind {
                        PatchKind::Text { text: slot } => *slot = text.clone(),
                        _ => {
                            return Err(PatchValidationError::new(
                                "SetText target",
                                "applied to non-text",
                            ));
                        }
                    }
                }
                DomPatch::AppendText { key, text } => {
                    self.ensure_node(*key, "AppendText target")?;
                    let node = self.nodes.get_mut(key).ok_or_else(|| {
                        PatchValidationError::new("AppendText target", "missing node")
                    })?;
                    match &mut node.kind {
                        PatchKind::Text { text: slot } => slot.push_str(text),
                        _ => {
                            return Err(PatchValidationError::new(
                                "AppendText target",
                                "applied to non-text",
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn assert_invariants(&self) -> ArenaResult<()> {
        if self.nodes.is_empty() {
            if self.root.is_some() {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    "root must be absent when no nodes remain",
                ));
            }
            return Ok(());
        }

        let root = self.root.ok_or_else(|| {
            PatchValidationError::new(
                "post-apply invariants",
                "non-empty patch arena must declare a document root",
            )
        })?;
        let root_node = self.nodes.get(&root).ok_or_else(|| {
            PatchValidationError::new("post-apply invariants", "declared root node is missing")
        })?;
        if !matches!(root_node.kind, PatchKind::Document { .. }) {
            return Err(PatchValidationError::new(
                "post-apply invariants",
                format!("root {root:?} must be a document node"),
            ));
        }
        if root_node.parent.is_some() {
            return Err(PatchValidationError::new(
                "post-apply invariants",
                "root node must not have a parent",
            ));
        }

        for (key, node) in &self.nodes {
            if matches!(node.kind, PatchKind::Document { .. }) && *key != root {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("document node {key:?} must be the declared root {root:?}"),
                ));
            }

            if let Some(parent) = node.parent {
                let parent_node = self.nodes.get(&parent).ok_or_else(|| {
                    PatchValidationError::new(
                        "post-apply invariants",
                        format!("dangling parent reference for {key:?}"),
                    )
                })?;
                let matches = parent_node
                    .children
                    .iter()
                    .filter(|child| **child == *key)
                    .count();
                if matches != 1 {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!(
                            "parent/child mismatch for {key:?}: expected exactly one reference from {parent:?}, found {matches}"
                        ),
                    ));
                }
            } else if *key != root {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("detached non-root node {key:?}"),
                ));
            }

            let mut unique_children = HashSet::new();
            for child in &node.children {
                if !self.nodes.contains_key(child) {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!("dangling child reference {child:?} under {key:?}"),
                    ));
                }
                if !unique_children.insert(*child) {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!("duplicate child reference {child:?} under {key:?}"),
                    ));
                }
                let child_parent = self
                    .nodes
                    .get(child)
                    .and_then(|child_node| child_node.parent)
                    .ok_or_else(|| {
                        PatchValidationError::new(
                            "post-apply invariants",
                            format!("child {child:?} missing parent back-reference"),
                        )
                    })?;
                if child_parent != *key {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!(
                            "child {child:?} parent mismatch: expected {key:?}, found {child_parent:?}"
                        ),
                    ));
                }
            }
        }

        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        for key in self.nodes.keys().copied() {
            self.assert_acyclic_from(key, &mut visited, &mut visiting)?;
        }

        Ok(())
    }

    fn assert_acyclic_from(
        &self,
        key: PatchKey,
        visited: &mut HashSet<PatchKey>,
        visiting: &mut HashSet<PatchKey>,
    ) -> ArenaResult<()> {
        if visited.contains(&key) {
            return Ok(());
        }
        if !visiting.insert(key) {
            return Err(PatchValidationError::new(
                "post-apply invariants",
                format!("cycle detected at {key:?}"),
            ));
        }
        let node = self.nodes.get(&key).ok_or_else(|| {
            PatchValidationError::new(
                "post-apply invariants",
                format!("missing node during cycle check: {key:?}"),
            )
        })?;
        for child in &node.children {
            self.assert_acyclic_from(*child, visited, visiting)?;
        }
        visiting.remove(&key);
        visited.insert(key);
        Ok(())
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
            PatchKind::Element { name, attributes } => Node::Element {
                id: Id::INVALID,
                name: Arc::clone(name),
                attributes: attributes.clone(),
                style: Vec::new(),
                children,
            },
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
}

#[cfg(test)]
mod tests {
    use super::PatchValidationArena;
    use crate::DomPatch;
    use crate::dom_patch::PatchKey;

    #[test]
    fn patch_validation_arena_accepts_valid_batches_and_materializes() {
        let mut arena = PatchValidationArena::default();
        arena
            .apply_batch(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: Some("html".to_string()),
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "html".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::CreateText {
                    key: PatchKey(3),
                    text: "ok".to_string(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(3),
                },
            ])
            .expect("valid batch should apply");

        let dom = arena.materialize().expect("valid arena should materialize");
        match dom {
            crate::Node::Document { children, .. } => assert_eq!(children.len(), 1),
            other => panic!("expected document root, got {other:?}"),
        }
    }

    #[test]
    fn patch_validation_arena_reports_clear_ordering_actionably() {
        let mut arena = PatchValidationArena::default();
        let err = arena
            .apply_batch(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::Clear,
            ])
            .expect_err("Clear after the first patch must fail");

        assert!(
            err.to_string()
                .contains("batch order: Clear may only appear as the first patch in a batch"),
            "unexpected clear-ordering error: {err}"
        );
    }

    #[test]
    fn patch_validation_arena_reports_missing_child_actionably() {
        let mut arena = PatchValidationArena::default();
        let err = arena
            .apply_batch(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(9),
                },
            ])
            .expect_err("missing child reference must fail");

        assert!(
            err.to_string()
                .contains("AppendChild child: missing node PatchKey(9)"),
            "unexpected append-child error: {err}"
        );
    }

    #[test]
    fn patch_validation_arena_rejects_detached_non_root_nodes() {
        let mut arena = PatchValidationArena::default();
        let err = arena
            .apply_batch(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "html".into(),
                    attributes: Vec::new(),
                },
            ])
            .expect_err("detached non-root nodes must fail validation");

        assert!(
            err.to_string()
                .contains("post-apply invariants: detached non-root node PatchKey(2)"),
            "unexpected detached-node error: {err}"
        );
    }

    #[test]
    fn patch_validation_arena_preserves_key_freshness_across_clear() {
        let mut arena = PatchValidationArena::default();
        arena
            .apply_batch(&[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "html".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
            ])
            .expect("seed batch should apply");

        let err = arena
            .apply_batch(&[
                DomPatch::Clear,
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
            ])
            .expect_err("Clear must not allow patch-key reuse");

        assert!(
            err.to_string()
                .contains("create: duplicate patch key PatchKey(1)"),
            "unexpected duplicate-key error after Clear: {err}"
        );
    }
}
