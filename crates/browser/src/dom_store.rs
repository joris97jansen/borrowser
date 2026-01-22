use core_types::{DomHandle, DomVersion};
use html::{DomPatch, Node, PatchKey};
use html::internal::Id;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug)]
pub enum DomPatchError {
    UnknownHandle(DomHandle),
    VersionMismatch { expected: DomVersion, got: DomVersion },
    NonMonotonicVersion { from: DomVersion, to: DomVersion },
    InvalidKey(PatchKey),
    DuplicateKey(PatchKey),
    MissingKey(PatchKey),
    WrongNodeKind(PatchKey),
    InvalidParent(PatchKey),
    InvalidSibling { parent: PatchKey, before: PatchKey },
    CycleDetected { parent: PatchKey, child: PatchKey },
    MissingRoot,
}

pub struct DomStore {
    docs: HashMap<DomHandle, DomDoc>,
}

impl DomStore {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    pub fn create(&mut self, handle: DomHandle) -> Result<(), DomPatchError> {
        if self.docs.contains_key(&handle) {
            return Ok(());
        }
        self.docs.insert(handle, DomDoc::new());
        Ok(())
    }

    pub fn drop_handle(&mut self, handle: DomHandle) {
        self.docs.remove(&handle);
    }

    pub fn clear(&mut self) {
        self.docs.clear();
    }

    pub fn apply(
        &mut self,
        handle: DomHandle,
        from: DomVersion,
        to: DomVersion,
        patches: &[DomPatch],
    ) -> Result<(), DomPatchError> {
        let doc = self
            .docs
            .get_mut(&handle)
            .ok_or(DomPatchError::UnknownHandle(handle))?;
        if doc.version != from {
            return Err(DomPatchError::VersionMismatch {
                expected: doc.version,
                got: from,
            });
        }
        if to != from.next() {
            return Err(DomPatchError::NonMonotonicVersion { from, to });
        }

        doc.apply(patches)?;
        doc.version = to;
        doc.rebuild_cache()?;
        Ok(())
    }

    pub fn get_current(&self, handle: DomHandle) -> Option<&Node> {
        self.docs.get(&handle).and_then(|doc| doc.current.as_deref())
    }

    pub fn materialize(&self, handle: DomHandle) -> Result<Box<Node>, DomPatchError> {
        let doc = self
            .docs
            .get(&handle)
            .ok_or(DomPatchError::UnknownHandle(handle))?;
        doc.materialize()
    }
}

impl Default for DomStore {
    fn default() -> Self {
        Self::new()
    }
}

struct DomDoc {
    version: DomVersion,
    arena: DomArena,
    root: Option<PatchKey>,
    current: Option<Box<Node>>,
}

impl DomDoc {
    fn new() -> Self {
        Self {
            version: DomVersion::INITIAL,
            arena: DomArena::new(),
            root: None,
            current: None,
        }
    }

    fn apply(&mut self, patches: &[DomPatch]) -> Result<(), DomPatchError> {
        for patch in patches {
            self.apply_one(patch)?;
        }
        Ok(())
    }

