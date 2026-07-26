use super::super::Html5Tokenizer;
use super::super::control::TextModeKind;
use crate::entities::{CharacterReferenceContext, decode_character_references};
use crate::html5::shared::{DocumentParseContext, Input, TextSpan, TextValue, Token};

impl Html5Tokenizer {
    pub(crate) fn emit_text_span(&mut self, start: usize, end: usize) {
        if start == end {
            return;
        }
        self.emit_token(Token::Text {
            text: TextValue::Span(TextSpan::new(start, end)),
        });
    }

    pub(crate) fn emit_text_owned(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.emit_token(Token::Text {
            text: TextValue::Owned(text.to_string()),
        });
    }

    fn emit_null_normalized_text(&mut self, text: &str, raw: &str) {
        if text.is_empty() {
            return;
        }
        self.emit_token(Token::Text {
            text: TextValue::NullNormalized {
                text: text.to_string(),
                had_null: true,
                had_non_whitespace_non_null: raw.chars().any(|character| {
                    character != '\0' && !matches!(character, '\t' | '\n' | '\x0C' | '\r' | ' ')
                }),
            },
        });
    }

    pub(crate) fn flush_pending_text(&mut self, input: &Input) -> bool {
        self.flush_pending_text_checked(input, None)
    }

    pub(crate) fn flush_pending_text_with_context(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> bool {
        self.flush_pending_text_checked(input, Some(ctx))
    }

    fn flush_pending_text_checked(
        &mut self,
        input: &Input,
        ctx: Option<&mut DocumentParseContext>,
    ) -> bool {
        match self.flush_pending_text_impl(input, ctx) {
            Ok(()) => true,
            Err(invariant) => {
                self.latch_invariant(invariant);
                false
            }
        }
    }

    fn flush_pending_text_impl(
        &mut self,
        input: &Input,
        mut ctx: Option<&mut DocumentParseContext>,
    ) -> Result<(), super::super::invariants::TokenizerInvariantKind> {
        let start = match self.pending_text_start.take() {
            Some(start) => start,
            None => return Ok(()),
        };
        let end = self.cursor;
        let text = input.as_str();
        if start > end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            return Err(super::super::invariants::TokenizerInvariantKind::PendingTextRangeInvalid);
        }
        if start == end {
            return Ok(());
        }
        let raw = &text[start..end];
        let null_normalized = if let Some(ctx) = &mut ctx {
            self.replace_nulls_for_token_text(input, ctx, raw, start)
        } else if raw.contains('\0') {
            Some(replace_nulls_without_reporting(raw))
        } else {
            None
        };
        let normalized = null_normalized.as_deref().unwrap_or(raw);
        let character_reference_context = self.character_reference_context_for_current_text();
        let should_decode =
            character_reference_context.is_some() && normalized.as_bytes().contains(&b'&');
        if !should_decode && null_normalized.is_none() {
            self.emit_text_span(start, end);
            return Ok(());
        }
        if !should_decode {
            if null_normalized.is_some() {
                self.emit_null_normalized_text(normalized, raw);
            } else {
                self.emit_text_owned(normalized);
            }
            return Ok(());
        }
        let decoded = decode_character_references(normalized, character_reference_context.unwrap());
        if let Some(ctx) = &mut ctx {
            self.record_character_reference_parse_errors(input, ctx, start, &decoded.diagnostics);
        }
        match decoded.text {
            std::borrow::Cow::Borrowed(_) if null_normalized.is_none() => {
                self.emit_text_span(start, end)
            }
            std::borrow::Cow::Borrowed(text) if null_normalized.is_some() => {
                self.emit_null_normalized_text(text, raw)
            }
            std::borrow::Cow::Owned(text) if null_normalized.is_some() => {
                self.emit_null_normalized_text(&text, raw)
            }
            std::borrow::Cow::Borrowed(text) => self.emit_text_owned(text),
            std::borrow::Cow::Owned(text) => self.emit_text_owned(&text),
        }
        Ok(())
    }

    pub(super) fn ensure_pending_text_start(&mut self) {
        if self.pending_text_start.is_none() {
            self.pending_text_start = Some(self.cursor);
        }
    }

    fn character_reference_context_for_current_text(&self) -> Option<CharacterReferenceContext> {
        match self.active_text_mode.map(|mode| mode.kind) {
            None => Some(CharacterReferenceContext::DataText),
            Some(TextModeKind::Rcdata) => Some(CharacterReferenceContext::RcdataText),
            Some(TextModeKind::RawText | TextModeKind::ScriptData) => None,
        }
    }
}

fn replace_nulls_without_reporting(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch == '\0' { '\u{FFFD}' } else { ch })
        .collect()
}
