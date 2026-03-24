use super::input::MatchResult;
use super::states::TokenizerState;
use super::{Html5Tokenizer, TokenizeResult};
use crate::html5::shared::{AtomError, AtomId, DocumentParseContext, Input};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Progress,
    NeedMoreInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StopCondition {
    DrainAvailableInput,
    YieldAfterToken,
}

pub(crate) const MAX_STEPS_PER_PUMP: usize = 16_384;

impl Html5Tokenizer {
    pub(crate) fn push_input_internal(
        &mut self,
        input: &mut Input,
        ctx: &mut DocumentParseContext,
        stop_condition: StopCondition,
    ) -> TokenizeResult {
        self.assert_atom_table_binding(ctx);
        assert!(
            !self.end_of_stream,
            "Html5Tokenizer::push_input called after finish(); this violates end-of-stream contract"
        );
        if let Some(id) = self.input_id {
            assert_eq!(
                id,
                input.id(),
                "tokenizer is bound to a single Input instance"
            );
        } else {
            self.input_id = Some(input.id());
        }
        if stop_condition == StopCondition::YieldAfterToken && !self.tokens.is_empty() {
            return TokenizeResult::Progress;
        }

        let initial_token_count = self.tokens.len();
        let initial_cursor = self.cursor;
        let initial_state_transitions = self.stats.state_transitions;
        let mut remaining_budget = MAX_STEPS_PER_PUMP;

        while remaining_budget > 0 {
            remaining_budget -= 1;
            self.stats_inc_steps();
            let step_result = self.step(input, ctx);
            // Keep bytes_consumed aligned with absolute cursor progress.
            self.stats_set_bytes_consumed();
            if stop_condition == StopCondition::YieldAfterToken
                && self.tokens.len() > initial_token_count
            {
                break;
            }
            if matches!(step_result, Step::NeedMoreInput) {
                break;
            }
        }
        // Keep the metric consistent even if loop/control-flow changes later.
        self.stats_set_bytes_consumed();

        if remaining_budget == 0 {
            self.stats_inc_budget_exhaustions();
            let final_cursor = self.cursor;
            let final_tokens = self.tokens.len();
            let final_transitions = self.stats.state_transitions;
            #[cfg(any(test, feature = "debug-stats"))]
            log::trace!(
                target: "html5.tokenizer",
                "step budget exhausted in push_input: state={:?} cursor={} tokens={} transitions={} (initial: cursor={} tokens={} transitions={})",
                self.state,
                final_cursor,
                final_tokens,
                final_transitions,
                initial_cursor,
                initial_token_count,
                initial_state_transitions
            );
            let no_observable_progress =
                final_cursor == initial_cursor && final_tokens == initial_token_count;
            assert!(
                !no_observable_progress,
                "tokenizer step budget exhausted without observable progress: state={:?} cursor={} tokens={} transitions={} (initial: cursor={} tokens={} transitions={})",
                self.state,
                final_cursor,
                final_tokens,
                final_transitions,
                initial_cursor,
                initial_token_count,
                initial_state_transitions
            );
        }

        let observable_progress =
            self.cursor != initial_cursor || self.tokens.len() != initial_token_count;

        if observable_progress {
            TokenizeResult::Progress
        } else {
            TokenizeResult::NeedMoreInput
        }
    }

    pub(crate) fn transition_to(&mut self, next: TokenizerState) {
        if self.state == next {
            return;
        }
        #[cfg(any(test, feature = "debug-stats"))]
        {
            log::trace!(
                target: "html5.tokenizer",
                "state {:?} -> {:?} @{}",
                self.state,
                next,
                self.cursor
            );
        }
        self.state = next;
        self.mark_progress();
        self.stats_inc_state_transitions();
    }

