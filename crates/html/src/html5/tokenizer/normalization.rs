use super::Html5Tokenizer;
use crate::entities::CharacterReferenceDiagnostic;
use crate::html5::shared::{DocumentParseContext, ErrorOrigin, ParseError, ParseErrorCode};

pub(super) const ERROR_DETAIL_UNEXPECTED_NULL_CHARACTER: &str = "unexpected-null-character";
pub(super) const ERROR_DETAIL_EOF_IN_COMMENT: &str = "eof-in-comment";
pub(super) const ERROR_DETAIL_EOF_IN_DOCTYPE: &str = "eof-in-doctype";
pub(super) const ERROR_DETAIL_EOF_IN_END_TAG_OPEN: &str = "eof-in-end-tag-open";
pub(super) const ERROR_DETAIL_EOF_IN_MARKUP_DECLARATION: &str = "eof-in-markup-declaration";
pub(super) const ERROR_DETAIL_EOF_IN_SELF_CLOSING_START_TAG: &str = "eof-in-self-closing-start-tag";
pub(super) const ERROR_DETAIL_EOF_IN_TAG_NAME: &str = "eof-in-tag-name";
pub(super) const ERROR_DETAIL_EOF_IN_TAG_OPEN: &str = "eof-in-tag-open";
pub(super) const ERROR_DETAIL_EOF_IN_ATTRIBUTE: &str = "eof-in-attribute";
pub(super) const ERROR_DETAIL_EOF_IN_TEXT_MODE: &str = "eof-in-text-mode";
pub(super) const ERROR_DETAIL_EOF_IN_CDATA: &str = "eof-in-cdata";
pub(super) const ERROR_DETAIL_INVALID_ATTRIBUTE_NAME: &str = "invalid-attribute-name";
pub(super) const ERROR_DETAIL_INVALID_ATTRIBUTE_VALUE: &str = "invalid-attribute-value";
pub(super) const ERROR_DETAIL_DUPLICATE_ATTRIBUTE: &str = "duplicate-attribute";
pub(super) const ERROR_DETAIL_INVALID_END_TAG_OPEN: &str = "invalid-end-tag-open";
pub(super) const ERROR_DETAIL_INVALID_END_TAG_TRAILING_CONTENT: &str =
    "invalid-end-tag-trailing-content";
pub(super) const ERROR_DETAIL_INVALID_MARKUP_DECLARATION: &str = "invalid-markup-declaration";
pub(super) const ERROR_DETAIL_INVALID_SELF_CLOSING_START_TAG: &str =
    "invalid-self-closing-start-tag";
pub(super) const ERROR_DETAIL_INVALID_TAG_OPEN: &str = "invalid-tag-open";
pub(super) const ERROR_DETAIL_MALFORMED_COMMENT: &str = "malformed-comment";
pub(super) const ERROR_DETAIL_MALFORMED_DOCTYPE: &str = "malformed-doctype";
pub(super) const ERROR_DETAIL_MISSING_WHITESPACE_BETWEEN_ATTRIBUTES: &str =
    "missing-whitespace-between-attributes";

impl Html5Tokenizer {
    pub(in crate::html5::tokenizer) fn record_tokenizer_parse_error(
        &self,
        ctx: &mut DocumentParseContext,
        code: ParseErrorCode,
        position: usize,
        detail: &'static str,
        aux: Option<u32>,
    ) {
        ctx.record_error(ParseError {
            origin: ErrorOrigin::Tokenizer,
            code,
            position,
            detail: Some(detail),
            aux,
        });
    }

    pub(in crate::html5::tokenizer) fn record_character_reference_parse_errors(
        &self,
        ctx: &mut DocumentParseContext,
        base_position: usize,
        diagnostics: &[CharacterReferenceDiagnostic],
    ) {
        for diagnostic in diagnostics {
            self.record_tokenizer_parse_error(
                ctx,
                ParseErrorCode::InvalidCharacterReference,
                base_position + diagnostic.offset,
                diagnostic.kind.detail(),
                diagnostic.aux,
            );
        }
    }

    pub(in crate::html5::tokenizer) fn replace_nulls_for_token_text(
        &self,
        ctx: &mut DocumentParseContext,
        raw: &str,
        base_position: usize,
    ) -> Option<String> {
        let mut normalized = None;
        for (offset, ch) in raw.char_indices() {
            if ch == '\0' {
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::UnexpectedNullCharacter,
                    base_position + offset,
                    ERROR_DETAIL_UNEXPECTED_NULL_CHARACTER,
                    Some(0),
                );
                normalized
                    .get_or_insert_with(|| {
                        let mut prefix = String::with_capacity(raw.len());
                        prefix.push_str(&raw[..offset]);
                        prefix
                    })
                    .push('\u{FFFD}');
            } else if let Some(out) = normalized.as_mut() {
                out.push(ch);
            }
        }
        normalized
    }
}
