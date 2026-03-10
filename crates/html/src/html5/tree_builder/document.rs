use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomId, AtomTable};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::resolve_atom;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuirksMode {
    NoQuirks,
    Quirks,
}

#[derive(Clone, Debug)]
pub(crate) struct DocumentState {
    pub(crate) quirks_mode: QuirksMode,
    pub(crate) frameset_ok: bool,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            quirks_mode: QuirksMode::NoQuirks,
            frameset_ok: true,
        }
    }
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn ensure_document_created(
        &mut self,
    ) -> Result<PatchKey, TreeBuilderError> {
        if let Some(key) = self.document_key {
            return Ok(key);
        }
        self.with_structural_mutation(|this| {
            let key = this.alloc_patch_key()?;
            let doctype = this.pending_doctype.take();
            this.push_structural_patch(DomPatch::CreateDocument { key, doctype });
            this.document_key = Some(key);
            this.insertion_mode = InsertionMode::BeforeHtml;
            debug_assert!(
                this.open_elements.is_empty(),
                "document creation expected empty SOE before bootstrap reset (len={})",
                this.open_elements.len()
            );
            this.open_elements.clear();
            debug_assert!(
                this.active_formatting.is_empty(),
                "document creation expected empty AFE before bootstrap reset (len={})",
                this.active_formatting.len()
            );
            this.active_formatting.clear();
            this.original_insertion_mode = None;
            this.active_text_mode = None;
            this.document_state.frameset_ok = true;
            Ok(key)
        })
    }

    pub(in crate::html5::tree_builder) fn handle_doctype(
        &mut self,
        name: &Option<AtomId>,
        force_quirks: bool,
        atoms: &AtomTable,
    ) -> Result<(), TreeBuilderError> {
        self.invalidate_text_coalescing();
        if self.document_key.is_none() && self.pending_doctype.is_none() {
            self.pending_doctype = match name {
                Some(id) => Some(resolve_atom(atoms, *id)?.to_string()),
                None => None,
            };
        }
        if force_quirks {
            self.document_state.quirks_mode = QuirksMode::Quirks;
        }
        Ok(())
    }
}
