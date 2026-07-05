use super::Html5Tokenizer;
use super::limits::LIMIT_DETAIL_COMMENT;
use super::machine::Step;
use super::states::TokenizerState;
use crate::html5::shared::{
    DocumentParseContext, Input, ParseErrorCode, TextSpan, TextValue, Token,
};

impl Html5Tokenizer {
    pub(crate) fn step_comment_start(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentStart);
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
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::Other,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                    Some('>' as u32),
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
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::Other,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                    Some('>' as u32),
                );
                let end = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::Other,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                    self.peek(input).map(|ch| ch as u32),
                );
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::Comment);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.pending_comment_start.is_none() {
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        match self.peek(input) {
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

    pub(crate) fn step_comment_end_dash(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEndDash);
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
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                let end = self.cursor.saturating_sub(2);
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
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::Other,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                    Some('!' as u32),
                );
                let _ = self.consume_if(input, '!');
                self.check_pending_comment_limit(input, ctx);
                self.transition_to(TokenizerState::CommentEndBang);
                Step::Progress
            }
            Some(_) => {
                self.record_tokenizer_parse_error(
                    ctx,
                    ParseErrorCode::Other,
                    self.cursor,
                    super::normalization::ERROR_DETAIL_MALFORMED_COMMENT,
                    self.peek(input).map(|ch| ch as u32),
                );
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
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                let end = self.cursor.saturating_sub(3);
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
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.pending_comment_start.is_none() {
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
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

    fn check_pending_comment_limit(&mut self, _input: &Input, ctx: &mut DocumentParseContext) {
        let Some(start) = self.pending_comment_start else {
            return;
        };
        if self.pending_comment_limit_reported {
            return;
        }
        let len = self.cursor.saturating_sub(start);
        if len > self.max_comment_bytes() {
            self.pending_comment_limit_reported = true;
            self.record_limit_error(ctx, start, LIMIT_DETAIL_COMMENT, self.max_comment_bytes());
        }
    }

    fn emit_pending_comment_range(
        &mut self,
        input: &Input,
        end: usize,
        ctx: &mut DocumentParseContext,
    ) {
        let start = match self.pending_comment_start.take() {
            Some(start) => start,
            None => return,
        };
        let truncated = self.truncate_input_range(input, start, end, self.max_comment_bytes());
        self.pending_comment_limit_reported = false;
        let Some((bounded_end, _was_truncated)) = truncated else {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        };
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        }
        let raw = &input.as_str()[start..bounded_end];
        if let Some(text) = self.replace_nulls_for_token_text(ctx, raw, start) {
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
                | TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::CommentEndBang
                | TokenizerState::BogusComment
        );
        if !in_comment_family {
            return;
        }
        let Some(start) = self.pending_comment_start.take() else {
            return;
        };
        let truncated =
            self.truncate_input_range(input, start, self.cursor, self.max_comment_bytes());
        self.pending_comment_limit_reported = false;
        let Some((bounded_end, _was_truncated)) = truncated else {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        };
        let end = self.cursor;
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        }
        self.emit_token(Token::Comment {
            text: TextValue::Span(TextSpan::new(start, bounded_end)),
        });
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
                | TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::CommentEndBang
                | TokenizerState::BogusComment
        );
        if !in_comment_family {
            return;
        }
        if record_eof {
            self.record_tokenizer_parse_error(
                ctx,
                crate::html5::shared::ParseErrorCode::UnexpectedEof,
                self.cursor.min(input.as_str().len()),
                super::normalization::ERROR_DETAIL_EOF_IN_COMMENT,
                None,
            );
        }
        let Some(start) = self.pending_comment_start.take() else {
            return;
        };
        let truncated =
            self.truncate_input_range(input, start, self.cursor, self.max_comment_bytes());
        self.pending_comment_limit_reported = false;
        let Some((bounded_end, _was_truncated)) = truncated else {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        };
        let end = self.cursor;
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        }
        let raw = &input.as_str()[start..bounded_end];
        if let Some(text) = self.replace_nulls_for_token_text(ctx, raw, start) {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(text),
            });
        } else {
            self.emit_token(Token::Comment {
                text: TextValue::Span(TextSpan::new(start, bounded_end)),
            });
        }
    }
}