    fn step(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        self.assert_cursor_on_char_boundary(input);
        // Explicit dispatcher scaffold. New states should be implemented as
        // dedicated handlers that return `Step::Progress` or `Step::NeedMoreInput`.
        match self.state {
            TokenizerState::Data => self.step_data(input),
            TokenizerState::RawText => self.step_raw_text(input, ctx),
            TokenizerState::Rcdata => self.step_rcdata(input, ctx),
            TokenizerState::ScriptData => self.step_script_data(input, ctx),
            TokenizerState::ScriptDataEscaped => self.step_script_data_escaped(input, ctx),
            TokenizerState::ScriptDataEscapedDash => self.step_script_data_escaped_dash(input),
            TokenizerState::ScriptDataEscapedDashDash => {
                self.step_script_data_escaped_dash_dash(input)
            }
            TokenizerState::ScriptDataDoubleEscaped => self.step_script_data_double_escaped(input),
            TokenizerState::ScriptDataDoubleEscapedDash => {
                self.step_script_data_double_escaped_dash(input)
            }
            TokenizerState::ScriptDataDoubleEscapedDashDash => {
                self.step_script_data_double_escaped_dash_dash(input)
            }
            TokenizerState::TagOpen => self.step_tag_open(input),
            TokenizerState::EndTagOpen => self.step_end_tag_open(input),
            TokenizerState::TagName => self.step_tag_name(input, ctx),
            TokenizerState::BeforeAttributeName => self.step_before_attribute_name(input, ctx),
            TokenizerState::AttributeName => self.step_attribute_name(input, ctx),
            TokenizerState::AfterAttributeName => self.step_after_attribute_name(input, ctx),
            TokenizerState::BeforeAttributeValue => self.step_before_attribute_value(input, ctx),
            TokenizerState::AttributeValueDoubleQuoted => {
                self.step_attribute_value_double_quoted(input)
            }
            TokenizerState::AttributeValueSingleQuoted => {
                self.step_attribute_value_single_quoted(input)
            }
            TokenizerState::AttributeValueUnquoted => {
                self.step_attribute_value_unquoted(input, ctx)
            }
            TokenizerState::AfterAttributeValueQuoted => {
                self.step_after_attribute_value_quoted(input, ctx)
            }
            TokenizerState::SelfClosingStartTag => self.step_self_closing_start_tag(input, ctx),
            TokenizerState::MarkupDeclarationOpen => self.step_markup_declaration_open(input, ctx),
            TokenizerState::CommentStart => self.step_comment_start(input),
            TokenizerState::CommentStartDash => self.step_comment_start_dash(input),
            TokenizerState::Comment => self.step_comment(input),
            TokenizerState::CommentEndDash => self.step_comment_end_dash(input),
            TokenizerState::CommentEnd => self.step_comment_end(input),
            TokenizerState::BogusComment => self.step_bogus_comment(input),
            TokenizerState::Doctype => self.step_doctype(input),
            TokenizerState::BeforeDoctypeName => self.step_before_doctype_name(input),
            TokenizerState::DoctypeName => self.step_doctype_name(input, ctx),
            TokenizerState::AfterDoctypeName => self.step_after_doctype_name(input),
            TokenizerState::BogusDoctype => self.step_bogus_doctype(input),
            // Placeholder: state families are wired into the dispatcher now,
            // behavior will land incrementally in follow-up issues.
            _ => {
                // Scaffold-only behavior: transition unknown states back to Data and
                // allow progress only when buffered input remains for Data to consume.
                self.transition_to(TokenizerState::Data);
                if self.has_unconsumed_input(input) {
                    Step::Progress
                } else {
                    Step::NeedMoreInput
                }
            }
        }
    }

    fn step_data(&mut self, input: &Input) -> Step {
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) == Some('<') {
            self.flush_pending_text(input);
            self.transition_to(TokenizerState::TagOpen);
            return Step::Progress;
        }
        if self.pending_text_start.is_none() {
            self.pending_text_start = Some(self.cursor);
        }
        // Core v0: character references are decoded in tokenizer text emission.
        let consumed = self.consume_while(input, |ch| ch != '<');
        assert!(
            consumed > 0,
            "data state must make progress if input remains"
        );
        if self.has_unconsumed_input(input) && self.peek(input) == Some('<') {
            // Flush the text run immediately when we encounter a delimiter so
            // token boundaries do not depend on pump scheduling granularity.
            self.flush_pending_text(input);
            self.transition_to(TokenizerState::TagOpen);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    fn step_markup_declaration_open(
        &mut self,
        input: &Input,
        _ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::MarkupDeclarationOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        // Core v0 comment/markup simplifications:
        // - Recognize only DOCTYPE and `<!--` entry points.
        // - All other `<!...` forms enter BogusComment.
        // - Fine-grained WHATWG parse-error branches are deferred.
        //
        // We enter this state after consuming "<!", so cursor is at declaration body.
        match self.match_ascii_prefix_ci(input, b"DOCTYPE") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence_ci(input, b"DOCTYPE");
                debug_assert!(did_consume, "matched DOCTYPE prefix must be consumable");
                self.begin_doctype();
                self.transition_to(TokenizerState::Doctype);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.match_ascii_prefix(input, b"--") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence(input, b"--");
                debug_assert!(did_consume, "matched comment prefix must be consumable");
                self.pending_comment_start = Some(self.cursor);
                self.transition_to(TokenizerState::CommentStart);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        // Core v0: unsupported `<!...` declarations enter bogus comment mode.
        self.pending_comment_start = Some(self.cursor);
        self.transition_to(TokenizerState::BogusComment);
        Step::Progress
    }

    #[cold]
    #[track_caller]
    fn assert_atom_table_binding(&self, ctx: &DocumentParseContext) {
        let actual = ctx.atoms.id();
        let expected = self.atom_table_id;
        assert_eq!(
            actual, expected,
            "tokenizer atom table mismatch (expected={expected}, actual={actual})"
        );
    }

    pub(crate) fn intern_atom_or_invariant(
        &self,
        ctx: &mut DocumentParseContext,
        raw: &str,
        what: &str,
    ) -> AtomId {
        match ctx.atoms.intern_ascii_folded(raw) {
            Ok(id) => id,
            Err(AtomError::OutOfIds) => {
                panic!("tokenizer atom table exhausted while interning {what}")
            }
            Err(AtomError::InvalidUtf8) => unreachable!(
                "intern_ascii_folded received &str; invalid UTF-8 is impossible ({what})"
            ),
        }
    }
}
