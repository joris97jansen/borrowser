use super::super::Html5Tokenizer;
use super::super::control::TextModeKind;
use super::super::input::MatchResult;
use super::super::machine::Step;
use super::super::scan::{ScriptTagBoundaryMatch, match_script_tag_boundary_at};
use super::super::states::TokenizerState;
use super::{PendingTextModeEndTag, ScriptFamilyState, TextModeEndTagMatch};
use crate::html5::shared::{DocumentParseContext, Input, Token};

impl Html5Tokenizer {
    pub(crate) fn step_script_data(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptData);
        self.step_script_data_core(input, ctx, ScriptFamilyState::ScriptData)
    }

    pub(crate) fn step_script_data_escaped(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptDataEscaped);
        self.step_script_data_core(input, ctx, ScriptFamilyState::Escaped)
    }

    pub(crate) fn step_script_data_escaped_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptDataEscapedDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::ScriptDataEscapedDashDash);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::ScriptDataEscaped);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_script_data_escaped_dash_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptDataEscapedDashDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                Step::Progress
            }
            Some('>') => {
                self.ensure_pending_text_start();
                let _ = self.consume_if(input, '>');
                self.transition_to(TokenizerState::ScriptData);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::ScriptDataEscaped);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_script_data_double_escaped(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptDataDoubleEscaped);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('<') => self.handle_script_double_escaped_less_than(input),
            Some('-') => {
                self.ensure_pending_text_start();
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::ScriptDataDoubleEscapedDash);
                Step::Progress
            }
            Some(_) => {
                self.ensure_pending_text_start();
                let consumed = self.consume_while(input, |ch| ch != '<' && ch != '-');
                assert!(
                    consumed > 0,
                    "double-escaped script-data state must make progress if input remains"
                );
                if self.has_unconsumed_input(input) {
                    Step::Progress
                } else {
                    Step::NeedMoreInput
                }
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_script_data_double_escaped_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptDataDoubleEscapedDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::ScriptDataDoubleEscapedDashDash);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::ScriptDataDoubleEscaped);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_script_data_double_escaped_dash_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ScriptDataDoubleEscapedDashDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                Step::Progress
            }
            Some('>') => {
                self.ensure_pending_text_start();
                let _ = self.consume_if(input, '>');
                self.transition_to(TokenizerState::ScriptDataDoubleEscaped);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::ScriptDataDoubleEscaped);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_script_data_core(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        family_state: ScriptFamilyState,
    ) -> Step {
        if let Some(pending_end_tag) = self.pending_text_mode_end_tag.take() {
            self.set_cursor(pending_end_tag.cursor_after);
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
                ctx,
                TextModeKind::ScriptData,
                matcher.start(),
                Some(matcher),
            );
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('<') => match family_state {
                ScriptFamilyState::ScriptData => self.handle_script_data_less_than(input, ctx),
                ScriptFamilyState::Escaped => self.handle_script_escaped_less_than(input, ctx),
            },
            Some('-') if family_state == ScriptFamilyState::Escaped => {
                self.ensure_pending_text_start();
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::ScriptDataEscapedDash);
                Step::Progress
            }
            Some(_) => {
                self.ensure_pending_text_start();
                let consumed = match family_state {
                    ScriptFamilyState::ScriptData => self.consume_while(input, |ch| ch != '<'),
                    ScriptFamilyState::Escaped => {
                        self.consume_while(input, |ch| ch != '<' && ch != '-')
                    }
                };
                assert!(
                    consumed > 0,
                    "script-data family state must make progress if input remains"
                );
                if self.has_unconsumed_input(input) {
                    Step::Progress
                } else {
                    Step::NeedMoreInput
                }
            }
            None => Step::NeedMoreInput,
        }
    }

    fn handle_script_data_less_than(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        let less_than_pos = self.cursor;
        match self.match_text_mode_end_tag(input, TextModeKind::ScriptData, None) {
            TextModeEndTagMatch::Matched {
                cursor_after,
                name,
                had_attributes,
                self_closing,
            } => {
                self.record_text_mode_end_tag_parse_errors(
                    ctx,
                    less_than_pos,
                    had_attributes,
                    self_closing,
                );
                if self
                    .pending_text_start
                    .is_some_and(|text_start| text_start < less_than_pos)
                {
                    self.pending_text_mode_end_tag =
                        Some(PendingTextModeEndTag { cursor_after, name });
                    self.flush_pending_text(input);
                    Step::Progress
                } else {
                    self.set_cursor(cursor_after);
                    self.emit_token(Token::EndTag { name });
                    self.transition_to(TokenizerState::Data);
                    Step::Progress
                }
            }
            TextModeEndTagMatch::NeedMoreInput(matcher) => {
                match self.match_ascii_prefix(input, b"<!--") {
                    MatchResult::NeedMoreInput => Step::NeedMoreInput,
                    _ => {
                        self.pending_text_mode_end_tag_matcher = Some(matcher);
                        self.set_cursor(less_than_pos);
                        Step::NeedMoreInput
                    }
                }
            }
            TextModeEndTagMatch::LimitExceeded => {
                self.recover_from_text_mode_end_tag_limit(ctx, input, less_than_pos)
            }
            TextModeEndTagMatch::NoMatch => match self.match_ascii_prefix(input, b"<!--") {
                MatchResult::Matched => {
                    self.ensure_pending_text_start();
                    let _ = self.consume_ascii_sequence(input, b"<!--");
                    self.transition_to(TokenizerState::ScriptDataEscaped);
                    Step::Progress
                }
                MatchResult::NeedMoreInput => Step::NeedMoreInput,
                MatchResult::NoMatch => {
                    self.ensure_pending_text_start();
                    let _ = self.consume_if(input, '<');
                    Step::Progress
                }
            },
        }
    }

    fn handle_script_escaped_less_than(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        let less_than_pos = self.cursor;
        match self.match_text_mode_end_tag(input, TextModeKind::ScriptData, None) {
            TextModeEndTagMatch::Matched {
                cursor_after,
                name,
                had_attributes,
                self_closing,
            } => {
                self.record_text_mode_end_tag_parse_errors(
                    ctx,
                    less_than_pos,
                    had_attributes,
                    self_closing,
                );
                if self
                    .pending_text_start
                    .is_some_and(|text_start| text_start < less_than_pos)
                {
                    self.pending_text_mode_end_tag =
                        Some(PendingTextModeEndTag { cursor_after, name });
                    self.flush_pending_text(input);
                    Step::Progress
                } else {
                    self.set_cursor(cursor_after);
                    self.emit_token(Token::EndTag { name });
                    self.transition_to(TokenizerState::Data);
                    Step::Progress
                }
            }
            TextModeEndTagMatch::NeedMoreInput(matcher) => {
                let bytes = input.as_str().as_bytes();
                match match_script_tag_boundary_at(bytes, less_than_pos, false) {
                    ScriptTagBoundaryMatch::NeedMoreInput => Step::NeedMoreInput,
                    _ => {
                        self.pending_text_mode_end_tag_matcher = Some(matcher);
                        self.set_cursor(less_than_pos);
                        Step::NeedMoreInput
                    }
                }
            }
            TextModeEndTagMatch::LimitExceeded => {
                self.recover_from_text_mode_end_tag_limit(ctx, input, less_than_pos)
            }
            TextModeEndTagMatch::NoMatch => {
                let bytes = input.as_str().as_bytes();
                match match_script_tag_boundary_at(bytes, less_than_pos, false) {
                    ScriptTagBoundaryMatch::Matched { cursor_after } => {
                        self.ensure_pending_text_start();
                        self.set_cursor(cursor_after);
                        self.transition_to(TokenizerState::ScriptDataDoubleEscaped);
                        Step::Progress
                    }
                    ScriptTagBoundaryMatch::NeedMoreInput => Step::NeedMoreInput,
                    ScriptTagBoundaryMatch::NoMatch => {
                        self.ensure_pending_text_start();
                        let _ = self.consume_if(input, '<');
                        Step::Progress
                    }
                }
            }
        }
    }

    fn handle_script_double_escaped_less_than(&mut self, input: &Input) -> Step {
        let less_than_pos = self.cursor;
        let bytes = input.as_str().as_bytes();
        match match_script_tag_boundary_at(bytes, less_than_pos, true) {
            ScriptTagBoundaryMatch::Matched { cursor_after } => {
                self.ensure_pending_text_start();
                self.set_cursor(cursor_after);
                self.transition_to(TokenizerState::ScriptDataEscaped);
                Step::Progress
            }
            ScriptTagBoundaryMatch::NeedMoreInput => Step::NeedMoreInput,
            ScriptTagBoundaryMatch::NoMatch => {
                self.ensure_pending_text_start();
                let _ = self.consume_if(input, '<');
                Step::Progress
            }
        }
    }
}
