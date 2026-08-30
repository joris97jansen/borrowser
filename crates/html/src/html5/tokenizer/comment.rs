use super::Html5Tokenizer;
use super::limits::LIMIT_DETAIL_COMMENT;
use super::machine::Step;
use super::states::TokenizerState;
use crate::html5::shared::{
    DocumentParseContext, Input, ParserRecoveryAction, ParserResourceLimit, TextSpan, TextValue,
    Token, WhatwgParseErrorCode,
};

impl Html5Tokenizer {
    pub(crate) fn step_comment_start(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentStart);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentStartDash);
                Step::Progress
            }
            Some('>') => {
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::AbruptClosingOfEmptyComment,
                    self.cursor,
                    ParserRecoveryAction::EmitCurrentCommentAndSwitchToData,
                    super::normalization::legacy_diagnostic(
                        super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                        Some('>' as u32),
                    ),
                );
                let end = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment_start_dash(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentStartDash);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentEnd);
                Step::Progress
            }
            Some('>') => {
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::AbruptClosingOfEmptyComment,
                    self.cursor,
                    ParserRecoveryAction::EmitCurrentCommentAndSwitchToData,
                    super::normalization::legacy_diagnostic(
                        super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                        Some('>' as u32),
                    ),
                );
                let end = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::Comment);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('<') => {
                let _ = self.consume_if(input, '<');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentLessThanSign);
                Step::Progress
            }
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::CommentEndDash);
                Step::Progress
            }
            Some(_) => {
                // Linear scan invariant: each comment byte is consumed at most once
                // while searching for '-'/'-->' boundaries.
                let _ = self.consume(input);
                self.check_pending_comment_limit(input, ctx);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment_less_than_sign(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentLessThanSign);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('!') => {
                let _ = self.consume_if(input, '!');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentLessThanSignBang);
                Step::Progress
            }
            Some('<') => {
                let _ = self.consume_if(input, '<');
                self.check_pending_comment_limit(input, ctx);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment_less_than_sign_bang(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentLessThanSignBang);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) == Some('-') {
            let _ = self.consume_if(input, '-');
            self.check_pending_comment_limit(input, ctx);
            self.transition_to(TokenizerState::CommentLessThanSignBangDash);
        } else {
            self.transition_to(TokenizerState::Comment);
        }
        Step::Progress
    }

    pub(crate) fn step_comment_less_than_sign_bang_dash(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentLessThanSignBangDash);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.peek(input) == Some('-') {
            let _ = self.consume_if(input, '-');
            self.check_pending_comment_limit(input, ctx);
            self.transition_to(TokenizerState::CommentLessThanSignBangDashDash);
        } else {
            self.transition_to(TokenizerState::CommentEndDash);
        }
        Step::Progress
    }

    pub(crate) fn step_comment_less_than_sign_bang_dash_dash(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentLessThanSignBangDashDash);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                self.transition_to(TokenizerState::CommentEnd);
                Step::Progress
            }
            Some(code_point) => {
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::NestedComment,
                    self.cursor,
                    ParserRecoveryAction::RetainNestedCommentDelimiterAndReconsumeInCommentEnd {
                        code_point,
                    },
                    super::normalization::legacy_diagnostic(
                        super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                        Some(code_point as u32),
                    ),
                );
                self.transition_to(TokenizerState::CommentEnd);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment_end_dash(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEndDash);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentEnd);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment_end(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEnd);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                let Some(start) = self.require_pending_comment_start(input) else {
                    return Step::InvariantFailure;
                };
                let Some(end) = self.pending_comment_end_for_state(input, start) else {
                    return Step::InvariantFailure;
                };
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.check_pending_comment_limit(input, ctx);
                Step::Progress
            }
            Some('!') => {
                let _ = self.consume_if(input, '!');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentEndBang);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment_end_bang(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEndBang);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                let Some(start) = self.require_pending_comment_start(input) else {
                    return Step::InvariantFailure;
                };
                let Some(end) = self.pending_comment_end_for_state(input, start) else {
                    return Step::InvariantFailure;
                };
                self.record_tokenizer_parse_error_with_recovery(
                    input,
                    ctx,
                    WhatwgParseErrorCode::IncorrectlyClosedComment,
                    self.cursor,
                    ParserRecoveryAction::EmitCurrentCommentAndSwitchToData,
                    super::normalization::legacy_diagnostic(
                        super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                        Some('>' as u32),
                    ),
                );
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentEndDash);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_bogus_comment(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BogusComment);
        if self.require_pending_comment_start(input).is_none() {
            return Step::InvariantFailure;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != '>');
        if consumed > 0 {
            self.check_pending_comment_limit(input, ctx);
            return Step::Progress;
        }
        let end = self.cursor;
        if self.consume_if(input, '>') {
            self.emit_pending_comment_range(input, end, ctx);
            self.transition_to(TokenizerState::Data);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    fn check_pending_comment_limit(&mut self, input: &Input, ctx: &mut DocumentParseContext) {
        // Closing syntax advances the cursor beyond the retained comment body.
        // Reaching the byte limit while consuming `-`, `--`, or `--!` does not
        // truncate observable comment data and therefore is not a semantic
        // degradation activation.
        if matches!(
            self.state,
            TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::CommentEndBang
        ) {
            return;
        }
        let Some(start) = self.require_pending_comment_start(input) else {
            return;
        };
        if self.pending_comment_limit_reported {
            return;
        }
        let Some(len) = self.cursor.checked_sub(start) else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return;
        };
        if len > self.max_comment_bytes() {
            self.pending_comment_limit_reported = true;
            self.record_limit_error(
                input,
                ctx,
                start,
                ParserResourceLimit::CommentBytes,
                LIMIT_DETAIL_COMMENT,
                self.max_comment_bytes(),
            );
        }
    }

    fn emit_pending_comment_range(
        &mut self,
        input: &Input,
        end: usize,
        ctx: &mut DocumentParseContext,
    ) {
        let Some(start) = self.require_pending_comment_start(input) else {
            return;
        };
        if end < start
            || end > self.cursor
            || end > input.as_str().len()
            || !input.as_str().is_char_boundary(start)
            || !input.as_str().is_char_boundary(end)
        {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return;
        }
        let Some((bounded_end, _was_truncated)) =
            self.truncate_input_range(input, start, end, self.max_comment_bytes())
        else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return;
        };
        self.pending_comment_start = None;
        self.pending_comment_limit_reported = false;
        let raw = &input.as_str()[start..bounded_end];
        if let Some(text) = self.replace_nulls_for_token_text(input, ctx, raw, start) {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(text),
            });
        } else {
            self.emit_token(Token::Comment {
                text: TextValue::Span(TextSpan::new(start, bounded_end)),
            });
        }
    }

    pub(crate) fn flush_pending_comment_eof(&mut self, input: &Input) {
        let in_comment_family = matches!(
            self.state,
            TokenizerState::CommentStart
                | TokenizerState::CommentStartDash
                | TokenizerState::Comment
                | TokenizerState::CommentLessThanSign
                | TokenizerState::CommentLessThanSignBang
                | TokenizerState::CommentLessThanSignBangDash
                | TokenizerState::CommentLessThanSignBangDashDash
                | TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::CommentEndBang
                | TokenizerState::BogusComment
        );
        if !in_comment_family {
            return;
        }
        let Some(start) = self.require_pending_comment_start(input) else {
            return;
        };
        let Some(end) = self.pending_comment_end_for_state(input, start) else {
            self.pending_comment_start = None;
            return;
        };
        self.pending_comment_start = None;
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return;
        }
        let Some((bounded_end, _was_truncated)) =
            self.truncate_input_range(input, start, end, self.max_comment_bytes())
        else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return;
        };
        self.pending_comment_limit_reported = false;
        self.emit_token(Token::Comment {
            text: TextValue::Span(TextSpan::new(start, bounded_end)),
        });
        self.transition_to(TokenizerState::Data);
    }

    pub(crate) fn flush_pending_comment_eof_with_context(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        record_eof: bool,
    ) {
        let in_comment_family = matches!(
            self.state,
            TokenizerState::CommentStart
                | TokenizerState::CommentStartDash
                | TokenizerState::Comment
                | TokenizerState::CommentLessThanSign
                | TokenizerState::CommentLessThanSignBang
                | TokenizerState::CommentLessThanSignBangDash
                | TokenizerState::CommentLessThanSignBangDashDash
                | TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::CommentEndBang
                | TokenizerState::BogusComment
        );
        if !in_comment_family {
            return;
        }
        let eof_in_comment_condition = matches!(
            self.state,
            TokenizerState::CommentStart
                | TokenizerState::CommentStartDash
                | TokenizerState::Comment
                | TokenizerState::CommentLessThanSign
                | TokenizerState::CommentLessThanSignBang
                | TokenizerState::CommentLessThanSignBangDash
                | TokenizerState::CommentLessThanSignBangDashDash
                | TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::CommentEndBang
        );
        let Some(start) = self.require_pending_comment_start(input) else {
            return;
        };
        let Some(end) = self.pending_comment_end_for_state(input, start) else {
            self.pending_comment_start = None;
            return;
        };
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return;
        }
        let Some((bounded_end, _was_truncated)) =
            self.truncate_input_range(input, start, end, self.max_comment_bytes())
        else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return;
        };
        if record_eof && eof_in_comment_condition {
            self.record_tokenizer_parse_error_with_recovery(
                input,
                ctx,
                WhatwgParseErrorCode::EofInComment,
                input.as_str().len(),
                ParserRecoveryAction::EmitCurrentCommentAtEof,
                super::normalization::legacy_diagnostic(
                    super::normalization::ERROR_DETAIL_EOF_IN_COMMENT,
                    None,
                ),
            );
        }
        self.pending_comment_start = None;
        self.pending_comment_limit_reported = false;
        let raw = &input.as_str()[start..bounded_end];
        if let Some(text) = self.replace_nulls_for_token_text(input, ctx, raw, start) {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(text),
            });
        } else {
            self.emit_token(Token::Comment {
                text: TextValue::Span(TextSpan::new(start, bounded_end)),
            });
        }
        self.transition_to(TokenizerState::Data);
    }

    fn pending_comment_end_for_state(&mut self, input: &Input, start: usize) -> Option<usize> {
        let expected_delimiter: &[u8] = match self.state {
            TokenizerState::CommentStartDash
            | TokenizerState::CommentLessThanSignBangDash
            | TokenizerState::CommentEndDash => b"-",
            TokenizerState::CommentLessThanSignBangDashDash | TokenizerState::CommentEnd => b"--",
            TokenizerState::CommentEndBang => b"--!",
            TokenizerState::CommentStart
            | TokenizerState::Comment
            | TokenizerState::CommentLessThanSign
            | TokenizerState::CommentLessThanSignBang
            | TokenizerState::BogusComment => return Some(self.cursor),
            _ => {
                self.latch_invariant(
                    super::invariants::TokenizerInvariantKind::
                        CommentPendingDelimiterDoesNotMatchState,
                );
                return None;
            }
        };
        let Some(end) = self
            .cursor
            .checked_sub(expected_delimiter.len())
            .filter(|end| *end >= start)
        else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    CommentPendingDelimiterOutsideCurrentRange,
            );
            return None;
        };
        let Some(actual_delimiter) = input.as_str().as_bytes().get(end..self.cursor) else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::
                    CommentPendingDelimiterOutsideCurrentRange,
            );
            return None;
        };
        if actual_delimiter != expected_delimiter {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState,
            );
            return None;
        }
        Some(end)
    }

    pub(in crate::html5::tokenizer) fn require_pending_comment_start(
        &mut self,
        input: &Input,
    ) -> Option<usize> {
        let Some(start) = self.pending_comment_start else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentStateMissingPendingStart,
            );
            return None;
        };
        if start > self.cursor
            || self.cursor > input.as_str().len()
            || !input.as_str().is_char_boundary(start)
            || !input.as_str().is_char_boundary(self.cursor)
        {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::CommentPendingRangeInvalid,
            );
            return None;
        }
        Some(start)
    }

    #[cfg(test)]
    pub(crate) fn force_comment_eof_state_for_test(
        &mut self,
        state: TokenizerState,
        pending_start: usize,
        cursor: usize,
    ) {
        self.state = state;
        self.pending_comment_start = Some(pending_start);
        self.cursor = cursor;
    }

    #[cfg(test)]
    pub(crate) fn force_comment_state_without_pending_start_for_test(
        &mut self,
        state: TokenizerState,
    ) {
        self.state = state;
        self.pending_comment_start = None;
    }

    #[cfg(test)]
    pub(crate) fn force_comment_start_after_cursor_for_test(&mut self) {
        self.pending_comment_start = self.cursor.checked_add(1);
    }
}
