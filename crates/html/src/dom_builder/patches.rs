use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::NodeKey;

use super::{TreeBuilder, TreeBuilderError, TreeBuilderResult};

impl TreeBuilder {
    pub(super) fn emit_patch(&mut self, patch: DomPatch) -> TreeBuilderResult<()> {
        #[cfg(any(test, debug_assertions))]
        {
            let invalid = patch_has_invalid_key(&patch);
            debug_assert!(!invalid, "patch emission must not use invalid keys");
            #[cfg(test)]
            assert!(!invalid, "patch emission must not use invalid keys");
            if !self.document_emitted {
                debug_assert!(
                    matches!(patch, DomPatch::CreateDocument { .. }),
                    "CreateDocument must be the first emitted patch"
                );
            }
        }
        if patch_has_invalid_key(&patch) {
            return Err(TreeBuilderError::InvariantViolation(
                "patch emission used invalid key",
            ));
        }
        if !self.document_emitted && !matches!(patch, DomPatch::CreateDocument { .. }) {
            return Err(TreeBuilderError::Protocol(
                "CreateDocument must be the first emitted patch",
            ));
        }
        self.patches.push(patch);
        Ok(())
    }

    pub(super) fn ensure_document_emitted(&mut self) -> TreeBuilderResult<()> {
        if self.document_emitted {
            return Ok(());
        }
        let doctype = self.arena.doctype(self.root_index).map(|s| s.to_string());
        self.emit_patch(DomPatch::CreateDocument {
            key: patch_key(self.root_key),
            doctype,
        })?;
        self.document_emitted = true;
        Ok(())
    }
}

#[inline]
pub(super) fn patch_key(key: NodeKey) -> PatchKey {
    debug_assert_ne!(key, NodeKey::INVALID, "node key must be valid");
    PatchKey::from_node_key(key)
}

#[inline]
pub(super) fn patch_has_invalid_key(patch: &DomPatch) -> bool {
    match patch {
        DomPatch::Clear => false,
        DomPatch::CreateDocument { key, .. }
        | DomPatch::CreateElement { key, .. }
        | DomPatch::CreateText { key, .. }
        | DomPatch::CreateComment { key, .. }
        | DomPatch::RemoveNode { key }
        | DomPatch::SetAttributes { key, .. }
        | DomPatch::SetText { key, .. }
        | DomPatch::AppendText { key, .. } => *key == PatchKey::INVALID,
        DomPatch::AppendChild { parent, child } => {
            *parent == PatchKey::INVALID || *child == PatchKey::INVALID
        }
        DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => {
            *parent == PatchKey::INVALID
                || *child == PatchKey::INVALID
                || *before == PatchKey::INVALID
        }
    }
}
