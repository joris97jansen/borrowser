use super::super::Html5Tokenizer;
use super::super::limits::LIMIT_DETAIL_TAG_NAME;
use crate::html5::shared::{
    DocumentParseContext, Input, ParserResourceLimit, Token, WhatwgParseErrorCode,
};

impl Html5Tokenizer {
    pub(super) fn emit_current_tag(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        tag_end_position: usize,
    ) {
        if !self.ensure_current_tag_solidus_invariant(input) {
            self.abandon_pending_tag();
            return;
        }
        let trailing_solidus_position = if self.current_tag_self_closing {
            let Some(position) = self.current_tag_self_closing_solidus_position else {
                self.latch_invariant(
                    super::super::invariants::TokenizerInvariantKind::
                        SelfClosingFlagMissingSolidusPosition,
                );
                self.abandon_pending_tag();
                return;
            };
            Some(position)
        } else {
            None
        };
        let (name_start, end) = match (self.tag_name_start.take(), self.tag_name_end.take()) {
            (Some(start), Some(end)) => (start, end),
            _ => {
                self.abandon_pending_tag();
                return;
            }
        };
        if name_start > end || end > input.as_str().len() {
            self.abandon_pending_tag();
            return;
        }
        let raw = &input.as_str()[name_start..end];
        let (raw, truncated) = self.truncate_str_to_bytes(raw, self.max_tag_name_bytes());
        if truncated {
            self.record_limit_error(
                input,
                ctx,
                name_start,
                ParserResourceLimit::TagNameBytes,
                LIMIT_DETAIL_TAG_NAME,
                self.max_tag_name_bytes(),
            );
        }
        // Canonicalization policy: HTML tag names are interned with ASCII
        // folding (`A-Z` -> `a-z`) and preserve non-ASCII bytes.
        let normalized = self.replace_nulls_for_token_text(input, ctx, raw, name_start);
        let atom_text = normalized.as_deref().unwrap_or(raw);
        let name = self.intern_atom_or_invariant(ctx, atom_text, "tag name");
        if self.current_tag_is_end {
            if !self.current_tag_attrs.is_empty() {
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::EndTagWithAttributes,
                    tag_end_position,
                    crate::html5::shared::ParserRecoveryAction::DropEndTagAttributes,
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::ERROR_DETAIL_END_TAG_WITH_ATTRIBUTES,
                        None,
                    ),
                );
            }
            if let Some(position) = trailing_solidus_position {
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                    position,
                    crate::html5::shared::ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                    super::super::normalization::legacy_diagnostic(
                        super::super::normalization::ERROR_DETAIL_END_TAG_WITH_TRAILING_SOLIDUS,
                        None,
                    ),
                );
            }
            self.emit_token(Token::EndTag { name });
        } else {
            let attrs = std::mem::take(&mut self.current_tag_attrs);
            let self_closing = self.current_tag_self_closing;
            self.emit_token(Token::StartTag {
                name,
                attrs,
                self_closing,
            });
        }
        self.abandon_pending_tag();
    }
}
