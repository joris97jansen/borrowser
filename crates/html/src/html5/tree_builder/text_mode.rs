use crate::html5::shared::{AtomId, AtomTable, Token};
use crate::html5::tokenizer::{TextModeSpec, TextResolver, TokenizerControl};
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::resolve_atom;
use crate::html5::tree_builder::stack::OpenElement;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_text_mode(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Text { text: token_text } => {
                self.insert_text(token_text, text)?;
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
            }
            Token::Eof => {
                self.record_parse_error("eof-in-text-mode", None, None);
                let _ = self.ensure_document_created()?;
                self.exit_text_mode();
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                self.record_parse_error("start-tag-in-text-mode", Some(*name), None);
                let tag_name = resolve_atom(atoms, *name)?;
                if attrs.iter().any(|attr| attr.value.is_some()) {
                    self.record_parse_error(
                        "text-mode-literalized-start-tag-attribute-values-dropped",
                        Some(*name),
                        None,
                    );
                }
                let mut attr_names = Vec::with_capacity(attrs.len());
                for attr in attrs {
                    attr_names.push(resolve_atom(atoms, attr.name)?.to_string());
                }
                attr_names.sort();
                let len_before_dedup = attr_names.len();
                attr_names.dedup();
                if attr_names.len() != len_before_dedup {
                    self.record_parse_error(
                        "text-mode-literalized-start-tag-duplicate-attributes-deduped",
                        Some(*name),
                        None,
                    );
                }
                let mut literal = String::new();
                literal.push('<');
                literal.push_str(tag_name);
                for attr_name in attr_names {
                    literal.push(' ');
                    literal.push_str(&attr_name);
                }
                if *self_closing {
                    literal.push_str("/>");
                } else {
                    literal.push('>');
                }
                self.insert_recovery_literal_text(&literal)?;
            }
            Token::Doctype { .. } => {
                self.record_parse_error("doctype-in-text-mode", None, None);
            }
            Token::EndTag { name } => {
                let closed = self.active_text_mode_end_tag_name() == Some(*name)
                    && self.close_active_text_mode_element();
                if closed {
                    self.exit_text_mode();
                } else {
                    self.record_parse_error(
                        "unexpected-end-tag-in-text-mode",
                        Some(*name),
                        Some(InsertionMode::Text),
                    );
                    self.record_parse_error(
                        "text-mode-end-tag-literalized",
                        Some(*name),
                        Some(InsertionMode::Text),
                    );
                    self.insert_text_mode_end_tag_literal(*name, atoms)?;
                }
            }
        }
        Ok(DispatchOutcome::Done)
    }

    pub(in crate::html5::tree_builder) fn insert_text_mode_end_tag_literal(
        &mut self,
        name: AtomId,
        atoms: &AtomTable,
    ) -> Result<(), TreeBuilderError> {
        debug_assert_eq!(atoms.id(), self.atom_table_id);
        let literal = format!("</{}>", resolve_atom(atoms, name)?);
        self.insert_recovery_literal_text(&literal)
    }

    pub(in crate::html5::tree_builder) fn queue_tokenizer_control(
        &mut self,
        control: TokenizerControl,
    ) {
        assert!(self.pending_tokenizer_control.is_none());
        self.pending_tokenizer_control = Some(control);
    }

    pub(in crate::html5::tree_builder) fn enter_text_mode_for_element(&mut self, name: AtomId) {
        let Some(text_mode) = self.text_mode_spec_for_tag(name) else {
            return;
        };
        debug_assert!(self.original_insertion_mode.is_none());
        debug_assert!(self.active_text_mode.is_none());
        self.original_insertion_mode = Some(self.insertion_mode);
        self.active_text_mode = Some(text_mode);
        self.insertion_mode = InsertionMode::Text;
        self.queue_tokenizer_control(TokenizerControl::EnterTextMode(text_mode));
    }

    pub(in crate::html5::tree_builder) fn exit_text_mode(&mut self) {
        debug_assert!(self.original_insertion_mode.is_some());
        debug_assert!(self.active_text_mode.is_some());
        self.active_text_mode = None;
        self.insertion_mode = self
            .original_insertion_mode
            .take()
            .unwrap_or(InsertionMode::InBody);
        self.queue_tokenizer_control(TokenizerControl::ExitTextMode);
    }

    pub(in crate::html5::tree_builder) fn active_text_mode_end_tag_name(&self) -> Option<AtomId> {
        self.active_text_mode.map(|mode| mode.end_tag_name)
    }

    pub(in crate::html5::tree_builder) fn close_active_text_mode_element(&mut self) -> bool {
        let Some(active) = self.active_text_mode else {
            return false;
        };
        let current = self.open_elements.current();
        debug_assert_eq!(current.map(OpenElement::name), Some(active.end_tag_name));
        if current.map(OpenElement::name) != Some(active.end_tag_name) {
            return false;
        }
        let popped = self.open_elements.pop();
        if popped.is_some() {
            self.invalidate_text_coalescing();
            true
        } else {
            false
        }
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn is_text_mode_container_tag(&self, name: AtomId) -> bool {
        name == self.known_tags.script
            || name == self.known_tags.style
            || name == self.known_tags.title
            || name == self.known_tags.textarea
    }

    pub(in crate::html5::tree_builder) fn text_mode_spec_for_tag(
        &self,
        name: AtomId,
    ) -> Option<TextModeSpec> {
        if name == self.known_tags.style {
            Some(TextModeSpec::rawtext_style(name))
        } else if name == self.known_tags.title || name == self.known_tags.textarea {
            if name == self.known_tags.title {
                Some(TextModeSpec::rcdata_title(name))
            } else {
                Some(TextModeSpec::rcdata_textarea(name))
            }
        } else if name == self.known_tags.script {
            Some(TextModeSpec::script_data(name))
        } else {
            None
        }
    }
}
