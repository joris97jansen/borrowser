use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomId, AtomTable, Attribute, TextValue};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::coalescing::LastTextPatch;
use crate::html5::tree_builder::formatting::{AfeAttributeSnapshot, AfeElementEntry};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::{
    resolve_atom_arc, resolve_attribute_value, resolve_text_value,
};
use crate::html5::tree_builder::stack::{OpenElement, ScopeKind};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    fn current_insertion_parent(&self) -> Result<PatchKey, TreeBuilderError> {
        let document_key = self
            .document_key
            .ok_or(crate::html5::shared::EngineInvariantError)?;
        Ok(self
            .open_elements
            .current()
            .map(OpenElement::key)
            .unwrap_or(document_key))
    }

    pub(in crate::html5::tree_builder) fn append_existing_child(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
    ) {
        self.push_structural_patch(DomPatch::AppendChild { parent, child });
    }

    #[allow(
        dead_code,
        reason = "kept for upcoming AAA parser integration and foster-parent insertion work"
    )]
    pub(in crate::html5::tree_builder) fn insert_existing_child_before(
        &mut self,
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    ) {
        self.push_structural_patch(DomPatch::InsertBefore {
            parent,
            child,
            before,
        });
    }

    pub(in crate::html5::tree_builder) fn create_detached_element(
        &mut self,
        name: AtomId,
        attrs: &[(std::sync::Arc<str>, Option<String>)],
        atoms: &AtomTable,
    ) -> Result<PatchKey, TreeBuilderError> {
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateElement {
            key,
            name: resolve_atom_arc(atoms, name)?,
            attributes: attrs.to_vec(),
        });
        Ok(key)
    }

    fn create_detached_element_from_token_attrs(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<PatchKey, TreeBuilderError> {
        let mut attributes = Vec::with_capacity(attrs.len());
        for attr in attrs {
            let attr_name = resolve_atom_arc(atoms, attr.name)?;
            let attr_value = resolve_attribute_value(attr, text)?;
            attributes.push((attr_name, attr_value));
        }
        self.create_detached_element(name, &attributes, atoms)
    }

    pub(in crate::html5::tree_builder) fn create_detached_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        atoms: &AtomTable,
    ) -> Result<PatchKey, TreeBuilderError> {
        let mut attributes = Vec::with_capacity(entry.attrs.len());
        for attr in &entry.attrs {
            let attr_name = resolve_atom_arc(atoms, attr.name)?;
            attributes.push((attr_name, attr.value.clone()));
        }
        self.create_detached_element(entry.name, &attributes, atoms)
    }

    pub(in crate::html5::tree_builder) fn insert_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<PatchKey, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let _ = this.ensure_document_created()?;
            let parent = this.current_insertion_parent()?;
            let key = this.create_detached_element_from_token_attrs(name, attrs, atoms, text)?;
            this.append_existing_child(parent, key);
            if !self_closing {
                this.open_elements.push(OpenElement::new(key, name));
            }
            Ok(key)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        atoms: &AtomTable,
    ) -> Result<PatchKey, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let _ = this.ensure_document_created()?;
            let parent = this.current_insertion_parent()?;
            let key = this.create_detached_element_from_afe_entry(entry, atoms)?;
            this.append_existing_child(parent, key);
            this.open_elements.push(OpenElement::new(key, entry.name));
            Ok(key)
        })
    }

    pub(in crate::html5::tree_builder) fn snapshot_afe_attributes(
        &self,
        attrs: &[Attribute],
        text: &dyn TextResolver,
    ) -> Result<Vec<AfeAttributeSnapshot>, TreeBuilderError> {
        let mut snapshot = Vec::with_capacity(attrs.len());
        for attr in attrs {
            snapshot.push(AfeAttributeSnapshot::new(
                attr.name,
                resolve_attribute_value(attr, text)?,
            ));
        }
        Ok(snapshot)
    }

    pub(in crate::html5::tree_builder) fn insert_text(
        &mut self,
        token_text: &TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let resolved = resolve_text_value(token_text, text)?;
        self.insert_resolved_text(&resolved)
    }

    pub(in crate::html5::tree_builder) fn insert_literal_text(
        &mut self,
        literal: &str,
    ) -> Result<(), TreeBuilderError> {
        self.insert_resolved_text(literal)
    }

    pub(in crate::html5::tree_builder) fn insert_recovery_literal_text(
        &mut self,
        literal: &str,
    ) -> Result<(), TreeBuilderError> {
        self.invalidate_text_coalescing();
        self.insert_literal_text(literal)?;
        self.invalidate_text_coalescing();
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn insert_resolved_text(
        &mut self,
        resolved: &str,
    ) -> Result<(), TreeBuilderError> {
        debug_assert_eq!(self.structural_mutation_depth, 0);
        if resolved.is_empty() {
            return Ok(());
        }
        let document_key = self.ensure_document_created()?;
        let parent = self
            .open_elements
            .current()
            .map(OpenElement::key)
            .unwrap_or(document_key);
        if self.config.coalesce_text
            && let Some(last) = self.last_text_patch.as_ref()
            && last.parent == parent
        {
            let key = self
                .last_text_patch
                .as_ref()
                .expect("coalescing state must remain present within branch")
                .text_key;
            self.push_patch(DomPatch::AppendText {
                key,
                text: resolved.to_string(),
            });
            self.perf_text_appends = self.perf_text_appends.saturating_add(1);
            return Ok(());
        }
        let key = self.with_structural_mutation(|this| {
            let key = this.alloc_patch_key()?;
            this.push_structural_patch(DomPatch::CreateText {
                key,
                text: resolved.to_string(),
            });
            this.push_structural_patch(DomPatch::AppendChild { parent, child: key });
            this.perf_text_nodes_created = this.perf_text_nodes_created.saturating_add(1);
            Ok(key)
        })?;
        self.last_text_patch = if self.config.coalesce_text {
            Some(LastTextPatch {
                parent,
                text_key: key,
            })
        } else {
            None
        };
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn insert_comment(
        &mut self,
        token_text: &TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let resolved = resolve_text_value(token_text, text)?;
            let document_key = this.ensure_document_created()?;
            let parent = this
                .open_elements
                .current()
                .map(OpenElement::key)
                .unwrap_or(document_key);
            let key = this.alloc_patch_key()?;
            this.push_structural_patch(DomPatch::CreateComment {
                key,
                text: resolved,
            });
            this.push_structural_patch(DomPatch::AppendChild { parent, child: key });
            Ok(())
        })
    }

    pub(in crate::html5::tree_builder) fn close_element_in_scope(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
    ) -> bool {
        self.pop_element_in_scope_with_reporting(name, scope, true)
            .is_some()
    }

    #[inline]
    #[allow(
        dead_code,
        reason = "kept as a convenience wrapper while insertion-mode AFE/AAA integration expands"
    )]
    pub(in crate::html5::tree_builder) fn close_element_in_scope_with_reporting(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
        report_not_in_scope_error: bool,
    ) -> bool {
        self.pop_element_in_scope_with_reporting(name, scope, report_not_in_scope_error)
            .is_some()
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn pop_element_in_scope_with_reporting(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
        report_not_in_scope_error: bool,
    ) -> Option<OpenElement> {
        let popped = self
            .open_elements
            .pop_until_including_in_scope(name, scope, &self.scope_tags);
        if popped.is_none() {
            if report_not_in_scope_error {
                self.record_parse_error("end-tag-not-in-scope", Some(name), None);
            }
            return None;
        }
        self.invalidate_text_coalescing();
        popped
    }

    pub(in crate::html5::tree_builder) fn update_mode_for_start_tag(&mut self, name: AtomId) {
        self.insertion_mode = if name == self.known_tags.html {
            InsertionMode::BeforeHead
        } else if name == self.known_tags.head {
            InsertionMode::InHead
        } else {
            InsertionMode::InBody
        };
    }

    pub(in crate::html5::tree_builder) fn update_mode_for_end_tag(&mut self, name: AtomId) {
        self.insertion_mode = if name == self.known_tags.head {
            InsertionMode::AfterHead
        } else if name == self.known_tags.body {
            InsertionMode::InBody
        } else {
            self.insertion_mode
        };
    }

    pub(in crate::html5::tree_builder) fn scope_kind_for_in_body_end_tag(
        &self,
        name: AtomId,
    ) -> ScopeKind {
        if name == self.known_tags.button {
            ScopeKind::Button
        } else if name == self.known_tags.li {
            ScopeKind::ListItem
        } else if name == self.known_tags.table {
            ScopeKind::Table
        } else {
            ScopeKind::InScope
        }
    }
}
