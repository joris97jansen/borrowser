use super::super::Html5Tokenizer;
use super::super::input::MatchResult;
use super::super::machine::Step;
use super::super::states::TokenizerState;
use crate::html5::shared::Input;

impl Html5Tokenizer {
    pub(crate) fn step_tag_open(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::TagOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        #[cfg(any(test, feature = "debug-stats"))]
        {
            let tail: String = input.as_str()[self.cursor..].chars().take(8).collect();
            log::trace!(
                target: "html5.tokenizer",
                "step_tag_open cursor={} head={:?} next={:?} tail={:?}",
                self.cursor,
                self.peek(input),
                self.peek_next_char(input),
                tail
            );
        }
        if self.peek(input) != Some('<') {
            // Recovery: if state got desynchronized, continue in Data.
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }

        // Prefix-first ASCII dispatch keeps chunk-boundary behavior deterministic
        // for spec keywords that begin with `<`.
        match self.match_ascii_prefix(input, b"</") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence(input, b"</");
                debug_assert!(did_consume, "matched prefix must be consumable");
                self.end_tag_prefix_consumed = true;
                self.clear_current_attribute();
                self.transition_to(TokenizerState::EndTagOpen);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.match_ascii_prefix(input, b"<!") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence(input, b"<!");
                debug_assert!(did_consume, "matched prefix must be consumable");
                self.transition_to(TokenizerState::MarkupDeclarationOpen);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.peek_next_char(input) {
            None => Step::NeedMoreInput,
            Some(ch) if ch.is_ascii_alphabetic() => {
                if !self.consume_if(input, '<') {
                    return Step::NeedMoreInput;
                }
                self.tag_name_start = Some(self.cursor);
                self.tag_name_end = None;
                self.tag_name_complete = false;
                self.current_tag_is_end = false;
                self.current_tag_self_closing = false;
                self.current_tag_attrs.clear();
                self.clear_current_attribute();
                self.transition_to(TokenizerState::TagName);
                Step::Progress
            }
            Some(_) => {
                // Recovery: not a valid tag opener for Core v0, emit `<` as text.
                if !self.consume_if(input, '<') {
                    return Step::NeedMoreInput;
                }
                self.emit_text_owned("<");
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
        }
    }

    pub(crate) fn step_end_tag_open(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::EndTagOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_alphabetic() => {
                self.tag_name_start = Some(self.cursor);
                self.tag_name_end = None;
                self.tag_name_complete = false;
                self.current_tag_is_end = true;
                self.current_tag_self_closing = false;
                self.current_tag_attrs.clear();
                self.clear_current_attribute();
                self.end_tag_prefix_consumed = false;
                self.transition_to(TokenizerState::TagName);
                Step::Progress
            }
            Some('>') => {
                // Recovery for `</>` style malformed end tags.
                let _ = self.consume_if(input, '>');
                self.end_tag_prefix_consumed = false;
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                // Recovery per Core v0: emit consumed `</` as owned text and
                // reprocess the current non-alpha byte in Data (we do not consume
                // it here, so Data observes it on the next step).
                if self.end_tag_prefix_consumed {
                    self.emit_text_owned("</");
                } else {
                    self.emit_text_owned("<");
                }
                self.end_tag_prefix_consumed = false;
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }
}
