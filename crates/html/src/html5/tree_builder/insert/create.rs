use crate::dom_patch::DomPatch;
use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, Attribute, TextValue};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::formatting::{AfeAttributeSnapshot, AfeElementEntry};
use crate::html5::tree_builder::resolve::{
    resolve_atom_arc, resolve_attribute_value, resolve_text_value,
};
use crate::html5::tree_builder::stack::OpenElement;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn create_detached_element(
        &mut self,
        name: AtomId,
        attrs: &[(std::sync::Arc<str>, Option<String>)],
        atoms: &AtomTable,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        if !self.allow_node_creation(Some(name)) {
            return Ok(None);
        }
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateElement {
            key,
            name: resolve_atom_arc(atoms, name)?,
            attributes: attrs.to_vec(),
        });
        self.note_node_created();
        Ok(Some(key))
    }

    fn create_detached_element_from_token_attrs(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
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
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
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
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            if !self_closing && !this.allow_non_self_closing_element(name) {
                return Ok(None);
            }
            let _ = this.ensure_document_created()?;
            let location = this.element_or_text_insertion_location()?;
            if !this.allow_new_child(location.parent, Some(name)) {
                return Ok(None);
            }
            let Some(key) =
                this.create_detached_element_from_token_attrs(name, attrs, atoms, text)?
            else {
                return Ok(None);
            };
            let inserted = this.insert_existing_child_at(location, key);
            debug_assert!(
                inserted,
                "newly created element insertion must succeed after precheck"
            );
            if !self_closing {
                this.open_elements.push(OpenElement::new(key, name));
            }
            Ok(Some(key))
        })
    }

    pub(in crate::html5::tree_builder) fn insert_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        atoms: &AtomTable,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            if !this.allow_non_self_closing_element(entry.name) {
                return Ok(None);
            }
            let _ = this.ensure_document_created()?;
            let location = this.element_or_text_insertion_location()?;
            if !this.allow_new_child(location.parent, Some(entry.name)) {
                return Ok(None);
            }
            let Some(key) = this.create_detached_element_from_afe_entry(entry, atoms)? else {
                return Ok(None);
            };
            let inserted = this.insert_existing_child_at(location, key);
            debug_assert!(
                inserted,
                "newly created AFE element insertion must succeed after precheck"
            );
            this.open_elements.push(OpenElement::new(key, entry.name));
            Ok(Some(key))
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
            if !this.allow_new_child(parent, None) || !this.allow_node_creation(None) {
                return Ok(());
            }
            let key = this.alloc_patch_key()?;
            this.push_structural_patch(DomPatch::CreateComment {
                key,
                text: resolved,
            });
            this.note_node_created();
            let inserted = this.append_existing_child(parent, key);
            debug_assert!(
                inserted,
                "newly created comment insertion must succeed after precheck"
            );
            Ok(())
        })
    }
}
