use super::DispatchOutcome;
use crate::html5::shared::{
    AtomTable, ParserRecoveryAction, Token, TreeConstructionImplementationDiagnosticCode,
    TreeConstructionParseErrorCode,
};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::is_html_whitespace_text;
use crate::html5::tree_builder::stack::ScopeKind;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_initial(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                self.handle_doctype(
                    name,
                    public_id.as_deref(),
                    system_id.as_deref(),
                    *force_quirks,
                    context,
                    atoms,
                )?;
                self.insertion_mode = InsertionMode::BeforeHtml;
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_initial_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                self.insert_initial_processing_instruction(processing_instruction, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } => {
                if is_html_whitespace_text(token_text, text)? {
                    Ok(DispatchOutcome::Done)
                } else {
                    self.record_tree_parse_error(
                        context,
                        TreeConstructionParseErrorCode::ExpectedDoctypeBeforeNonSpaceToken,
                        Some(ParserRecoveryAction::ReprocessToken),
                        Some("initial-unexpected-token"),
                    );
                    Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHtml))
                }
            }
            Token::Eof => {
                let _ = self.ensure_document_created(context)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHtml))
            }
            _ => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::ExpectedDoctypeBeforeNonSpaceToken,
                    Some(ParserRecoveryAction::ReprocessToken),
                    Some("initial-unexpected-token"),
                );
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHtml))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_before_html(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("before-html-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                let document = self.ensure_document_created(context)?;
                self.insert_processing_instruction(
                    processing_instruction,
                    context,
                    text,
                    Some(document),
                )?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.html, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHead))
            }
            _ => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.html, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHead))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_before_head(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("before-head-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                self.insert_processing_instruction(processing_instruction, context, text, None)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.head => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let inserted = self.insert_element(*name, attrs, false, context, atoms, text)?;
                self.head_element_pointer = inserted;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let inserted =
                    self.insert_element(self.known_tags.head, &[], false, context, atoms, text)?;
                self.head_element_pointer = inserted;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InHead))
            }
            _ => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let inserted =
                    self.insert_element(self.known_tags.head, &[], false, context, atoms, text)?;
                self.head_element_pointer = inserted;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InHead))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_in_head(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                self.insert_processing_instruction(processing_instruction, context, text, None)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("in-head-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                self.insert_text(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { .. } => {
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::HtmlStartTagAfterHtmlElement,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("in-head-unexpected-html-start-tag"),
                );
                if !attrs.is_empty() {
                    self.record_tree_implementation_diagnostic(
                        context,
                        TreeConstructionImplementationDiagnosticCode::HtmlElementAttributesNotMerged,
                        Some("html-start-tag-attributes-ignored"),
                    );
                }
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.style
                || *name == self.known_tags.title
                || *name == self.known_tags.script =>
            {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let inserted =
                    self.insert_element(*name, attrs, *self_closing, context, atoms, text)?;
                if !self_closing && inserted.is_some() {
                    self.enter_text_mode_for_element(*name);
                }
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.base
                || *name == self.known_tags.link
                || *name == self.known_tags.meta =>
            {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, *self_closing, context, atoms, text)?;
                if *self_closing {
                    context.acknowledge_self_closing_flag()?;
                }
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.head => {
                let _ = self.close_element_in_scope(*name, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.html => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode,
                    Some(ParserRecoveryAction::ReprocessToken),
                    Some("in-head-unexpected-html-end-tag"),
                );
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
            Token::Eof => {
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
            _ => {
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_after_head(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                self.insert_processing_instruction(processing_instruction, context, text, None)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.body => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::HtmlStartTagAfterHtmlElement,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-head-unexpected-html-start-tag"),
                );
                if !attrs.is_empty() {
                    self.record_tree_implementation_diagnostic(
                        context,
                        TreeConstructionImplementationDiagnosticCode::HtmlElementAttributesNotMerged,
                        Some("html-start-tag-attributes-ignored"),
                    );
                }
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.head => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::StartTagForbiddenByActiveInsertionMode,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-head-unexpected-head-start-tag"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.base
                    || *name == self.known_tags.link
                    || *name == self.known_tags.meta
                    || *name == self.known_tags.script
                    || *name == self.known_tags.style
                    || *name == self.known_tags.title =>
            {
                let outcome = self
                    .with_temporary_head_element(context, |this, context| {
                        this.handle_in_head(token, atoms, context, text)
                    })?
                    .unwrap_or(DispatchOutcome::Done);
                Ok(outcome)
            }
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-head-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.head => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-head-unexpected-head-end-tag"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name }
                if *name == self.known_tags.body || *name == self.known_tags.html =>
            {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.body, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
            Token::EndTag { name: _ } => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-head-unexpected-end-tag"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.body, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
            _ => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.body, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_after_body(
        &mut self,
        token: &Token,
        _atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                self.insert_text(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                let html = self
                    .open_elements
                    .get(0)
                    .ok_or(crate::html5::shared::EngineInvariantError)?
                    .key();
                self.insert_processing_instruction(
                    processing_instruction,
                    context,
                    text,
                    Some(html),
                )?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-body-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::HtmlStartTagAfterHtmlElement,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-body-unexpected-html-start-tag"),
                );
                if !attrs.is_empty() {
                    self.record_tree_implementation_diagnostic(
                        context,
                        TreeConstructionImplementationDiagnosticCode::HtmlElementAttributesNotMerged,
                        Some("html-start-tag-attributes-ignored"),
                    );
                }
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.html => {
                self.insertion_mode = InsertionMode::AfterAfterBody;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => Ok(DispatchOutcome::Done),
            _ => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::TokenForbiddenAfterBody,
                    Some(ParserRecoveryAction::ReprocessToken),
                    Some("after-body-unexpected-token"),
                );
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_after_after_body(
        &mut self,
        token: &Token,
        _atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_document_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                let document = self.ensure_document_created(context)?;
                self.insert_processing_instruction(
                    processing_instruction,
                    context,
                    text,
                    Some(document),
                )?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                self.insert_text(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-after-body-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::HtmlStartTagAfterHtmlElement,
                    Some(ParserRecoveryAction::IgnoreToken),
                    Some("after-after-body-unexpected-html-start-tag"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => Ok(DispatchOutcome::Done),
            _ => {
                self.record_tree_parse_error(
                    context,
                    TreeConstructionParseErrorCode::TokenForbiddenAfterAfterBody,
                    Some(ParserRecoveryAction::ReprocessToken),
                    Some("after-after-body-unexpected-token"),
                );
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
        }
    }
}
