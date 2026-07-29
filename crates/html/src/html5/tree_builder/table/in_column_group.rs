use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::{is_html_whitespace_str, resolve_text_value};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_column_group(
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
                    crate::html5::shared::TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken),
                    Some("in-column-group-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } => {
                let resolved = resolve_text_value(token_text, text)?;
                if is_html_whitespace_str(&resolved) {
                    self.insert_resolved_text(&resolved, context)?;
                    Ok(DispatchOutcome::Done)
                } else {
                    if !self.close_column_group(context) {
                        return Ok(DispatchOutcome::Done);
                    }
                    Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
                }
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, context, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.col => {
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, true, context, atoms, text)?;
                if *self_closing {
                    context.acknowledge_self_closing_flag()?;
                }
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.colgroup => {
                if !self.close_column_group(context) {
                    self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-column-group-colgroup-end-tag-ignored"));
                }
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.col => {
                self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-column-group-col-end-tag-ignored"));
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                if self.current_node_name() != Some(self.known_tags.colgroup) {
                    let _ = self.ensure_document_created(context)?;
                    return Ok(DispatchOutcome::Done);
                }
                let _ = self.close_column_group(context);
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            _ => {
                if !self.close_column_group(context) {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
        }
    }
}
