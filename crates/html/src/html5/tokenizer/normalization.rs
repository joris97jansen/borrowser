use super::Html5Tokenizer;
use crate::html5::shared::{DocumentParseContext, ErrorOrigin, ParseError, ParseErrorCode};

pub(super) const ERROR_DETAIL_UNEXPECTED_NULL_CHARACTER: &str = "unexpected-null-character";
pub(super) const ERROR_DETAIL_EOF_IN_COMMENT: &str = "eof-in-comment";
pub(super) const ERROR_DETAIL_EOF_IN_DOCTYPE: &str = "eof-in-doctype";
pub(super) const ERROR_DETAIL_EOF_IN_TAG_OPEN: &str = "eof-in-tag-open";
pub(super) const ERROR_DETAIL_EOF_IN_TEXT_MODE: &str = "eof-in-text-mode";

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
