use super::Html5Tokenizer;
use super::control::TextModeKind;
use super::machine::Step;
use super::scan::{IncrementalEndTagMatch, IncrementalEndTagMatcher};
use super::states::TokenizerState;
use crate::entities::decode_entities;
use crate::html5::shared::{Input, TextSpan, TextValue, Token};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextModeEndTagMatch {
    Matched {
        cursor_after: usize,
        name: crate::html5::shared::AtomId,
    },
    NeedMoreInput(IncrementalEndTagMatcher),
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
        self.step_text_mode_with_matching_end_tag(input, TextModeKind::RawText)
    }

    pub(crate) fn step_rcdata(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::Rcdata);
        self.step_text_mode_with_matching_end_tag(input, TextModeKind::Rcdata)
    }

    pub(crate) fn step_script_data(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptData);
        // Core-v0 script text-mode subset intentionally implements the bounded
        // "raw until matching </script>" behavior. Escaped/double-escaped
        // script-data state-family work remains tracked separately.
        self.step_text_mode_with_matching_end_tag(input, TextModeKind::ScriptData)
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

    fn step_text_mode_with_matching_end_tag(
        &mut self,
        input: &Input,
        expected_kind: TextModeKind,
    ) -> Step {
        if let Some(pending_end_tag) = self.pending_text_mode_end_tag.take() {
            self.cursor = pending_end_tag.cursor_after;
            self.emit_token(Token::EndTag {
                name: pending_end_tag.name,
            });
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        if let Some(matcher) = self.pending_text_mode_end_tag_matcher.take() {
            debug_assert_eq!(matcher.start(), self.cursor);
            return self.resolve_text_mode_end_tag_attempt(
                input,
                expected_kind,
                matcher.start(),
                Some(matcher),
            );
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
                "text-mode state must make progress if input remains"
            );
            if !self.has_unconsumed_input(input) {
                return Step::NeedMoreInput;
            }
            debug_assert_eq!(self.peek(input), Some('<'));
        }

        let less_than_pos = self.cursor;
        self.resolve_text_mode_end_tag_attempt(input, expected_kind, less_than_pos, None)
    }

    fn resolve_text_mode_end_tag_attempt(
        &mut self,
        input: &Input,
        expected_kind: TextModeKind,
        less_than_pos: usize,
        matcher: Option<IncrementalEndTagMatcher>,
    ) -> Step {
        match self.match_text_mode_end_tag(input, expected_kind, matcher) {
            TextModeEndTagMatch::Matched { cursor_after, name } => {
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
            TextModeEndTagMatch::NeedMoreInput(matcher) => {
                self.pending_text_mode_end_tag_matcher = Some(matcher);
                self.cursor = less_than_pos;
                Step::NeedMoreInput
            }
            TextModeEndTagMatch::NoMatch => {
                if self.pending_text_start.is_none() {
                    self.pending_text_start = Some(self.cursor);
                }
                let _ = self.consume_if(input, '<');
                Step::Progress
            }
        }
    }

    fn match_text_mode_end_tag(
        &self,
        input: &Input,
        expected_kind: TextModeKind,
        matcher: Option<IncrementalEndTagMatcher>,
    ) -> TextModeEndTagMatch {
        let Some(active_text_mode) = self.active_text_mode else {
            return TextModeEndTagMatch::NoMatch;
        };
        if active_text_mode.kind != expected_kind {
            return TextModeEndTagMatch::NoMatch;
        }
        let tag_name = active_text_mode.text_mode_end_tag_literal();
        let matcher = matcher.unwrap_or_else(|| IncrementalEndTagMatcher::new(self.cursor));
        match matcher.advance(input.as_str().as_bytes(), tag_name) {
            IncrementalEndTagMatch::Matched { cursor_after } => TextModeEndTagMatch::Matched {
                cursor_after,
                name: active_text_mode.end_tag_name,
            },
            IncrementalEndTagMatch::NeedMoreInput(matcher) => {
                TextModeEndTagMatch::NeedMoreInput(matcher)
            }
            IncrementalEndTagMatch::NoMatch => TextModeEndTagMatch::NoMatch,
        }
    }
}
