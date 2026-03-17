use super::super::Html5Tokenizer;
use super::super::machine::Step;
use super::super::scan::is_tag_name_stop;
use super::super::states::TokenizerState;
use crate::html5::shared::{DocumentParseContext, Input};

impl Html5Tokenizer {
    pub(crate) fn step_tag_name(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::TagName);
        if self.tag_name_start.is_none() {
            // Invariant fallback: reset to Data instead of panicking on malformed state.
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }

        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        if !self.tag_name_complete {
            let consumed = self.consume_while(input, |ch| !is_tag_name_stop(ch));
            if consumed > 0 {
                self.tag_name_end = Some(self.cursor);
                if self.has_unconsumed_input(input)
                    && let Some(next) = self.peek(input)
                    && is_tag_name_stop(next)
                {
                    self.tag_name_complete = true;
                }
            }
            if !self.has_unconsumed_input(input) {
                return Step::NeedMoreInput;
            }
            if consumed == 0 {
                self.tag_name_complete = true;
            }
        }

        if self.current_tag_is_end {
            match self.peek(input) {
                Some('>') => {
                    let _ = self.consume_if(input, '>');
                    self.emit_current_tag(input, ctx);
                    self.transition_to(TokenizerState::Data);
                    Step::Progress
                }
                Some(_) => {
                    // End tags do not carry attributes in Core v0; skip until close.
                    let _ = self.consume(input);
                    Step::Progress
                }
                None => Step::NeedMoreInput,
            }
        } else {
            match self.peek(input) {
                Some(ch) if ch.is_ascii_whitespace() => {
                    let _ = self.consume_if(input, ch);
                    self.transition_to(TokenizerState::BeforeAttributeName);
                    Step::Progress
                }
                Some('/') => {
                    let _ = self.consume_if(input, '/');
                    self.transition_to(TokenizerState::SelfClosingStartTag);
                    Step::Progress
                }
                Some('>') => {
                    let _ = self.consume_if(input, '>');
                    self.emit_current_tag(input, ctx);
                    self.transition_to(TokenizerState::Data);
                    Step::Progress
                }
                Some(_) => {
                    // Recovery: consume unexpected bytes in tag context.
                    let _ = self.consume(input);
                    Step::Progress
                }
                None => Step::NeedMoreInput,
            }
        }
    }
}
