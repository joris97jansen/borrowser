use super::super::Html5Tokenizer;
use super::super::control::TextModeKind;
use crate::entities::decode_entities;
use crate::html5::shared::{Input, TextSpan, TextValue, Token};

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

    pub(crate) fn flush_pending_text(&mut self, input: &Input) {
        let start = match self.pending_text_start.take() {
            Some(start) => start,
            None => return,
        };
        let end = self.cursor;
        let text = input.as_str();
        if !(start <= end
            && end <= text.len()
            && start != end
            && text.is_char_boundary(start)
            && text.is_char_boundary(end))
        {
            return;
        }
        let raw = &text[start..end];
        if !self.should_decode_character_references_in_current_text()
            || !raw.as_bytes().contains(&b'&')
        {
            self.emit_text_span(start, end);
            return;
        }
        let decoded = decode_entities(raw);
        match decoded {
            std::borrow::Cow::Borrowed(_) => self.emit_text_span(start, end),
            std::borrow::Cow::Owned(text) => self.emit_text_owned(&text),
        }
    }

    pub(super) fn ensure_pending_text_start(&mut self) {
        if self.pending_text_start.is_none() {
            self.pending_text_start = Some(self.cursor);
        }
    }

    fn should_decode_character_references_in_current_text(&self) -> bool {
        !matches!(
            self.active_text_mode.map(|mode| mode.kind),
            Some(TextModeKind::RawText | TextModeKind::ScriptData)
        )
    }
}
