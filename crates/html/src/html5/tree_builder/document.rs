use crate::DocumentMode;
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomId, AtomTable};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::resolve_atom;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Debug)]
pub(crate) struct DocumentState {
    pub(crate) quirks_mode: DocumentMode,
    pub(crate) frameset_ok: bool,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            quirks_mode: DocumentMode::NoQuirks,
            frameset_ok: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct PendingDoctype {
    pub(in crate::html5::tree_builder) name: Option<String>,
    pub(in crate::html5::tree_builder) public_id: Option<String>,
    pub(in crate::html5::tree_builder) system_id: Option<String>,
}

impl Html5TreeBuilder {
    fn classify_doctype_quirks_mode(
        name: Option<&str>,
        public_id: Option<&str>,
        system_id: Option<&str>,
        force_quirks: bool,
    ) -> DocumentMode {
        if force_quirks {
            return DocumentMode::Quirks;
        }

        let Some(name) = name else {
            return DocumentMode::Quirks;
        };
        if !name.eq_ignore_ascii_case("html") {
            return DocumentMode::Quirks;
        }

        let public_id = public_id.map(str::to_ascii_lowercase);
        let system_id = system_id.map(str::to_ascii_lowercase);

        if public_id.as_deref().is_some_and(|public_id| {
            public_id.starts_with("-//w3c//dtd xhtml 1.0 frameset//")
                || public_id.starts_with("-//w3c//dtd xhtml 1.0 transitional//")
        }) {
            return DocumentMode::LimitedQuirks;
        }

        if public_id.as_deref().is_some_and(|public_id| {
            public_id.starts_with("-//w3c//dtd html 4.01 frameset//")
                || public_id.starts_with("-//w3c//dtd html 4.01 transitional//")
        }) {
            return if system_id.is_some() {
                DocumentMode::LimitedQuirks
            } else {
                DocumentMode::Quirks
            };
        }

        DocumentMode::NoQuirks
    }

    pub(in crate::html5::tree_builder) fn closes_p_before_table_in_body(&self) -> bool {
        self.document_state.quirks_mode != DocumentMode::Quirks
    }

    pub(in crate::html5::tree_builder) fn ensure_document_created(
        &mut self,
    ) -> Result<PatchKey, TreeBuilderError> {
        if let Some(key) = self.document_key {
            return Ok(key);
        }
        self.with_structural_mutation(|this| {
            let key = this.create_document_node()?;
            let doctype = this.pending_doctype.take();
            if let Some(doctype) = doctype {
                this.append_doctype_child(key, doctype)?;
            }
            this.finish_document_bootstrap();
            Ok(key)
        })
    }

    /// Creates the Document for an Initial-mode non-doctype node without
    /// completing bootstrap, so a following conforming doctype is still
    /// accepted in source order.
    pub(in crate::html5::tree_builder) fn ensure_document_created_for_initial_node(
        &mut self,
    ) -> Result<PatchKey, TreeBuilderError> {
        if let Some(key) = self.document_key {
            return Ok(key);
        }
        assert!(
            self.pending_doctype.is_none(),
            "initial-node bootstrap must precede pending doctype capture"
        );
        self.create_document_node()
    }

    pub(in crate::html5::tree_builder) fn handle_doctype(
        &mut self,
        name: &Option<AtomId>,
        public_id: Option<&str>,
        system_id: Option<&str>,
        force_quirks: bool,
        atoms: &AtomTable,
    ) -> Result<(), TreeBuilderError> {
        self.invalidate_text_coalescing();
        let resolved_name = match name {
            Some(id) => Some(resolve_atom(atoms, *id)?),
            None => None,
        };
        let doctype = PendingDoctype {
            name: resolved_name.map(str::to_string),
            public_id: public_id.map(str::to_string),
            system_id: system_id.map(str::to_string),
        };

        if self.document_key.is_none() && self.pending_doctype.is_none() {
            self.pending_doctype = Some(doctype);
        } else if let Some(document_key) = self.document_key
            && self.insertion_mode == InsertionMode::Initial
            && self.pending_doctype.is_none()
            && !self.live_tree.has_document_type_child(document_key)
        {
            self.with_structural_mutation(|this| this.append_doctype_child(document_key, doctype))?;
        }

        self.document_state.quirks_mode =
            Self::classify_doctype_quirks_mode(resolved_name, public_id, system_id, force_quirks);
        Ok(())
    }

    fn create_document_node(&mut self) -> Result<PatchKey, TreeBuilderError> {
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateDocument { key, doctype: None });
        self.document_key = Some(key);
        Ok(key)
    }

    fn append_doctype_child(
        &mut self,
        document_key: PatchKey,
        doctype: PendingDoctype,
    ) -> Result<(), TreeBuilderError> {
        if !self.allow_node_creation(None) || !self.allow_new_child(document_key, None) {
            return Ok(());
        }

        let doctype_key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateDocumentType {
            key: doctype_key,
            name: doctype.name,
            public_id: doctype.public_id,
            system_id: doctype.system_id,
        });
        self.note_node_created();
        let inserted = self.append_existing_child(document_key, doctype_key);
        debug_assert!(
            inserted,
            "newly created doctype insertion must succeed after precheck"
        );
        Ok(())
    }

    fn finish_document_bootstrap(&mut self) {
        self.insertion_mode = InsertionMode::BeforeHtml;
        debug_assert!(
            self.open_elements.is_empty(),
            "document creation expected empty SOE before bootstrap reset (len={})",
            self.open_elements.len()
        );
        self.open_elements.clear();
        debug_assert!(
            self.active_formatting.is_empty(),
            "document creation expected empty AFE before bootstrap reset (len={})",
            self.active_formatting.len()
        );
        self.active_formatting.clear();
        self.original_insertion_mode = None;
        self.active_text_mode = None;
        self.foster_parenting_enabled = false;
        self.clear_pending_table_text_state();
        self.document_state.frameset_ok = true;
    }
}

#[cfg(test)]
mod tests {
    use crate::DocumentMode;
    use crate::html5::tree_builder::Html5TreeBuilder;

    #[test]
    fn doctype_classifier_distinguishes_no_limited_and_quirks_modes() {
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(Some("html"), None, None, false),
            DocumentMode::NoQuirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(
                Some("html"),
                Some("-//W3C//DTD XHTML 1.0 Transitional//EN"),
                Some("http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd"),
                false,
            ),
            DocumentMode::LimitedQuirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(
                Some("html"),
                Some("-//W3C//DTD HTML 4.01 Transitional//EN"),
                Some("http://www.w3.org/TR/html4/loose.dtd"),
                false,
            ),
            DocumentMode::LimitedQuirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(
                Some("html"),
                Some("-//W3C//DTD HTML 4.01 Transitional//EN"),
                None,
                false,
            ),
            DocumentMode::Quirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(Some("foo"), None, None, false),
            DocumentMode::Quirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(None, None, None, true),
            DocumentMode::Quirks
        );
    }
}