    fn apply_one(&mut self, patch: &DomPatch) -> Result<(), DomPatchError> {
        match patch {
            DomPatch::CreateDocument { key, doctype } => {
                self.ensure_key(*key)?;
                self.arena.insert_node(*key, NodeKind::Document {
                    doctype: doctype.clone(),
                })?;
                self.root = Some(*key);
            }
            DomPatch::CreateElement {
                key,
                name,
                attributes,
            } => {
                self.ensure_key(*key)?;
                self.arena.insert_node(
                    *key,
                    NodeKind::Element {
                        name: Arc::clone(name),
                        attributes: attributes.clone(),
                    },
                )?;
            }
            DomPatch::CreateText { key, text } => {
                self.ensure_key(*key)?;
                self.arena
                    .insert_node(*key, NodeKind::Text { text: text.clone() })?;
            }
            DomPatch::CreateComment { key, text } => {
                self.ensure_key(*key)?;
                self.arena
                    .insert_node(*key, NodeKind::Comment { text: text.clone() })?;
            }
            DomPatch::AppendChild { parent, child } => {
                self.ensure_live(*parent)?;
                self.ensure_live(*child)?;
                self.arena.append_child(*parent, *child)?;
            }
            DomPatch::InsertBefore {
                parent,
                child,
                before,
            } => {
                self.ensure_live(*parent)?;
                self.ensure_live(*child)?;
                self.ensure_live(*before)?;
                self.arena.insert_before(*parent, *child, *before)?;
            }
            DomPatch::RemoveNode { key } => {
                self.ensure_live(*key)?;
                if self.root == Some(*key) {
                    self.root = None;
                }
                self.arena.remove_subtree(*key)?;
            }
            DomPatch::SetAttributes { key, attributes } => {
                self.ensure_live(*key)?;
                self.arena.set_attributes(*key, attributes)?;
            }
            DomPatch::SetText { key, text } => {
                self.ensure_live(*key)?;
                self.arena.set_text(*key, text)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn ensure_key(&self, key: PatchKey) -> Result<(), DomPatchError> {
        if key == PatchKey::INVALID {
            debug_assert!(false, "patch key must be non-zero");
            return Err(DomPatchError::InvalidKey(key));
        }
        Ok(())
    }

    fn ensure_live(&self, key: PatchKey) -> Result<(), DomPatchError> {
        self.ensure_key(key)?;
        if !self.arena.live.contains_key(&key) {
            debug_assert!(false, "missing node key");
            return Err(DomPatchError::MissingKey(key));
        }
        Ok(())
    }

    fn rebuild_cache(&mut self) -> Result<(), DomPatchError> {
        let Some(root) = self.root else {
            self.current = None;
            return Ok(());
        };
        let node = self.arena.materialize(root)?;
        self.current = Some(Box::new(node));
        Ok(())
    }

    fn materialize(&self) -> Result<Box<Node>, DomPatchError> {
        let Some(root) = self.root else {
            return Err(DomPatchError::MissingRoot);
        };
        Ok(Box::new(self.arena.materialize(root)?))
    }
}

struct DomArena {
    nodes: Vec<NodeRecord>,
    live: HashMap<PatchKey, usize>,
    allocated: HashSet<PatchKey>,
}

impl DomArena {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            live: HashMap::new(),
            allocated: HashSet::new(),
        }
    }

    fn insert_node(&mut self, key: PatchKey, kind: NodeKind) -> Result<(), DomPatchError> {
        if self.allocated.contains(&key) {
            debug_assert!(false, "duplicate node key");
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
        Ok(())
    }

    fn append_child(&mut self, parent: PatchKey, child: PatchKey) -> Result<(), DomPatchError> {
        if parent == child {
            debug_assert!(false, "cannot create cycle");
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        if self.is_descendant(child, parent) {
            debug_assert!(false, "cannot create cycle");
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        let parent_index = *self.live.get(&parent).unwrap();
        let child_index = *self.live.get(&child).unwrap();
        if !self.nodes[parent_index].allows_children() {
            debug_assert!(false, "parent node cannot have children");
            return Err(DomPatchError::InvalidParent(parent));
        }
        if self.nodes[child_index].parent.is_some() {
            debug_assert!(false, "child already has a parent");
            return Err(DomPatchError::InvalidParent(child));
        }
        self.nodes[parent_index].children.push(child);
        self.nodes[child_index].parent = Some(parent);
        Ok(())
    }

    fn insert_before(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) -> Result<(), DomPatchError> {
        if parent == child {
            debug_assert!(false, "cannot create cycle");
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        if self.is_descendant(child, parent) {
            debug_assert!(false, "cannot create cycle");
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        let parent_index = *self.live.get(&parent).unwrap();
        let child_index = *self.live.get(&child).unwrap();
        if !self.nodes[parent_index].allows_children() {
            debug_assert!(false, "parent node cannot have children");
            return Err(DomPatchError::InvalidParent(parent));
        }
        if self.nodes[child_index].parent.is_some() {
            debug_assert!(false, "child already has a parent");
            return Err(DomPatchError::InvalidParent(child));
        }
        let before_index = *self.live.get(&before).unwrap();
        if self.nodes[before_index].parent != Some(parent) {
            debug_assert!(false, "before is not a child of parent");
            return Err(DomPatchError::InvalidSibling { parent, before });
        }
        let siblings = &mut self.nodes[parent_index].children;
        let pos = siblings
            .iter()
            .position(|k| *k == before)
            .ok_or(DomPatchError::InvalidSibling { parent, before })?;
        siblings.insert(pos, child);
        self.nodes[child_index].parent = Some(parent);
        Ok(())
    }

    fn remove_subtree(&mut self, key: PatchKey) -> Result<(), DomPatchError> {
        let index = *self.live.get(&key).unwrap();
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
        Ok(())
    }

    fn set_attributes(
        &mut self,
        key: PatchKey,
        attributes: &[(Arc<str>, Option<String>)],
    ) -> Result<(), DomPatchError> {
        let index = *self.live.get(&key).unwrap();
        match &mut self.nodes[index].kind {
            NodeKind::Element { attributes: attrs, .. } => {
                attrs.clear();
                attrs.extend(attributes.iter().cloned());
                Ok(())
            }
            _ => Err(DomPatchError::WrongNodeKind(key)),
        }
    }

    fn set_text(&mut self, key: PatchKey, text: &str) -> Result<(), DomPatchError> {
        let index = *self.live.get(&key).unwrap();
        match &mut self.nodes[index].kind {
            NodeKind::Text { text: existing } => {
                existing.clear();
                existing.push_str(text);
                Ok(())
            }
            _ => Err(DomPatchError::WrongNodeKind(key)),
        }
    }

    fn is_descendant(&self, ancestor: PatchKey, maybe_descendant: PatchKey) -> bool {
        let Some(&index) = self.live.get(&ancestor) else {
            return false;
        };
        let mut stack = Vec::new();
        stack.extend(self.nodes[index].children.iter().copied());
        while let Some(current) = stack.pop() {
            if current == maybe_descendant {
                return true;
            }
            if let Some(&child_index) = self.live.get(&current) {
                stack.extend(self.nodes[child_index].children.iter().copied());
            }
        }
        false
    }

    fn materialize(&self, root: PatchKey) -> Result<Node, DomPatchError> {
        let Some(&index) = self.live.get(&root) else {
            return Err(DomPatchError::MissingKey(root));
        };
        self.materialize_node(index, root)
    }

    fn materialize_node(&self, index: usize, key: PatchKey) -> Result<Node, DomPatchError> {
        let id = Id(key.0);
        let children = self.nodes[index]
            .children
            .iter()
            .map(|child_key| {
                let child_index = *self
                    .live
                    .get(child_key)
                    .ok_or(DomPatchError::MissingKey(*child_key))?;
                self.materialize_node(child_index, *child_key)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let node = match &self.nodes[index].kind {
            NodeKind::Document { doctype } => Node::Document {
                id,
                doctype: doctype.clone(),
                children,
            },
            NodeKind::Element { name, attributes } => Node::Element {
                id,
                name: Arc::clone(name),
                attributes: attributes.clone(),
                style: Vec::new(),
                children,
            },
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
}

struct NodeRecord {
    kind: NodeKind,
    parent: Option<PatchKey>,
    children: Vec<PatchKey>,
}

impl NodeRecord {
    fn allows_children(&self) -> bool {
        matches!(self.kind, NodeKind::Document { .. } | NodeKind::Element { .. })
    }
}

enum NodeKind {
    Document { doctype: Option<String> },
    Element {
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
    },
    Text { text: String },
    Comment { text: String },
}
