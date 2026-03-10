use super::arena::{DomArena, NodeKind};
use super::error::DomPatchError;
use core_types::DomVersion;
use html::{DomPatch, Node, PatchKey};
use std::sync::Arc;

pub(crate) struct DomDoc {
    pub(crate) version: DomVersion,
    arena: DomArena,
    root: Option<PatchKey>,
    pub(crate) current: Option<Box<Node>>,
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
    pub(crate) fn new() -> Self {
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

    pub(crate) fn apply(&mut self, patches: &[DomPatch]) -> Result<(), DomPatchError> {
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

    pub(crate) fn rebuild_cache(&mut self) -> Result<(), DomPatchError> {
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

    pub(crate) fn materialize_owned(&self) -> Result<Node, DomPatchError> {
        let Some(root) = self.root else {
            return Err(DomPatchError::MissingRoot);
        };
        // TODO(v5.1): materialization is O(n); render pipeline should consume the arena directly.
        self.arena.materialize(root)
    }
}
