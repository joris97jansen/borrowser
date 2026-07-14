use crate::dom_patch::DomPatch;
use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, Attribute, TextValue};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::attributes::{
    ParserCreatedAttribute, resolve_afe_attributes_first_wins, resolve_token_attributes_first_wins,
    snapshot_token_attributes_first_wins,
};
use crate::html5::tree_builder::formatting::{AfeAttributeSnapshot, AfeElementEntry};
use crate::html5::tree_builder::resolve::{resolve_atom_arc, resolve_text_value};
use crate::html5::tree_builder::stack::OpenElement;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

/// Stack disposition is deliberately private to the insertion layer. Tree
/// construction dispatch chooses only semantic normal or void insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StackDisposition {
    Push,
    PopImmediately,
    /// Preserves pre-AE9 behavior for the deprecated helper only: attach the
    /// node without a stack transition. AE9b removes this disposition with the
    /// helper and its frozen call sites.
    LegacySkipPush,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn create_detached_element(
        &mut self,
        name: AtomId,
        attrs: &[ParserCreatedAttribute],
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
        let attributes = resolve_token_attributes_first_wins(attrs, atoms, text)?;
        self.create_detached_element(name, &attributes, atoms)
    }

    pub(in crate::html5::tree_builder) fn create_detached_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        atoms: &AtomTable,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        let attributes = resolve_afe_attributes_first_wins(&entry.attrs, atoms)?;
        self.create_detached_element(entry.name, &attributes, atoms)
    }

    /// Temporary compatibility path for pre-AE9 call sites.
    ///
    /// New parser code must use `insert_normal_html_element` or
    /// `insert_void_html_element`. AE9b removes this helper and the frozen
    /// call-site expectations that still reference it.
    #[deprecated(note = "frozen legacy insertion helper; removal owned by AE9b")]
    pub(in crate::html5::tree_builder) fn insert_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        let disposition = if self_closing || self.known_tags.is_void_tag(name) {
            StackDisposition::LegacySkipPush
        } else {
            StackDisposition::Push
        };
        self.insert_html_element_with_private_disposition(name, attrs, atoms, text, disposition)
    }

    /// Inserts an implemented non-void HTML element and retains it on the
    /// stack of open elements.
    pub(in crate::html5::tree_builder) fn insert_normal_html_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        assert!(
            !self.known_tags.is_void_tag(name),
            "normal insertion received parser-classified void HTML element"
        );
        self.insert_html_element_with_private_disposition(
            name,
            attrs,
            atoms,
            text,
            StackDisposition::Push,
        )
    }

    /// Inserts an implemented void HTML element through a bounded, real stack
    /// push/pop transition. The transient entry is never observable outside
    /// this insertion operation.
    pub(in crate::html5::tree_builder) fn insert_void_html_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        assert!(
            self.known_tags.is_void_tag(name),
            "void insertion received non-void HTML element"
        );
        self.insert_html_element_with_private_disposition(
            name,
            attrs,
            atoms,
            text,
            StackDisposition::PopImmediately,
        )
    }

    fn insert_html_element_with_private_disposition(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
        disposition: StackDisposition,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            if disposition == StackDisposition::Push && !this.allow_non_self_closing_element(name) {
                return Ok(None);
            }

            // All fallible resource checks complete before a stack transition.
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

            let entry = OpenElement::new(key, name);
            match disposition {
                StackDisposition::Push => this.open_elements.push(entry),
                StackDisposition::PopImmediately => {
                    let length_before = this.open_elements.len();
                    this.open_elements.push(entry);
                    let popped = this
                        .open_elements
                        .pop()
                        .expect("void insertion push must have a matching pop");
                    assert_eq!(popped, entry, "void insertion must pop its exact entry");
                    assert_eq!(
                        this.open_elements.len(),
                        length_before,
                        "void insertion must restore retained stack depth"
                    );
                }
                StackDisposition::LegacySkipPush => {}
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
        snapshot_token_attributes_first_wins(attrs, text)
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
            this.append_comment_child(parent, resolved)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_initial_comment(
        &mut self,
        token_text: &TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let resolved = resolve_text_value(token_text, text)?;
            let document_key = this.ensure_document_created_for_initial_comment()?;
            this.append_comment_child(document_key, resolved)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_document_comment(
        &mut self,
        token_text: &TextValue,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let resolved = resolve_text_value(token_text, text)?;
            let document_key = this.ensure_document_created()?;
            this.append_comment_child(document_key, resolved)
        })
    }

    fn append_comment_child(
        &mut self,
        parent: PatchKey,
        text: String,
    ) -> Result<(), TreeBuilderError> {
        if !self.allow_new_child(parent, None) || !self.allow_node_creation(None) {
            return Ok(());
        }
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateComment { key, text });
        self.note_node_created();
        let inserted = self.append_existing_child(parent, key);
        debug_assert!(
            inserted,
            "newly created comment insertion must succeed after precheck"
        );
        Ok(())
    }
}
