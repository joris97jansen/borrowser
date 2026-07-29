use crate::attributes::ParserCreatedAttribute;
use crate::dom_patch::DomPatch;
use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, Attribute, ProcessingInstructionToken, TextValue};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::attributes::{
    resolve_afe_attributes_first_wins, resolve_token_attributes_first_wins,
    snapshot_token_attributes_first_wins,
};
use crate::html5::tree_builder::formatting::AfeElementEntry;
use crate::html5::tree_builder::insert::location::InsertionLocation;
use crate::html5::tree_builder::resolve::resolve_text_value;
use crate::html5::tree_builder::stack::OpenElement;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};
use crate::names::{ElementNamespace, ExpandedElementName};
use std::num::NonZeroU32;

/// Stack disposition is deliberately private to the insertion layer. Tree
/// construction dispatch chooses only semantic normal or void insertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StackDisposition {
    Push,
    PopImmediately,
    /// Preserves pre-AE9 behavior for the deprecated helper only: attach the
    /// node without a stack transition. Separate follow-up work removes this
    /// disposition with the helper and its frozen call sites.
    LegacySkipPush,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LegacySelfClosingEffect {
    None,
    NonVoidHtmlAltersStackDisposition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HtmlInsertionSemantics {
    disposition: StackDisposition,
    self_closing_effect: LegacySelfClosingEffect,
}

struct HtmlElementInsertionPreflight {
    location: InsertionLocation,
    key: PatchKey,
    next_key: NonZeroU32,
    expanded_name: ExpandedElementName,
    attributes: Vec<ParserCreatedAttribute>,
    semantics: HtmlInsertionSemantics,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn insert_foreign_element(
        &mut self,
        namespace: ElementNamespace,
        name: AtomId,
        attributes: Vec<ParserCreatedAttribute>,
        self_closing: bool,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        debug_assert!(namespace != ElementNamespace::Html);
        self.with_structural_mutation(|this| {
            if !self_closing && !this.allow_non_self_closing_element(name, context) {
                return Ok(None);
            }
            let _ = this.ensure_document_created(context)?;
            let location = this.element_or_text_insertion_location()?;
            if !this.allow_new_child(location.parent, Some(name), context) {
                return Ok(None);
            }
            let key = this.alloc_patch_key()?;
            let expanded_name = atoms
                .expanded_name(namespace, name)
                .ok_or(crate::html5::shared::ParserFatalError::EngineInvariant)?;
            this.push_structural_patch(DomPatch::CreateElement {
                key,
                name: expanded_name,
                attributes,
            });
            this.note_node_created();
            let inserted = this.insert_existing_child_at(location, key, context);
            debug_assert!(inserted, "prechecked foreign insertion must succeed");
            let entry = OpenElement::new_foreign(key, namespace, name);
            this.open_elements.push(entry);
            if self_closing {
                let popped = this
                    .open_elements
                    .pop()
                    .ok_or(crate::html5::shared::ParserFatalError::EngineInvariant)?;
                debug_assert_eq!(popped, entry);
            }
            Ok(Some(key))
        })
    }

    pub(in crate::html5::tree_builder) fn create_detached_element(
        &mut self,
        name: AtomId,
        attrs: &[ParserCreatedAttribute],
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        if !self.allow_node_creation(Some(name), context) {
            return Ok(None);
        }
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateElement {
            key,
            name: atoms
                .expanded_name(ElementNamespace::Html, name)
                .ok_or(crate::html5::shared::ParserFatalError::EngineInvariant)?,
            attributes: attrs.to_vec(),
        });
        self.note_node_created();
        Ok(Some(key))
    }

    pub(in crate::html5::tree_builder) fn create_detached_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        let attributes = resolve_afe_attributes_first_wins(&entry.attrs);
        self.create_detached_element(entry.name, &attributes, context, atoms)
    }

    /// Temporary compatibility path for pre-AE9 call sites.
    ///
    /// New parser code must use `insert_normal_html_element` or
    /// `insert_void_html_element`. Separate follow-up work removes this helper
    /// and the frozen call-site expectations that still reference it.
    #[deprecated(note = "frozen legacy insertion helper; removal tracked separately")]
    pub(in crate::html5::tree_builder) fn insert_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        self_closing: bool,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        let disposition = if self_closing || self.known_tags.is_void_tag(name) {
            StackDisposition::LegacySkipPush
        } else {
            StackDisposition::Push
        };
        let self_closing_effect = if self_closing && !self.known_tags.is_void_tag(name) {
            LegacySelfClosingEffect::NonVoidHtmlAltersStackDisposition
        } else {
            LegacySelfClosingEffect::None
        };
        self.insert_html_element_with_private_disposition(
            name,
            attrs,
            context,
            atoms,
            text,
            HtmlInsertionSemantics {
                disposition,
                self_closing_effect,
            },
        )
    }

    /// Compatibility insertion for a production rule that explicitly
    /// acknowledges the token's self-closing flag.
    ///
    /// Some supported legacy void names are not yet part of the old
    /// `is_void_tag` stack policy. This keeps their existing DOM behavior while
    /// preventing the acknowledged flag from being mislabeled as the
    /// non-void `LegacySkipPush` deviation.
    #[deprecated(note = "frozen legacy insertion helper; removal tracked separately")]
    pub(in crate::html5::tree_builder) fn insert_element_for_acknowledged_void_rule(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        self_closing: bool,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        let disposition = if self_closing || self.known_tags.is_void_tag(name) {
            StackDisposition::LegacySkipPush
        } else {
            StackDisposition::Push
        };
        self.insert_html_element_with_private_disposition(
            name,
            attrs,
            context,
            atoms,
            text,
            HtmlInsertionSemantics {
                disposition,
                self_closing_effect: LegacySelfClosingEffect::None,
            },
        )
    }

    /// Inserts an implemented non-void HTML element and retains it on the
    /// stack of open elements.
    pub(in crate::html5::tree_builder) fn insert_normal_html_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
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
            context,
            atoms,
            text,
            HtmlInsertionSemantics {
                disposition: StackDisposition::Push,
                self_closing_effect: LegacySelfClosingEffect::None,
            },
        )
    }

    /// Inserts an implemented void HTML element through a bounded, real stack
    /// push/pop transition. The transient entry is never observable outside
    /// this insertion operation.
    pub(in crate::html5::tree_builder) fn insert_void_html_element(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
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
            context,
            atoms,
            text,
            HtmlInsertionSemantics {
                disposition: StackDisposition::PopImmediately,
                self_closing_effect: LegacySelfClosingEffect::None,
            },
        )
    }

    fn insert_html_element_with_private_disposition(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
        text: &dyn TextResolver,
        semantics: HtmlInsertionSemantics,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let Some(preflight) = this
                .preflight_html_element_insertion(name, attrs, context, atoms, text, semantics)?
            else {
                return Ok(None);
            };

            if preflight.semantics.self_closing_effect
                == LegacySelfClosingEffect::NonVoidHtmlAltersStackDisposition
            {
                context.mark_self_closing_flag_altered_html_stack_disposition()?;
                this.record_tree_implementation_diagnostic(
                    context,
                    crate::html5::shared::TreeConstructionImplementationDiagnosticCode::
                        NonVoidHtmlSelfClosingFlagAlteredStackDisposition,
                    Some("non-void-html-self-closing-flag-altered-stack-disposition"),
                );
            }

            // Preflight completed every fallible operation. From this point the
            // selected stack disposition is committed together with the
            // structural insertion.
            this.next_patch_key = preflight.next_key;
            this.push_structural_patch(DomPatch::CreateElement {
                key: preflight.key,
                name: preflight.expanded_name,
                attributes: preflight.attributes,
            });
            this.note_node_created();
            let inserted =
                this.insert_existing_child_at(preflight.location, preflight.key, context);
            debug_assert!(
                inserted,
                "newly created element insertion must succeed after preflight"
            );

            let entry = OpenElement::new_html(preflight.key, name);
            match preflight.semantics.disposition {
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
            Ok(Some(preflight.key))
        })
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "preflight keeps the production insertion dependencies and semantic stack effect explicit"
    )]
    fn preflight_html_element_insertion(
        &mut self,
        name: AtomId,
        attrs: &[Attribute],
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
        text: &dyn TextResolver,
        semantics: HtmlInsertionSemantics,
    ) -> Result<Option<HtmlElementInsertionPreflight>, TreeBuilderError> {
        if (semantics.disposition == StackDisposition::Push
            || semantics.self_closing_effect
                == LegacySelfClosingEffect::NonVoidHtmlAltersStackDisposition)
            && !self.allow_non_self_closing_element(name, context)
        {
            return Ok(None);
        }

        let _ = self.ensure_document_created(context)?;
        let location = self.element_or_text_insertion_location()?;
        if !self.allow_new_child(location.parent, Some(name), context) {
            return Ok(None);
        }
        let attributes = resolve_token_attributes_first_wins(attrs, atoms, text)?;
        if !self.allow_node_creation(Some(name), context) {
            return Ok(None);
        }
        let key_value = self.next_patch_key.get();
        let next_value = key_value
            .checked_add(1)
            .ok_or(crate::html5::shared::ParserFatalError::EngineInvariant)?;
        let next_key = NonZeroU32::new(next_value)
            .ok_or(crate::html5::shared::ParserFatalError::EngineInvariant)?;
        let expanded_name = atoms
            .expanded_name(ElementNamespace::Html, name)
            .ok_or(crate::html5::shared::ParserFatalError::EngineInvariant)?;
        Ok(Some(HtmlElementInsertionPreflight {
            location,
            key: PatchKey(key_value),
            next_key,
            expanded_name,
            attributes,
            semantics,
        }))
    }

    pub(in crate::html5::tree_builder) fn insert_element_from_afe_entry(
        &mut self,
        entry: &AfeElementEntry,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        atoms: &AtomTable,
    ) -> Result<Option<PatchKey>, TreeBuilderError> {
        self.with_structural_mutation(|this| {
            if !this.allow_non_self_closing_element(entry.name, context) {
                return Ok(None);
            }
            let _ = this.ensure_document_created(context)?;
            let location = this.element_or_text_insertion_location()?;
            if !this.allow_new_child(location.parent, Some(entry.name), context) {
                return Ok(None);
            }
            let Some(key) = this.create_detached_element_from_afe_entry(entry, context, atoms)?
            else {
                return Ok(None);
            };
            let inserted = this.insert_existing_child_at(location, key, context);
            debug_assert!(
                inserted,
                "newly created AFE element insertion must succeed after precheck"
            );
            this.open_elements
                .push(OpenElement::new_html(key, entry.name));
            Ok(Some(key))
        })
    }

    pub(in crate::html5::tree_builder) fn snapshot_afe_attributes(
        &self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Vec<ParserCreatedAttribute>, TreeBuilderError> {
        snapshot_token_attributes_first_wins(attrs, atoms, text)
    }

    pub(in crate::html5::tree_builder) fn insert_comment(
        &mut self,
        token_text: &TextValue,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let resolved = resolve_text_value(token_text, text)?;
            let document_key = this.ensure_document_created(context)?;
            let parent = this
                .open_elements
                .current()
                .map(OpenElement::key)
                .unwrap_or(document_key);
            let parent = this.live_tree.template_contents(parent).unwrap_or(parent);
            this.append_comment_child(parent, resolved, context)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_initial_comment(
        &mut self,
        token_text: &TextValue,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let resolved = resolve_text_value(token_text, text)?;
            let document_key = this.ensure_document_created_for_initial_node()?;
            this.append_comment_child(document_key, resolved, context)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_document_comment(
        &mut self,
        token_text: &TextValue,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.with_structural_mutation(|this| {
            let resolved = resolve_text_value(token_text, text)?;
            let document_key = this.ensure_document_created(context)?;
            this.append_comment_child(document_key, resolved, context)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_processing_instruction(
        &mut self,
        token: &ProcessingInstructionToken,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
        override_parent: Option<PatchKey>,
    ) -> Result<(), TreeBuilderError> {
        let data = resolve_text_value(&token.data, text)?;
        crate::processing_instruction::validate_parser_created_processing_instruction(
            &token.target,
            &data,
        )
        .map_err(|_| crate::html5::shared::ParserFatalError::EngineInvariant)?;
        self.with_structural_mutation(|this| {
            let _ = this.ensure_document_created(context)?;
            let location = this.adjusted_insertion_location(override_parent)?;
            this.append_processing_instruction_at(location, token.target.clone(), data, context)
        })
    }

    pub(in crate::html5::tree_builder) fn insert_initial_processing_instruction(
        &mut self,
        token: &ProcessingInstructionToken,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let data = resolve_text_value(&token.data, text)?;
        crate::processing_instruction::validate_parser_created_processing_instruction(
            &token.target,
            &data,
        )
        .map_err(|_| crate::html5::shared::ParserFatalError::EngineInvariant)?;
        self.with_structural_mutation(|this| {
            let document_key = this.ensure_document_created_for_initial_node()?;
            let location = this.adjusted_insertion_location(Some(document_key))?;
            this.append_processing_instruction_at(location, token.target.clone(), data, context)
        })
    }

    fn append_processing_instruction_at(
        &mut self,
        location: super::InsertionLocation,
        target: String,
        data: String,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> Result<(), TreeBuilderError> {
        if !self.allow_new_child(location.parent, None, context)
            || !self.allow_node_creation(None, context)
        {
            return Ok(());
        }
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateProcessingInstruction { key, target, data });
        self.note_node_created();
        let inserted = self.insert_existing_child_at(location, key, context);
        debug_assert!(inserted, "prechecked PI insertion must succeed");
        Ok(())
    }

    fn append_comment_child(
        &mut self,
        parent: PatchKey,
        text: String,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> Result<(), TreeBuilderError> {
        if !self.allow_new_child(parent, None, context) || !self.allow_node_creation(None, context)
        {
            return Ok(());
        }
        let key = self.alloc_patch_key()?;
        self.push_structural_patch(DomPatch::CreateComment { key, text });
        self.note_node_created();
        let inserted = self.append_existing_child(parent, key, context);
        debug_assert!(
            inserted,
            "newly created comment insertion must succeed after precheck"
        );
        Ok(())
    }
}
