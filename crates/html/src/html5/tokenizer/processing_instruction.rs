use super::api::PendingProcessingInstruction;
use super::limits::{
    LIMIT_DETAIL_PROCESSING_INSTRUCTION_DATA, LIMIT_DETAIL_PROCESSING_INSTRUCTION_TARGET,
};
use super::machine::Step;
use super::states::TokenizerState;
use super::{Html5Tokenizer, is_html_space};
use crate::html5::shared::{
    DocumentParseContext, Input, ParserResourceLimit, ProcessingInstructionToken, TextSpan,
    TextValue, Token, WhatwgParseErrorCode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessingInstructionInvariantOperation {
    LiveState,
    Emission,
}

impl Html5Tokenizer {
    pub(in crate::html5::tokenizer) fn classify_processing_instruction_invariant(
        &self,
        input: &Input,
    ) -> Result<(), super::invariants::TokenizerInvariantKind> {
        self.classify_processing_instruction_invariant_for(
            input,
            ProcessingInstructionInvariantOperation::LiveState,
        )
    }

    fn classify_processing_instruction_invariant_for(
        &self,
        input: &Input,
        operation: ProcessingInstructionInvariantOperation,
    ) -> Result<(), super::invariants::TokenizerInvariantKind> {
        use super::invariants::TokenizerInvariantKind;

        let in_processing_instruction_state = self.state.is_processing_instruction();
        let Some(pending) = self.pending_processing_instruction else {
            if in_processing_instruction_state {
                return Err(
                    TokenizerInvariantKind::ProcessingInstructionStateMissingPendingMetadata,
                );
            }
            return Ok(());
        };
        if !in_processing_instruction_state {
            return Err(TokenizerInvariantKind::ProcessingInstructionMetadataOutsideState);
        }

        let text = input.as_str();
        if pending.target_start > self.cursor {
            return Err(TokenizerInvariantKind::ProcessingInstructionTargetStartAfterCursor);
        }
        let target_range_is_valid = pending
            .comment_start
            .checked_add(1)
            .is_some_and(|target_start| target_start == pending.target_start)
            && pending.target_start <= self.cursor
            && pending.target_start <= text.len()
            && text.is_char_boundary(pending.comment_start)
            && text.is_char_boundary(pending.target_start)
            && pending.target_end.is_none_or(|end| {
                end >= pending.target_start
                    && end <= self.cursor
                    && end <= text.len()
                    && text.is_char_boundary(end)
            });
        if !target_range_is_valid {
            return Err(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid);
        }
        if matches!(
            self.state,
            TokenizerState::AfterProcessingInstructionTarget
                | TokenizerState::ProcessingInstructionData
                | TokenizerState::ProcessingInstructionQuestionable
        ) && pending.target_end.is_none()
        {
            return Err(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid);
        }

        if pending.data_start.is_some_and(|start| start > self.cursor) {
            return Err(TokenizerInvariantKind::ProcessingInstructionDataStartAfterCursor);
        }
        let data_range_is_valid = pending.data_start.is_some()
            == pending.bounded_data_end.is_some()
            && pending.data_start.is_none_or(|start| {
                start <= self.cursor
                    && start <= text.len()
                    && text.is_char_boundary(start)
                    && pending.target_end.is_some_and(|end| start >= end)
            })
            && pending.bounded_data_end.is_none_or(|end| {
                pending.data_start.is_some_and(|start| end >= start)
                    && end <= self.cursor
                    && end <= text.len()
                    && text.is_char_boundary(end)
            });
        if !data_range_is_valid {
            return Err(TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid);
        }

        let state_shape_error = match self.state {
            TokenizerState::ProcessingInstructionOpen => (!(self.cursor == pending.target_start
                && pending.target_end.is_none()
                && pending.data_start.is_none()))
            .then_some(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid),
            TokenizerState::ProcessingInstructionTarget => (!(pending.target_end.is_none()
                && pending.data_start.is_none()))
            .then_some(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid),
            TokenizerState::AfterProcessingInstructionTarget => {
                if pending.target_end.is_none() {
                    Some(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid)
                } else {
                    pending
                        .data_start
                        .is_some()
                        .then_some(TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid)
                }
            }
            TokenizerState::ProcessingInstructionData => pending
                .target_end
                .is_none()
                .then_some(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid),
            TokenizerState::ProcessingInstructionQuestionable => {
                if pending.target_end.is_none() {
                    Some(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid)
                } else {
                    pending
                        .data_start
                        .is_none()
                        .then_some(TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid)
                }
            }
            _ => Some(TokenizerInvariantKind::ProcessingInstructionMetadataOutsideState),
        };
        if let Some(kind) = state_shape_error {
            return Err(kind);
        }
        if operation == ProcessingInstructionInvariantOperation::Emission {
            if pending.target_end.is_none() {
                return Err(TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid);
            }
            if pending.data_start.is_none() || pending.bounded_data_end.is_none() {
                return Err(TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid);
            }
        }
        Ok(())
    }

    pub(in crate::html5::tokenizer) fn ensure_processing_instruction_metadata_invariant(
        &mut self,
        input: &Input,
    ) -> bool {
        if let Err(kind) = self.classify_processing_instruction_invariant(input) {
            self.latch_invariant(kind);
            return false;
        }
        true
    }

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
        if !self.ensure_processing_instruction_metadata_invariant(input) {
            return Step::InvariantFailure;
        }
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
                    input,
                    ctx,
                    WhatwgParseErrorCode::InvalidFirstCharacterOfProcessingInstructionTarget,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_INVALID_FIRST_PROCESSING_INSTRUCTION_TARGET,
                    Some(ch as u32),
                );
                if self.convert_processing_instruction_to_bogus_comment() {
                    Step::Progress
                } else {
                    Step::InvariantFailure
                }
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
        if !self.ensure_processing_instruction_metadata_invariant(input) {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        match self.peek(input) {
            Some(ch) if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') => {
                let _ = self.consume(input);
                if self.check_processing_instruction_target_limit(input, ctx) {
                    Step::Progress
                } else {
                    Step::InvariantFailure
                }
            }
            Some(ch) if is_html_space(ch) || matches!(ch, '?' | '>') => {
                let target_end = self.cursor;
                let Some(pending) = self.pending_processing_instruction.as_mut() else {
                    self.latch_invariant(
                        super::invariants::TokenizerInvariantKind::
                            ProcessingInstructionStateMissingPendingMetadata,
                    );
                    return Step::InvariantFailure;
                };
                pending.target_end = Some(target_end);
                let Some(target) = input.as_str().get(pending.target_start..target_end) else {
                    self.latch_invariant(
                        super::invariants::TokenizerInvariantKind::
                            ProcessingInstructionTargetRangeInvalid,
                    );
                    return Step::InvariantFailure;
                };
                if target.eq_ignore_ascii_case("xml")
                    || target.eq_ignore_ascii_case("xml-stylesheet")
                {
                    self.record_tokenizer_parse_error(
                        input,
                        ctx,
                        WhatwgParseErrorCode::DisallowedProcessingInstructionTarget,
                        target_end,
                        super::normalization::ERROR_DETAIL_DISALLOWED_PROCESSING_INSTRUCTION_TARGET,
                        None,
                    );
                    if !self.convert_processing_instruction_to_bogus_comment() {
                        return Step::InvariantFailure;
                    }
                } else {
                    self.transition_to(TokenizerState::AfterProcessingInstructionTarget);
                }
                Step::Progress
            }
            Some(ch) => {
                self.record_tokenizer_parse_error(
                    input,
                    ctx,
                    WhatwgParseErrorCode::InvalidProcessingInstructionTarget,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_INVALID_PROCESSING_INSTRUCTION_TARGET,
                    Some(ch as u32),
                );
                if self.convert_processing_instruction_to_bogus_comment() {
                    Step::Progress
                } else {
                    Step::InvariantFailure
                }
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_after_processing_instruction_target(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterProcessingInstructionTarget);
        if !self.ensure_processing_instruction_metadata_invariant(input) {
            return Step::InvariantFailure;
        }
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
        if !self.ensure_processing_instruction_metadata_invariant(input) {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if !self.ensure_processing_instruction_data_start() {
            return Step::InvariantFailure;
        }
        match self.peek(input) {
            Some('?') => {
                let _ = self.consume(input);
                self.transition_to(TokenizerState::ProcessingInstructionQuestionable);
                Step::Progress
            }
            Some('>') => {
                let _ = self.consume(input);
                if !self.emit_pending_processing_instruction(input) {
                    return Step::InvariantFailure;
                }
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                let _ = self.consume(input);
                if self.confirm_processing_instruction_data_through_cursor(input, ctx) {
                    Step::Progress
                } else {
                    Step::InvariantFailure
                }
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
        if !self.ensure_processing_instruction_metadata_invariant(input) {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) == Some('>') {
            let _ = self.consume(input);
            if !self.emit_pending_processing_instruction(input) {
                return Step::InvariantFailure;
            }
            self.transition_to(TokenizerState::Data);
            Step::Progress
        } else {
            // The already-consumed `?` is now confirmed as data. The current
            // character remains unconsumed and is reprocessed in PI data.
            if !self.confirm_processing_instruction_data_through_cursor(input, ctx) {
                return Step::InvariantFailure;
            }
            self.transition_to(TokenizerState::ProcessingInstructionData);
            Step::Progress
        }
    }

    fn ensure_processing_instruction_data_start(&mut self) -> bool {
        let Some(pending) = self.pending_processing_instruction.as_mut() else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    ProcessingInstructionStateMissingPendingMetadata,
            );
            return false;
        };
        if pending.data_start.is_none() {
            pending.data_start = Some(self.cursor);
            pending.bounded_data_end = Some(self.cursor);
        }
        true
    }

    fn check_processing_instruction_target_limit(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> bool {
        let max = self.max_processing_instruction_target_bytes();
        let Some(target_start) = self
            .pending_processing_instruction
            .as_ref()
            .map(|pending| pending.target_start)
        else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    ProcessingInstructionStateMissingPendingMetadata,
            );
            return false;
        };
        let Some(len) = self.cursor.checked_sub(target_start) else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    ProcessingInstructionTargetStartAfterCursor,
            );
            return false;
        };
        let report_position = {
            let Some(pending) = self.pending_processing_instruction.as_mut() else {
                self.latch_invariant(
                    super::invariants::TokenizerInvariantKind::
                        ProcessingInstructionStateMissingPendingMetadata,
                );
                return false;
            };
            if len <= max {
                return true;
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
                input,
                ctx,
                position,
                ParserResourceLimit::ProcessingInstructionTargetBytes,
                LIMIT_DETAIL_PROCESSING_INSTRUCTION_TARGET,
                max,
            );
        }
        true
    }

    fn confirm_processing_instruction_data_through_cursor(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> bool {
        let max = self.max_processing_instruction_data_bytes();
        let Some(start) = self
            .pending_processing_instruction
            .as_ref()
            .and_then(|pending| pending.data_start)
        else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid,
            );
            return false;
        };
        let Some(len) = self.cursor.checked_sub(start) else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    ProcessingInstructionDataStartAfterCursor,
            );
            return false;
        };
        let report_position = {
            let Some(pending) = self.pending_processing_instruction.as_mut() else {
                self.latch_invariant(
                    super::invariants::TokenizerInvariantKind::
                        ProcessingInstructionStateMissingPendingMetadata,
                );
                return false;
            };
            if len <= max {
                pending.bounded_data_end = Some(self.cursor);
                return true;
            }
            if pending.data_limit_reported {
                None
            } else {
                pending.data_limit_reported = true;
                Some(start)
            }
        };
        if let Some(position) = report_position {
            self.record_limit_error(
                input,
                ctx,
                position,
                ParserResourceLimit::ProcessingInstructionDataBytes,
                LIMIT_DETAIL_PROCESSING_INSTRUCTION_DATA,
                max,
            );
        }
        true
    }

    fn emit_pending_processing_instruction(&mut self, input: &Input) -> bool {
        if let Err(kind) = self.classify_processing_instruction_invariant_for(
            input,
            ProcessingInstructionInvariantOperation::Emission,
        ) {
            self.latch_invariant(kind);
            return false;
        }
        let Some(pending) = self.pending_processing_instruction.take() else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    ProcessingInstructionStateMissingPendingMetadata,
            );
            return false;
        };
        if pending.suppress_token {
            return true;
        }
        let Some(target_end) = pending.target_end else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid,
            );
            return false;
        };
        let Some(target) = input.as_str().get(pending.target_start..target_end) else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid,
            );
            return false;
        };
        let (Some(data_start), Some(data_end)) = (pending.data_start, pending.bounded_data_end)
        else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid,
            );
            return false;
        };
        if input.as_str().get(data_start..data_end).is_none() {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid,
            );
            return false;
        }
        let data = TextValue::Span(TextSpan::new(data_start, data_end));
        self.emit_token(Token::ProcessingInstruction(ProcessingInstructionToken {
            target: target.to_string(),
            data,
        }));
        true
    }

    fn convert_processing_instruction_to_bogus_comment(&mut self) -> bool {
        let Some(pending) = self.pending_processing_instruction.take() else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    ProcessingInstructionStateMissingPendingMetadata,
            );
            return false;
        };
        self.pending_comment_start = Some(pending.comment_start);
        self.pending_comment_limit_reported = false;
        self.transition_to(TokenizerState::BogusComment);
        true
    }

    pub(in crate::html5::tokenizer) fn discard_pending_processing_instruction_eof(
        &mut self,
        input: &Input,
    ) -> bool {
        if !self.ensure_processing_instruction_metadata_invariant(input) {
            return false;
        }
        if self.state.is_processing_instruction() {
            let _ = self.pending_processing_instruction.take();
            self.transition_to(TokenizerState::Data);
            true
        } else {
            true
        }
    }

    #[cfg(test)]
    pub(crate) fn force_processing_instruction_metadata_missing_for_test(&mut self) {
        self.state = TokenizerState::ProcessingInstructionTarget;
        self.pending_processing_instruction = None;
    }

    #[cfg(test)]
    pub(crate) fn classify_processing_instruction_emission_invariant_for_test(
        &self,
        input: &Input,
    ) -> Result<(), super::invariants::TokenizerInvariantKind> {
        self.classify_processing_instruction_invariant_for(
            input,
            ProcessingInstructionInvariantOperation::Emission,
        )
    }

    #[cfg(test)]
    pub(crate) fn emit_pending_processing_instruction_for_test(&mut self, input: &Input) -> bool {
        self.emit_pending_processing_instruction(input)
    }
}
