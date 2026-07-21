use super::api::PendingProcessingInstruction;
use super::limits::{
    LIMIT_DETAIL_PROCESSING_INSTRUCTION_DATA, LIMIT_DETAIL_PROCESSING_INSTRUCTION_TARGET,
};
use super::machine::Step;
use super::states::TokenizerState;
use super::{Html5Tokenizer, is_html_space};
use crate::html5::shared::{
    DocumentParseContext, Input, ParseErrorCode, ProcessingInstructionToken, TextSpan, TextValue,
    Token,
};

impl Html5Tokenizer {
    pub(in crate::html5::tokenizer) fn begin_processing_instruction(
        &mut self,
        comment_start: usize,
    ) {
        self.pending_processing_instruction = Some(PendingProcessingInstruction {
            comment_start,
            target_start: self.cursor,
            target_end: None,
            target_limit_reported: false,
            suppress_token: false,
            data_start: None,
            bounded_data_end: None,
            data_limit_reported: false,
        });
    }

    pub(crate) fn step_processing_instruction_open(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ProcessingInstructionOpen);
        self.assert_processing_instruction_state_invariant(input);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {
                self.transition_to(TokenizerState::ProcessingInstructionTarget);
                Step::Progress
            }
            Some(ch) => {
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::Other,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_INVALID_FIRST_PROCESSING_INSTRUCTION_TARGET,
                    Some(ch as u32),
                );
                self.convert_processing_instruction_to_bogus_comment();
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_processing_instruction_target(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ProcessingInstructionTarget);
        self.assert_processing_instruction_state_invariant(input);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        match self.peek(input) {
            Some(ch) if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') => {
                let _ = self.consume(input);
                self.check_processing_instruction_target_limit(ctx);
                Step::Progress
            }
            Some(ch) if is_html_space(ch) || matches!(ch, '?' | '>') => {
                let target_end = self.cursor;
                let pending = self.pending_processing_instruction.as_mut().expect(
                    "tokenizer invariant failure: PI target state requires pending metadata",
                );
                pending.target_end = Some(target_end);
                let target = input.as_str().get(pending.target_start..target_end).expect(
                    "tokenizer invariant failure: PI target range must resolve against input",
                );
                if target.eq_ignore_ascii_case("xml")
                    || target.eq_ignore_ascii_case("xml-stylesheet")
                {
                    self.record_tokenizer_parse_error(
                        ctx,
                        ParseErrorCode::Other,
                        target_end,
                        super::normalization::ERROR_DETAIL_DISALLOWED_PROCESSING_INSTRUCTION_TARGET,
                        None,
                    );
                    self.convert_processing_instruction_to_bogus_comment();
                } else {
                    self.transition_to(TokenizerState::AfterProcessingInstructionTarget);
                }
                Step::Progress
            }
            Some(ch) => {
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::Other,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_INVALID_PROCESSING_INSTRUCTION_TARGET,
                    Some(ch as u32),
                );
                self.convert_processing_instruction_to_bogus_comment();
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_after_processing_instruction_target(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterProcessingInstructionTarget);
        self.assert_processing_instruction_state_invariant(input);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input).is_some_and(is_html_space) {
            let _ = self.consume(input);
            Step::Progress
        } else {
            self.transition_to(TokenizerState::ProcessingInstructionData);
            Step::Progress
        }
    }

    pub(crate) fn step_processing_instruction_data(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::ProcessingInstructionData);
        self.assert_processing_instruction_state_invariant(input);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        self.ensure_processing_instruction_data_start();
        match self.peek(input) {
            Some('?') => {
                let _ = self.consume(input);
                self.transition_to(TokenizerState::ProcessingInstructionQuestionable);
                Step::Progress
            }
            Some('>') => {
                let _ = self.consume(input);
                self.emit_pending_processing_instruction(input);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                let _ = self.consume(input);
                self.confirm_processing_instruction_data_through_cursor(ctx);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_processing_instruction_questionable(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(
            self.state,
            TokenizerState::ProcessingInstructionQuestionable
        );
        self.assert_processing_instruction_state_invariant(input);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) == Some('>') {
            let _ = self.consume(input);
            self.emit_pending_processing_instruction(input);
            self.transition_to(TokenizerState::Data);
            Step::Progress
        } else {
            // The already-consumed `?` is now confirmed as data. The current
            // character remains unconsumed and is reprocessed in PI data.
            self.confirm_processing_instruction_data_through_cursor(ctx);
            self.transition_to(TokenizerState::ProcessingInstructionData);
            Step::Progress
        }
    }

    fn ensure_processing_instruction_data_start(&mut self) {
        let pending = self
            .pending_processing_instruction
            .as_mut()
            .expect("tokenizer invariant failure: PI data state requires pending metadata");
        if pending.data_start.is_none() {
            pending.data_start = Some(self.cursor);
            pending.bounded_data_end = Some(self.cursor);
        }
    }

    fn check_processing_instruction_target_limit(&mut self, ctx: &mut DocumentParseContext) {
        let max = self.max_processing_instruction_target_bytes();
        let report_position = {
            let pending = self.pending_processing_instruction.as_mut().expect(
                "tokenizer invariant failure: PI target limit check requires pending metadata",
            );
            let len = self.cursor.saturating_sub(pending.target_start);
            if len <= max {
                return;
            }
            pending.suppress_token = true;
            if pending.target_limit_reported {
                None
            } else {
                pending.target_limit_reported = true;
                Some(pending.target_start)
            }
        };
        if let Some(position) = report_position {
            self.record_limit_error(
                ctx,
                position,
                LIMIT_DETAIL_PROCESSING_INSTRUCTION_TARGET,
                max,
            );
        }
    }

    fn confirm_processing_instruction_data_through_cursor(
        &mut self,
        ctx: &mut DocumentParseContext,
    ) {
        let max = self.max_processing_instruction_data_bytes();
        let report_position = {
            let pending = self.pending_processing_instruction.as_mut().expect(
                "tokenizer invariant failure: PI data accounting requires pending metadata",
            );
            let start = pending
                .data_start
                .expect("tokenizer invariant failure: PI data accounting requires a data start");
            let len = self.cursor.saturating_sub(start);
            if len <= max {
                pending.bounded_data_end = Some(self.cursor);
                return;
            }
            if pending.data_limit_reported {
                None
            } else {
                pending.data_limit_reported = true;
                Some(start)
            }
        };
        if let Some(position) = report_position {
            self.record_limit_error(ctx, position, LIMIT_DETAIL_PROCESSING_INSTRUCTION_DATA, max);
        }
    }

    fn emit_pending_processing_instruction(&mut self, input: &Input) {
        let pending = self
            .pending_processing_instruction
            .take()
            .expect("tokenizer invariant failure: PI emission requires pending metadata");
        if pending.suppress_token {
            return;
        }
        let target_end = pending
            .target_end
            .expect("tokenizer invariant failure: PI emission requires a completed target range");
        let target = input
            .as_str()
            .get(pending.target_start..target_end)
            .expect("tokenizer invariant failure: PI target range must resolve against input");
        let data_start = pending
            .data_start
            .expect("tokenizer invariant failure: PI emission requires a data range");
        let data_end = pending
            .bounded_data_end
            .expect("tokenizer invariant failure: PI emission requires a bounded data range");
        input
            .as_str()
            .get(data_start..data_end)
            .expect("tokenizer invariant failure: PI data range must resolve against input");
        let data = TextValue::Span(TextSpan::new(data_start, data_end));
        self.emit_token(Token::ProcessingInstruction(ProcessingInstructionToken {
            target: target.to_string(),
            data,
        }));
    }

    fn convert_processing_instruction_to_bogus_comment(&mut self) {
        let pending = self.pending_processing_instruction.take().expect(
            "tokenizer invariant failure: PI-to-comment conversion requires pending metadata",
        );
        self.pending_comment_start = Some(pending.comment_start);
        self.pending_comment_limit_reported = false;
        self.transition_to(TokenizerState::BogusComment);
    }

    pub(in crate::html5::tokenizer) fn discard_pending_processing_instruction_eof(&mut self) {
        if self.state.is_processing_instruction() {
            self.pending_processing_instruction
                .take()
                .expect("tokenizer invariant failure: PI EOF cleanup requires pending metadata");
            self.transition_to(TokenizerState::Data);
        } else {
            assert!(
                self.pending_processing_instruction.is_none(),
                "tokenizer invariant failure: pending PI metadata exists outside PI states at EOF"
            );
        }
    }
}
