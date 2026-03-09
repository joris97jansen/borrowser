use super::Html5Tokenizer;
use super::control::TextModeKind;
use super::input::MatchResult;
use super::machine::Step;
use super::scan::is_html_space_byte;
use super::states::TokenizerState;
use crate::entities::decode_entities;
use crate::html5::shared::{Input, TextSpan, TextValue, Token};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RawTextEndTagMatch {
    Matched {
        cursor_after: usize,
        name: crate::html5::shared::AtomId,
    },
    NeedMoreInput,
    NoMatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingTextModeEndTag {
    pub(crate) cursor_after: usize,
    pub(crate) name: crate::html5::shared::AtomId,
}

impl Html5Tokenizer {
    pub(crate) fn step_raw_text(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::RawText);
        if let Some(pending_end_tag) = self.pending_text_mode_end_tag.take() {
            self.cursor = pending_end_tag.cursor_after;
            self.emit_token(Token::EndTag {
                name: pending_end_tag.name,
            });
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) != Some('<') {
            if self.pending_text_start.is_none() {
                self.pending_text_start = Some(self.cursor);
            }
            let consumed = self.consume_while(input, |ch| ch != '<');
            assert!(
                consumed > 0,
                "rawtext state must make progress if input remains"
            );
            if !self.has_unconsumed_input(input) {
                return Step::NeedMoreInput;
            }
            debug_assert_eq!(self.peek(input), Some('<'));
        }

        let less_than_pos = self.cursor;
        match self.match_rawtext_end_tag(input) {
            RawTextEndTagMatch::Matched { cursor_after, name } => {
                if self
                    .pending_text_start
                    .is_some_and(|text_start| text_start < less_than_pos)
                {
                    self.pending_text_mode_end_tag =
                        Some(PendingTextModeEndTag { cursor_after, name });
                    self.flush_pending_text(input);
                    Step::Progress
                } else {
                    self.cursor = cursor_after;
                    self.emit_token(Token::EndTag { name });
                    self.transition_to(TokenizerState::Data);
                    Step::Progress
                }
            }
            RawTextEndTagMatch::NeedMoreInput => {
                self.cursor = less_than_pos;
                Step::NeedMoreInput
            }
            RawTextEndTagMatch::NoMatch => {
                if self.pending_text_start.is_none() {
                    self.pending_text_start = Some(self.cursor);
                }
                let _ = self.consume_if(input, '<');
                Step::Progress
            }
        }
    }

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

    fn should_decode_character_references_in_current_text(&self) -> bool {
        !matches!(
            self.active_text_mode.map(|mode| mode.kind),
            Some(TextModeKind::RawText | TextModeKind::ScriptData)
        )
    }

    fn match_rawtext_end_tag(&self, input: &Input) -> RawTextEndTagMatch {
        let Some(active_text_mode) = self.active_text_mode else {
            return RawTextEndTagMatch::NoMatch;
        };
        if active_text_mode.kind != TextModeKind::RawText {
            return RawTextEndTagMatch::NoMatch;
        }
        let Some(hint) = active_text_mode.rawtext_end_tag_literal() else {
            return RawTextEndTagMatch::NoMatch;
        };

        let text = input.as_str();
        let bytes = text.as_bytes();
        let cursor = self.cursor;
        if bytes.get(cursor) != Some(&b'<') {
            return RawTextEndTagMatch::NoMatch;
        }
        let slash = cursor + 1;
        if slash >= bytes.len() {
            return RawTextEndTagMatch::NeedMoreInput;
        }
        if bytes[slash] != b'/' {
            return RawTextEndTagMatch::NoMatch;
        }
        let name_start = slash + 1;
        match super::scan::match_ascii_prefix_ci_at(bytes, name_start, hint) {
            MatchResult::Matched => {}
            MatchResult::NeedMoreInput => return RawTextEndTagMatch::NeedMoreInput,
            MatchResult::NoMatch => return RawTextEndTagMatch::NoMatch,
        }

        let mut cursor_after_name = name_start + hint.len();
        if cursor_after_name >= bytes.len() {
            return RawTextEndTagMatch::NeedMoreInput;
        }
        while cursor_after_name < bytes.len() && is_html_space_byte(bytes[cursor_after_name]) {
            cursor_after_name += 1;
        }
        if cursor_after_name >= bytes.len() {
            return RawTextEndTagMatch::NeedMoreInput;
        }
        if bytes[cursor_after_name] != b'>' {
            return RawTextEndTagMatch::NoMatch;
        }
        RawTextEndTagMatch::Matched {
            cursor_after: cursor_after_name + 1,
            name: active_text_mode.end_tag_name,
        }
    }
}
