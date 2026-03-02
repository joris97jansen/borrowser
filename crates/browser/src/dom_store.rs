use core_types::{DomHandle, DomVersion};
use html::{DomPatch, Node, PatchKey};
// Temporary: Node requires an id field; materialization uses INVALID until render
// consumes the arena directly. Kept behind html's internal-api feature.
use html::internal::Id;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug)]
pub enum DomPatchError {
    UnknownHandle(DomHandle),
    DuplicateHandle(DomHandle),
    VersionMismatch {
        expected: DomVersion,
        got: DomVersion,
    },
    NonMonotonicVersion {
        from: DomVersion,
        to: DomVersion,
    },
    Protocol(&'static str),
    InvalidKey(PatchKey),
    DuplicateKey(PatchKey),
    MissingKey(PatchKey),
    WrongNodeKind {
        key: PatchKey,
        expected: &'static str,
        actual: &'static str,
    },
    InvalidParent(PatchKey),
    MoveNotSupported {
        key: PatchKey,
    },
    InvalidSibling {
        parent: PatchKey,
        before: PatchKey,
    },
    CycleDetected {
        parent: PatchKey,
        child: PatchKey,
    },
    MissingRoot,
    UnsupportedPatch(&'static str),
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
            return Err(DomPatchError::DuplicateHandle(handle));
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
        if patches.is_empty() {
            return Err(DomPatchError::Protocol("empty patch batch"));
        }
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
        // Apply transactionally: on any protocol/runtime error, keep the
        // previous document state unchanged.
        let mut staged = doc.clone();
        staged.apply(patches)?;
        staged.version = to;
        staged.rebuild_cache()?;
        *doc = staged;
        Ok(())
    }

    pub fn get_current(&self, handle: DomHandle) -> Option<&Node> {
        self.docs
            .get(&handle)
            .and_then(|doc| doc.current.as_deref())
    }

    pub fn materialize(&self, handle: DomHandle) -> Result<Box<Node>, DomPatchError> {
        Ok(Box::new(self.materialize_owned(handle)?))
    }

    pub fn materialize_owned(&self, handle: DomHandle) -> Result<Node, DomPatchError> {
        let doc = self
            .docs
            .get(&handle)
            .ok_or(DomPatchError::UnknownHandle(handle))?;
        doc.materialize_owned()
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

impl Clone for DomDoc {
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            arena: self.arena.clone(),
            root: self.root,
            // Cache is derived from arena/root and rebuilt on commit.
            current: None,
        }
    }
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

    fn clear(&mut self) {
        // Clear resets DOM contents (arena, root, cache); versioning is managed by DomStore.
        self.arena.clear();
        self.root = None;
        self.current = None;
    }

    fn apply(&mut self, patches: &[DomPatch]) -> Result<(), DomPatchError> {
        // Protocol invariants:
        // - Root is created only by `CreateDocument`.
        // - A `Clear` batch must re-establish a root by the end of the batch.
        // - A non-`Clear` batch must not leave the document rootless.
        if patches
            .get(1..)
            .is_some_and(|rest| rest.iter().any(|p| matches!(p, DomPatch::Clear)))
        {
            return Err(DomPatchError::Protocol(
                "Clear may only appear as the first patch",
            ));
        }
        let had_clear = matches!(patches.first(), Some(DomPatch::Clear));
        let start = if had_clear {
            self.clear();
            1
        } else {
            0
        };
        for patch in &patches[start..] {
            self.apply_one(patch, had_clear)?;
        }
        if had_clear && self.root.is_none() {
            return Err(DomPatchError::Protocol(
                "Clear batch must create a document root",
            ));
        }
        if !had_clear && self.root.is_none() {
            return Err(DomPatchError::Protocol(
                "document became rootless without Clear",
            ));
        }
        Ok(())
    }

    fn apply_one(&mut self, patch: &DomPatch, had_clear: bool) -> Result<(), DomPatchError> {
        match patch {
            DomPatch::Clear => {
                return Err(DomPatchError::Protocol(
                    "Clear must be first patch in a batch",
                ));
            }
            DomPatch::CreateDocument { key, doctype } => {
                if !had_clear && !self.is_fresh() {
                    return Err(DomPatchError::Protocol(
                        "CreateDocument requires Clear or fresh document",
                    ));
                }
                if self.root.is_some() {
                    return Err(DomPatchError::Protocol("root already set"));
                }
                self.ensure_key(*key)?;
                self.arena.insert_node(
                    *key,
                    NodeKind::Document {
                        doctype: doctype.clone(),
                    },
                )?;
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
            DomPatch::AppendText { key, text } => {
                self.ensure_live(*key)?;
                self.arena.append_text(*key, text)?;
            }
            _ => {
                return Err(DomPatchError::UnsupportedPatch(
                    "unsupported DomPatch variant in strict runtime applier",
                ));
            }
        }
        Ok(())
    }

    fn ensure_key(&self, key: PatchKey) -> Result<(), DomPatchError> {
        if key == PatchKey::INVALID {
            return Err(DomPatchError::InvalidKey(key));
        }
        Ok(())
    }

    fn ensure_live(&self, key: PatchKey) -> Result<(), DomPatchError> {
        self.ensure_key(key)?;
        if !self.arena.live.contains_key(&key) {
            return Err(DomPatchError::MissingKey(key));
        }
        Ok(())
    }

    fn rebuild_cache(&mut self) -> Result<(), DomPatchError> {
        let Some(root) = self.root else {
            self.current = None;
            return Ok(());
        };
        // TODO(v5.1): materialization is O(n); render pipeline should consume the arena directly.
        let node = self.arena.materialize(root)?;
        self.current = Some(Box::new(node));
        Ok(())
    }

    fn is_fresh(&self) -> bool {
        self.root.is_none() && self.arena.nodes.is_empty()
    }

    fn materialize_owned(&self) -> Result<Node, DomPatchError> {
        let Some(root) = self.root else {
            return Err(DomPatchError::MissingRoot);
        };
        // TODO(v5.1): materialization is O(n); render pipeline should consume the arena directly.
        self.arena.materialize(root)
    }
}

#[derive(Clone)]
struct DomArena {
    nodes: Vec<NodeRecord>,
    live: HashMap<PatchKey, usize>,
    // Keys allocated since last `clear()`. Keys are intentionally non-reusable
    // until Clear, even after subtree removal.
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

    fn clear(&mut self) {
        self.debug_check_invariants();
        self.nodes.clear();
        self.live.clear();
        self.allocated.clear();
        self.debug_check_invariants();
    }

    fn insert_node(&mut self, key: PatchKey, kind: NodeKind) -> Result<(), DomPatchError> {
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

    fn append_child(&mut self, parent: PatchKey, child: PatchKey) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        if parent == child {
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        if self.contains_in_subtree(child, parent) {
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
        if self.nodes[child_index].parent.is_some() {
            // Moving nodes is not supported yet.
            return Err(DomPatchError::MoveNotSupported { key: child });
        }
        self.nodes[parent_index].children.push(child);
        self.nodes[child_index].parent = Some(parent);
        self.debug_check_invariants();
        Ok(())
    }

    fn insert_before(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) -> Result<(), DomPatchError> {
        self.debug_check_invariants();
        if parent == child {
            return Err(DomPatchError::CycleDetected { parent, child });
        }
        if self.contains_in_subtree(child, parent) {
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
        if self.nodes[child_index].parent.is_some() {
            // Moving nodes is not supported yet.
            return Err(DomPatchError::MoveNotSupported { key: child });
        }
        let before_index = *self
            .live
            .get(&before)
            .ok_or(DomPatchError::MissingKey(before))?;
        if self.nodes[before_index].parent != Some(parent) {
            return Err(DomPatchError::InvalidSibling { parent, before });
        }
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
    fn remove_subtree(&mut self, key: PatchKey) -> Result<(), DomPatchError> {
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

    fn set_attributes(
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

    fn set_text(&mut self, key: PatchKey, text: &str) -> Result<(), DomPatchError> {
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

    fn append_text(&mut self, key: PatchKey, text: &str) -> Result<(), DomPatchError> {
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

    fn materialize(&self, root: PatchKey) -> Result<Node, DomPatchError> {
        let Some(&index) = self.live.get(&root) else {
            return Err(DomPatchError::MissingKey(root));
        };
        self.materialize_node(index, root)
    }

    fn materialize_node(&self, index: usize, _key: PatchKey) -> Result<Node, DomPatchError> {
        let id = Id::INVALID;
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

#[derive(Clone)]
struct NodeRecord {
    kind: NodeKind,
    parent: Option<PatchKey>,
    children: Vec<PatchKey>,
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
enum NodeKind {
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

#[cfg(test)]
mod tests {
    use super::{DomPatchError, DomStore};
    use core_types::{DomHandle, DomVersion};
    use html::PatchKey;
    use html::{DomPatch, Node};

    fn handle(id: u64) -> DomHandle {
        DomHandle(id)
    }

    fn stable_dom_lines(node: &Node) -> Vec<String> {
        fn escape(value: &str) -> String {
            let mut out = String::with_capacity(value.len());
            for ch in value.chars() {
                match ch {
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    '"' => out.push_str("\\\""),
                    '<' => out.push_str("\\u{3C}"),
                    '>' => out.push_str("\\u{3E}"),
                    c if c.is_ascii_control() => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
                    c if c.is_ascii() => out.push(c),
                    c => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
                }
            }
            out
        }

        fn push_node(out: &mut Vec<String>, node: &Node, depth: usize) {
            let indent = "  ".repeat(depth);
            match node {
                Node::Document {
                    doctype, children, ..
                } => {
                    out.push(match doctype {
                        Some(doctype) => {
                            format!("{indent}#document doctype=\"{}\"", escape(doctype))
                        }
                        None => format!("{indent}#document doctype=<none>"),
                    });
                    for child in children {
                        push_node(out, child, depth + 1);
                    }
                }
                Node::Element {
                    name,
                    attributes,
                    children,
                    ..
                } => {
                    let mut attrs = attributes
                        .iter()
                        .map(|(k, v)| (k.as_ref(), v.as_deref()))
                        .collect::<Vec<_>>();
                    attrs.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
                    let attrs = attrs
                        .into_iter()
                        .map(|(k, v)| match v {
                            Some(v) => format!("{k}=\"{}\"", escape(v)),
                            None => format!("{k}=<none>"),
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push(format!("{indent}<{name} attrs=[{attrs}]>"));
                    for child in children {
                        push_node(out, child, depth + 1);
                    }
                }
                Node::Text { text, .. } => {
                    out.push(format!("{indent}text=\"{}\"", escape(text)));
                }
                Node::Comment { text, .. } => {
                    out.push(format!("{indent}comment=\"{}\"", escape(text)));
                }
            }
        }

        let mut out = Vec::new();
        push_node(&mut out, node, 0);
        out
    }

    #[test]
    fn create_duplicate_handle_errors() {
        let mut store = DomStore::new();
        let h = handle(1);
        store.create(h).expect("first create should succeed");
        let err = store.create(h).expect_err("duplicate create should error");
        assert!(matches!(err, DomPatchError::DuplicateHandle(v) if v == h));
    }

    #[test]
    fn apply_is_atomic_on_mid_batch_error() {
        let mut store = DomStore::new();
        let h = handle(7);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                }],
            )
            .expect("bootstrap apply");

        let before = store
            .materialize(h)
            .expect("materialize before failed apply");
        let before = stable_dom_lines(&before);

        let err = store
            .apply(
                h,
                v1,
                v2,
                &[
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "div".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::AppendChild {
                        parent: PatchKey(1),
                        child: PatchKey(2),
                    },
                    DomPatch::AppendText {
                        key: PatchKey(1),
                        text: "x".to_string(),
                    },
                ],
            )
            .expect_err("invalid mid-batch operation should fail");
        assert!(matches!(err, DomPatchError::WrongNodeKind { .. }));

        // State remains unchanged and version did not advance.
        let after = store
            .materialize(h)
            .expect("materialize after failed apply");
        let after = stable_dom_lines(&after);
        assert_eq!(before, after, "failed batch must not partially commit");

        store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::CreateComment {
                    key: PatchKey(3),
                    text: "ok".to_string(),
                }],
            )
            .expect("version should remain unchanged after failed batch");
    }

    #[test]
    fn clear_only_batch_is_rejected() {
        let mut store = DomStore::new();
        let h = handle(9);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                }],
            )
            .expect("bootstrap apply");

        let err = store
            .apply(h, v1, v2, &[DomPatch::Clear])
            .expect_err("clear-only batch should be rejected");
        assert!(matches!(err, DomPatchError::Protocol(_)));
    }

    #[test]
    fn empty_patch_batch_is_rejected() {
        let mut store = DomStore::new();
        let h = handle(11);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let err = store
            .apply(h, v0, v1, &[])
            .expect_err("empty patch batch should be rejected");
        assert!(matches!(err, DomPatchError::Protocol(_)));
    }

    #[test]
    fn clear_batch_with_document_is_allowed() {
        let mut store = DomStore::new();
        let h = handle(12);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        // Bootstrap document.
        store
            .apply(
                h,
                v0,
                v1,
                &[DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                }],
            )
            .expect("bootstrap apply");

        // Reset to a new rooted document.
        store
            .apply(
                h,
                v1,
                v2,
                &[
                    DomPatch::Clear,
                    DomPatch::CreateDocument {
                        key: PatchKey(10),
                        doctype: None,
                    },
                ],
            )
            .expect("clear + CreateDocument should be accepted");

        let dom = store
            .materialize_owned(h)
            .expect("materialize after reset should succeed");
        let lines = stable_dom_lines(&dom);
        assert!(
            lines
                .first()
                .is_some_and(|line| line.starts_with("#document")),
            "reset batch should leave a rooted document"
        );
    }

    #[test]
    fn clear_not_first_is_rejected() {
        let mut store = DomStore::new();
        let h = handle(13);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();

        let err = store
            .apply(
                h,
                v0,
                v1,
                &[
                    DomPatch::CreateDocument {
                        key: PatchKey(1),
                        doctype: None,
                    },
                    DomPatch::Clear,
                ],
            )
            .expect_err("Clear not first should be rejected");
        assert!(
            matches!(err, DomPatchError::Protocol(msg) if msg.contains("first patch")),
            "expected protocol error about Clear ordering, got: {err:?}"
        );
    }

    #[test]
    fn duplicate_key_is_rejected_and_atomic() {
        let mut store = DomStore::new();
        let h = handle(14);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                }],
            )
            .expect("bootstrap apply");

        let before = store
            .materialize(h)
            .expect("materialize before failed apply");
        let before = stable_dom_lines(&before);

        let err = store
            .apply(
                h,
                v1,
                v2,
                &[
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "div".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "span".into(),
                        attributes: Vec::new(),
                    },
                ],
            )
            .expect_err("duplicate key apply should fail");
        assert!(matches!(err, DomPatchError::DuplicateKey(PatchKey(2))));

        let after = store
            .materialize(h)
            .expect("materialize after failed apply");
        let after = stable_dom_lines(&after);
        assert_eq!(before, after, "failed batch must not partially commit");

        store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::CreateComment {
                    key: PatchKey(3),
                    text: "ok".to_string(),
                }],
            )
            .expect("version should remain unchanged after failed batch");
    }

    #[test]
    fn invalid_key_is_rejected() {
        let mut store = DomStore::new();
        let h = handle(15);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();

        let err = store
            .apply(
                h,
                v0,
                v1,
                &[DomPatch::CreateDocument {
                    key: PatchKey::INVALID,
                    doctype: None,
                }],
            )
            .expect_err("invalid key should be rejected");
        assert!(matches!(err, DomPatchError::InvalidKey(PatchKey::INVALID)));
    }

    #[test]
    fn missing_key_is_rejected_and_atomic() {
        let mut store = DomStore::new();
        let h = handle(16);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[
                    DomPatch::CreateDocument {
                        key: PatchKey(1),
                        doctype: None,
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "div".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::AppendChild {
                        parent: PatchKey(1),
                        child: PatchKey(2),
                    },
                ],
            )
            .expect("bootstrap apply");

        let before = store
            .materialize(h)
            .expect("materialize before failed apply");
        let before = stable_dom_lines(&before);

        let err = store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::AppendChild {
                    parent: PatchKey(999),
                    child: PatchKey(2),
                }],
            )
            .expect_err("missing parent key should be rejected");
        assert!(matches!(err, DomPatchError::MissingKey(PatchKey(999))));

