use super::input::MatchResult;
use super::limits::LIMIT_DETAIL_TOKEN_BATCH;
use super::stall::StallResponseMode;
use super::states::TokenizerState;
use super::{Html5Tokenizer, TokenizeResult};
use crate::html5::shared::{
    AtomError, AtomId, DocumentParseContext, Input, ParserResourceLimit, WhatwgParseErrorCode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Progress,
    NeedMoreInput,
    InvariantFailure,
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
        if !self.ensure_current_tag_solidus_invariant(input) {
            return TokenizeResult::NeedMoreInput;
        }
        if !self.ensure_text_mode_matcher_invariant(input) {
            return TokenizeResult::NeedMoreInput;
        }
        if !self.ensure_processing_instruction_metadata_invariant(input) {
            return TokenizeResult::NeedMoreInput;
        }
        if self.state.owns_pending_comment() && self.require_pending_comment_start(input).is_none()
        {
            return TokenizeResult::NeedMoreInput;
        }
        if !self.ensure_cdata_pending_text_invariant(input) {
            return TokenizeResult::NeedMoreInput;
        }
        if !self.ensure_pending_doctype_state_invariant(input) {
            return TokenizeResult::NeedMoreInput;
        }
        if stop_condition == StopCondition::DrainAvailableInput
            && self.tokens.len() >= self.max_tokens_per_batch()
        {
            #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
            self.debug_assert_invariants(input);
            return TokenizeResult::Progress;
        }
        if stop_condition == StopCondition::YieldAfterToken && !self.tokens.is_empty() {
            #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
            self.debug_assert_invariants(input);
            return TokenizeResult::Progress;
        }
        let initial_snapshot = self.capture_invariant_snapshot();
        #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
        self.debug_assert_invariants(input);

        let initial_token_count = self.tokens.len();
        let initial_cursor = self.cursor;
        let initial_state_transitions = self.stats.state_transitions;
        let mut remaining_budget = MAX_STEPS_PER_PUMP;
        let mut consecutive_stalled_steps = 0usize;

        while remaining_budget > 0 {
            remaining_budget -= 1;
            self.stats_inc_steps();
            #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
            let step_before = self.capture_invariant_snapshot();
            let before_step_cursor = self.cursor;
            let before_step_tokens = self.tokens.len();
            let mut step_result = self.step(input, ctx);
            if matches!(step_result, Step::InvariantFailure) {
                debug_assert!(self.invariant_failure.is_some());
                self.stats_set_bytes_consumed();
                return TokenizeResult::NeedMoreInput;
            }
            if let Some(stall) = self.detect_stalled_progress_step(
                before_step_cursor,
                before_step_tokens,
                step_result,
                &mut consecutive_stalled_steps,
            ) {
                step_result = self.handle_detected_step_stall(
                    input,
                    ctx,
                    stall,
                    StallResponseMode::for_current_build(),
                );
            }
            if !self.ensure_current_tag_solidus_invariant(input) {
                self.stats_set_bytes_consumed();
                return TokenizeResult::NeedMoreInput;
            }
            // Keep bytes_consumed aligned with absolute cursor progress.
            self.stats_set_bytes_consumed();
            #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
            self.debug_assert_step_result(input, step_before, step_result);
            #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
            if stop_condition == StopCondition::YieldAfterToken {
                assert!(
                    self.tokens.len()
                        <= initial_token_count
                            .checked_add(1)
                            .expect("a live token queue cannot already have usize::MAX entries"),
                    "push_input_until_token queued more than one new token in a single pump: initial_tokens={} current_tokens={} state={:?} cursor={}",
                    initial_token_count,
                    self.tokens.len(),
                    self.state,
                    self.cursor
                );
            }
            if stop_condition == StopCondition::DrainAvailableInput
                && self.tokens.len() >= self.max_tokens_per_batch()
            {
                if self.has_unconsumed_input(input) {
                    self.record_limit_error(
                        input,
                        ctx,
                        self.cursor,
                        ParserResourceLimit::TokenBatchCapacity,
                        LIMIT_DETAIL_TOKEN_BATCH,
                        self.max_tokens_per_batch(),
                    );
                }
                break;
            }
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
            let no_observable_progress = {
                let final_snapshot = self.capture_invariant_snapshot();
                !initial_snapshot.made_observable_progress(final_snapshot)
            };
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

        let final_snapshot = self.capture_invariant_snapshot();
        let observable_progress = initial_snapshot.made_observable_progress(final_snapshot);

        if observable_progress {
            #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
            self.debug_assert_pump_result(input, initial_snapshot, TokenizeResult::Progress);
            TokenizeResult::Progress
        } else {
            #[cfg(any(debug_assertions, feature = "parser_invariants", test))]
            self.debug_assert_pump_result(input, initial_snapshot, TokenizeResult::NeedMoreInput);
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
        #[cfg(test)]
        if self.test_forced_stall_steps_remaining != 0 {
            self.test_forced_stall_steps_remaining -= 1;
            self.mark_progress();
            return Step::Progress;
        }
        self.assert_cursor_on_char_boundary(input);
        if self.state.owns_pending_comment() && self.require_pending_comment_start(input).is_none()
        {
            return Step::InvariantFailure;
        }
        // Explicit dispatcher scaffold. New states should be implemented as
        // dedicated handlers that return an explicit progress, input, or
        // invariant outcome.
        match self.state {
            TokenizerState::Data => self.step_data(input, ctx),
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
            TokenizerState::TagOpen => self.step_tag_open(input, ctx),
            TokenizerState::ProcessingInstructionOpen => {
                self.step_processing_instruction_open(input, ctx)
            }
            TokenizerState::ProcessingInstructionTarget => {
                self.step_processing_instruction_target(input, ctx)
            }
            TokenizerState::AfterProcessingInstructionTarget => {
                self.step_after_processing_instruction_target(input)
            }
            TokenizerState::ProcessingInstructionData => {
                self.step_processing_instruction_data(input, ctx)
            }
            TokenizerState::ProcessingInstructionQuestionable => {
                self.step_processing_instruction_questionable(input, ctx)
            }
            TokenizerState::EndTagOpen => self.step_end_tag_open(input, ctx),
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
            TokenizerState::CdataSection => self.step_cdata_section(input, ctx),
            TokenizerState::CdataSectionBracket => self.step_cdata_section_bracket(input),
            TokenizerState::CdataSectionEnd => self.step_cdata_section_end(input, ctx),
            TokenizerState::CommentStart => self.step_comment_start(input, ctx),
            TokenizerState::CommentStartDash => self.step_comment_start_dash(input, ctx),
            TokenizerState::Comment => self.step_comment(input, ctx),
            TokenizerState::CommentLessThanSign => self.step_comment_less_than_sign(input, ctx),
            TokenizerState::CommentLessThanSignBang => {
                self.step_comment_less_than_sign_bang(input, ctx)
            }
            TokenizerState::CommentLessThanSignBangDash => {
                self.step_comment_less_than_sign_bang_dash(input, ctx)
            }
            TokenizerState::CommentLessThanSignBangDashDash => {
                self.step_comment_less_than_sign_bang_dash_dash(input, ctx)
            }
            TokenizerState::CommentEndDash => self.step_comment_end_dash(input, ctx),
            TokenizerState::CommentEnd => self.step_comment_end(input, ctx),
            TokenizerState::CommentEndBang => self.step_comment_end_bang(input, ctx),
            TokenizerState::BogusComment => self.step_bogus_comment(input, ctx),
            TokenizerState::Doctype => self.step_doctype(input, ctx),
            TokenizerState::BeforeDoctypeName => self.step_before_doctype_name(input, ctx),
            TokenizerState::DoctypeName => self.step_doctype_name(input, ctx),
            TokenizerState::AfterDoctypeName => self.step_after_doctype_name(input, ctx),
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

    fn step_data(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) == Some('<') {
            if !self.flush_pending_text_with_context(input, ctx) {
                return Step::InvariantFailure;
            }
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
            if !self.flush_pending_text_with_context(input, ctx) {
                return Step::InvariantFailure;
            }
            self.transition_to(TokenizerState::TagOpen);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    fn step_markup_declaration_open(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::MarkupDeclarationOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        // Supported markup-declaration boundary:
        // - recognize DOCTYPE and `<!--` entry points;
        // - recognize `[CDATA[` only when tree construction reports a
        //   non-HTML adjusted current node;
        // - route all other `<!...` forms to BogusComment.
        // Processing-instruction tokenization remains the explicit AE12 gap.
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

        match self.match_ascii_prefix(input, b"[CDATA[") {
            MatchResult::Matched
                if self
                    .adjusted_current_node_namespace
                    .is_some_and(|namespace| namespace != crate::names::ElementNamespace::Html) =>
            {
                let did_consume = self.consume_ascii_sequence(input, b"[CDATA[");
                debug_assert!(did_consume);
                self.pending_text_start = Some(self.cursor);
                self.transition_to(TokenizerState::CdataSection);
                return Step::Progress;
            }
            MatchResult::Matched => {}
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        // Core v0: unsupported `<!...` declarations enter bogus comment mode.
        self.record_tokenizer_parse_error_with_recovery(
            input,
            ctx,
            WhatwgParseErrorCode::IncorrectlyOpenedComment,
            self.cursor,
            crate::html5::shared::ParserRecoveryAction::StartBogusComment,
            super::normalization::legacy_diagnostic(
                super::normalization::ERROR_DETAIL_INVALID_MARKUP_DECLARATION,
                self.peek(input).map(|ch| ch as u32),
            ),
        );
        self.pending_comment_start = Some(self.cursor);
        self.transition_to(TokenizerState::BogusComment);
        Step::Progress
    }

    fn step_cdata_section(&mut self, input: &Input, _ctx: &mut DocumentParseContext) -> Step {
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != ']');
        if consumed > 0 {
            return Step::Progress;
        }
        let _ = self.consume(input);
        self.transition_to(TokenizerState::CdataSectionBracket);
        Step::Progress
    }

    fn step_cdata_section_bracket(&mut self, input: &Input) -> Step {
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) == Some(']') {
            let _ = self.consume(input);
            self.transition_to(TokenizerState::CdataSectionEnd);
        } else {
            self.transition_to(TokenizerState::CdataSection);
        }
        Step::Progress
    }

    fn step_cdata_section_end(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                let Some(delimiter_start) = self.cursor.checked_sub(2) else {
                    self.latch_invariant(
                        super::invariants::TokenizerInvariantKind::
                            CdataEndDelimiterOutsidePendingTextRange,
                    );
                    return Step::InvariantFailure;
                };
                let Some(pending_range) =
                    self.require_cdata_pending_text_range(input, delimiter_start)
                else {
                    return Step::InvariantFailure;
                };
                let Some(delimiter) = input.as_str().as_bytes().get(delimiter_start..self.cursor)
                else {
                    self.latch_invariant(
                        super::invariants::TokenizerInvariantKind::
                            CdataEndDelimiterOutsidePendingTextRange,
                    );
                    return Step::InvariantFailure;
                };
                if delimiter != b"]]" {
                    self.latch_invariant(
                        super::invariants::TokenizerInvariantKind::
                            CdataEndDelimiterDoesNotMatchState,
                    );
                    return Step::InvariantFailure;
                }
                if pending_range.start < pending_range.end {
                    let saved_cursor = self.cursor;
                    self.cursor = pending_range.end;
                    if !self.flush_pending_text_with_context(input, ctx) {
                        self.cursor = saved_cursor;
                        return Step::InvariantFailure;
                    }
                    self.cursor = saved_cursor;
                } else {
                    self.pending_text_start = None;
                }
                let _ = self.consume(input);
                self.transition_to(TokenizerState::Data);
            }
            Some(']') => {
                let _ = self.consume(input);
            }
            Some(_) => self.transition_to(TokenizerState::CdataSection),
            None => return Step::NeedMoreInput,
        }
        Step::Progress
    }

    fn require_cdata_pending_text_range(
        &mut self,
        input: &Input,
        delimiter_start: usize,
    ) -> Option<std::ops::Range<usize>> {
        let Some(start) = self.pending_text_start else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CdataStateMissingPendingTextStart,
            );
            return None;
        };
        let text = input.as_str();
        if start > delimiter_start
            || delimiter_start > self.cursor
            || self.cursor > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(delimiter_start)
            || !text.is_char_boundary(self.cursor)
        {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange,
            );
            return None;
        }
        Some(start..delimiter_start)
    }

    pub(in crate::html5::tokenizer) fn ensure_cdata_pending_text_invariant(
        &mut self,
        input: &Input,
    ) -> bool {
        if !matches!(
            self.state,
            TokenizerState::CdataSection
                | TokenizerState::CdataSectionBracket
                | TokenizerState::CdataSectionEnd
        ) {
            return true;
        }
        let Some(start) = self.pending_text_start else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CdataStateMissingPendingTextStart,
            );
            return false;
        };
        let text = input.as_str();
        if start > self.cursor
            || self.cursor > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(self.cursor)
        {
            let invariant = if self.state == TokenizerState::CdataSectionEnd {
                super::invariants::TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange
            } else {
                super::invariants::TokenizerInvariantKind::PendingTextRangeInvalid
            };
            self.latch_invariant(invariant);
            return false;
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn force_cdata_end_state_for_test(
        &mut self,
        pending_text_start: Option<usize>,
        cursor: usize,
    ) {
        self.state = TokenizerState::CdataSectionEnd;
        self.pending_text_start = pending_text_start;
        self.cursor = cursor;
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
