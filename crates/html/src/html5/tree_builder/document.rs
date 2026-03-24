use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomId, AtomTable};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::resolve_atom;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuirksMode {
    NoQuirks,
    LimitedQuirks,
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
    fn classify_doctype_quirks_mode(
        name: Option<&str>,
        public_id: Option<&str>,
        system_id: Option<&str>,
        force_quirks: bool,
    ) -> QuirksMode {
        if force_quirks {
            return QuirksMode::Quirks;
        }

        let Some(name) = name else {
            return QuirksMode::Quirks;
        };
        if !name.eq_ignore_ascii_case("html") {
            return QuirksMode::Quirks;
        }

        let public_id = public_id.map(str::to_ascii_lowercase);
        let system_id = system_id.map(str::to_ascii_lowercase);

        if public_id.as_deref().is_some_and(|public_id| {
            public_id.starts_with("-//w3c//dtd xhtml 1.0 frameset//")
                || public_id.starts_with("-//w3c//dtd xhtml 1.0 transitional//")
        }) {
            return QuirksMode::LimitedQuirks;
        }

        if public_id.as_deref().is_some_and(|public_id| {
            public_id.starts_with("-//w3c//dtd html 4.01 frameset//")
                || public_id.starts_with("-//w3c//dtd html 4.01 transitional//")
        }) {
            return if system_id.is_some() {
                QuirksMode::LimitedQuirks
            } else {
                QuirksMode::Quirks
            };
        }

        QuirksMode::NoQuirks
    }

    pub(in crate::html5::tree_builder) fn closes_p_before_table_in_body(&self) -> bool {
        self.document_state.quirks_mode != QuirksMode::Quirks
    }

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
            this.foster_parenting_enabled = false;
            this.clear_pending_table_character_tokens();
            this.document_state.frameset_ok = true;
            Ok(key)
        })
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
        if self.document_key.is_none() && self.pending_doctype.is_none() {
            self.pending_doctype = match name {
                Some(id) => Some(resolve_atom(atoms, *id)?.to_string()),
                None => None,
            };
        }
        let resolved_name = match name {
            Some(id) => Some(resolve_atom(atoms, *id)?),
            None => None,
        };
        self.document_state.quirks_mode =
            Self::classify_doctype_quirks_mode(resolved_name, public_id, system_id, force_quirks);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::QuirksMode;
    use crate::html5::tree_builder::Html5TreeBuilder;

    #[test]
    fn doctype_classifier_distinguishes_no_limited_and_quirks_modes() {
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(Some("html"), None, None, false),
            QuirksMode::NoQuirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(
                Some("html"),
                Some("-//W3C//DTD XHTML 1.0 Transitional//EN"),
                Some("http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd"),
                false,
            ),
            QuirksMode::LimitedQuirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(
                Some("html"),
                Some("-//W3C//DTD HTML 4.01 Transitional//EN"),
                Some("http://www.w3.org/TR/html4/loose.dtd"),
                false,
            ),
            QuirksMode::LimitedQuirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(
                Some("html"),
                Some("-//W3C//DTD HTML 4.01 Transitional//EN"),
                None,
                false,
            ),
            QuirksMode::Quirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(Some("foo"), None, None, false),
            QuirksMode::Quirks
        );
        assert_eq!(
            Html5TreeBuilder::classify_doctype_quirks_mode(None, None, None, true),
            QuirksMode::Quirks
        );
    }
}