        let after = store
            .materialize(h)
            .expect("materialize after failed apply");
        let after = stable_dom_lines(&after);
        assert_eq!(before, after, "failed batch must not partially commit");

        store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::CreateComment {
                    key: PatchKey(3),
                    text: "ok".to_string(),
                }],
            )
            .expect("version should remain unchanged after failed batch");
    }

    #[test]
    fn cycle_detection_rejects_back_edge_and_is_atomic() {
        let mut store = DomStore::new();
        let h = handle(17);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();
        let v3 = v2.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[
                    DomPatch::CreateDocument {
                        key: PatchKey(1),
                        doctype: None,
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "a".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(3),
                        name: "b".into(),
                        attributes: Vec::new(),
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
            )
            .expect("bootstrap apply");

        let before = store
            .materialize(h)
            .expect("materialize before failed apply");
        let before = stable_dom_lines(&before);

        let err = store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::AppendChild {
                    parent: PatchKey(3),
                    child: PatchKey(2),
                }],
            )
            .expect_err("back-edge append should be rejected");
        assert!(matches!(
            err,
            DomPatchError::CycleDetected {
                parent: PatchKey(3),
                child: PatchKey(2)
            }
        ));

        let after = store
            .materialize(h)
            .expect("materialize after failed apply");
        let after = stable_dom_lines(&after);
        assert_eq!(before, after, "cycle failure must not partially commit");

        let err = store
            .apply(
                h,
                v2,
                v3,
                &[DomPatch::CreateComment {
                    key: PatchKey(999),
                    text: "late".to_string(),
                }],
            )
            .expect_err("advanced from-version should mismatch");
        assert!(matches!(err, DomPatchError::VersionMismatch { .. }));

        store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::CreateComment {
                    key: PatchKey(4),
                    text: "ok".to_string(),
                }],
            )
            .expect("version should remain unchanged after failed batch");
    }

    #[test]
    fn remove_root_without_clear_is_rejected_and_atomic() {
        let mut store = DomStore::new();
        let h = handle(18);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                }],
            )
            .expect("bootstrap apply");

        let before = store
            .materialize(h)
            .expect("materialize before failed apply");
        let before = stable_dom_lines(&before);

        let err = store
            .apply(h, v1, v2, &[DomPatch::RemoveNode { key: PatchKey(1) }])
            .expect_err("root removal without Clear should be rejected");
        assert!(matches!(
            err,
            DomPatchError::Protocol(msg) if msg.contains("rootless")
        ));

        let after = store
            .materialize(h)
            .expect("materialize after failed apply");
        let after = stable_dom_lines(&after);
        assert_eq!(before, after, "failed batch must not partially commit");

        store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::CreateComment {
                    key: PatchKey(2),
                    text: "ok".to_string(),
                }],
            )
            .expect("version should remain unchanged after failed batch");
    }

    #[test]
    fn key_reuse_is_rejected_until_clear_then_allowed() {
        let mut store = DomStore::new();
        let h = handle(19);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();
        let v3 = v2.next();
        let v4 = v3.next();
        let v5 = v4.next();
        let v6 = v5.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[
                    DomPatch::CreateDocument {
                        key: PatchKey(1),
                        doctype: None,
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "div".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::AppendChild {
                        parent: PatchKey(1),
                        child: PatchKey(2),
                    },
                ],
            )
            .expect("bootstrap apply");

        store
            .apply(h, v1, v2, &[DomPatch::RemoveNode { key: PatchKey(2) }])
            .expect("remove node");

        let err = store
            .apply(
                h,
                v2,
                v3,
                &[DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "span".into(),
                    attributes: Vec::new(),
                }],
            )
            .expect_err("key reuse without Clear should be rejected");
        assert!(matches!(err, DomPatchError::DuplicateKey(PatchKey(2))));

        let err = store
            .apply(
                h,
                v3,
                v4,
                &[DomPatch::CreateComment {
                    key: PatchKey(99),
                    text: "nope".to_string(),
                }],
            )
            .expect_err("version must not have advanced after failed duplicate-key batch");
        assert!(matches!(err, DomPatchError::VersionMismatch { .. }));

        // Failed v2->v3 batch must leave version exactly at v2.
        store
            .apply(
                h,
                v2,
                v3,
                &[DomPatch::CreateComment {
                    key: PatchKey(99),
                    text: "still v2".to_string(),
                }],
            )
            .expect("failed batch must not advance version; v2->v3 should still succeed");

        store
            .apply(
                h,
                v3,
                v4,
                &[
                    DomPatch::Clear,
                    DomPatch::CreateDocument {
                        key: PatchKey(10),
                        doctype: None,
                    },
                ],
            )
            .expect("Clear should reset allocation domain");

        store
            .apply(
                h,
                v4,
                v5,
                &[DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "span".into(),
                    attributes: Vec::new(),
                }],
            )
            .expect("key reuse should be allowed after Clear");

        store
            .apply(
                h,
                v5,
                v6,
                &[DomPatch::AppendChild {
                    parent: PatchKey(10),
                    child: PatchKey(2),
                }],
            )
            .expect("reused key should be attachable after Clear");
    }

    #[test]
    fn reattaching_parented_node_returns_move_not_supported() {
        let mut store = DomStore::new();
        let h = handle(20);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[
                    DomPatch::CreateDocument {
                        key: PatchKey(1),
                        doctype: None,
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "a".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(3),
                        name: "b".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::AppendChild {
                        parent: PatchKey(1),
                        child: PatchKey(2),
                    },
                    DomPatch::AppendChild {
                        parent: PatchKey(1),
                        child: PatchKey(3),
                    },
                ],
            )
            .expect("bootstrap apply");

        let err = store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::AppendChild {
                    parent: PatchKey(3),
                    child: PatchKey(2),
                }],
            )
            .expect_err("reattaching a parented node should fail");
        assert!(matches!(
            err,
            DomPatchError::MoveNotSupported { key: PatchKey(2) }
        ));
    }

    #[test]
    fn insert_before_with_parented_node_returns_move_not_supported() {
        let mut store = DomStore::new();
        let h = handle(21);
        store.create(h).expect("create handle");
        let v0 = DomVersion::INITIAL;
        let v1 = v0.next();
        let v2 = v1.next();

        store
            .apply(
                h,
                v0,
                v1,
                &[
                    DomPatch::CreateDocument {
                        key: PatchKey(1),
                        doctype: None,
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(2),
                        name: "child".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::CreateElement {
                        key: PatchKey(4),
                        name: "anchor".into(),
                        attributes: Vec::new(),
                    },
                    DomPatch::AppendChild {
                        parent: PatchKey(1),
                        child: PatchKey(2),
                    },
                    DomPatch::AppendChild {
                        parent: PatchKey(1),
                        child: PatchKey(4),
                    },
                ],
            )
            .expect("bootstrap apply");

        let err = store
            .apply(
                h,
                v1,
                v2,
                &[DomPatch::InsertBefore {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                    before: PatchKey(4),
                }],
            )
            .expect_err("insert_before with already-parented child should fail");
        assert!(matches!(
            err,
            DomPatchError::MoveNotSupported { key: PatchKey(2) }
        ));
    }
}
