use super::Html5Tokenizer;
use super::machine::Step;
use super::states::TokenizerState;
use crate::html5::shared::{Input, TextSpan, TextValue, Token};

impl Html5Tokenizer {
    pub(crate) fn step_comment_start(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentStart);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::CommentStartDash);
                Step::Progress
            }
            Some('>') => {
                let end = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end);
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

    pub(crate) fn step_comment_start_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentStartDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::CommentEnd);
                Step::Progress
            }
            Some('>') => {
                let end = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end);
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

    pub(crate) fn step_comment(&mut self, input: &Input) -> Step {
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
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_comment_end_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEndDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
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

    pub(crate) fn step_comment_end(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEnd);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                let end = self.cursor.saturating_sub(2);
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('-') => {
                let _ = self.consume_if(input, '-');
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_bogus_comment(&mut self, input: &Input) -> Step {
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
            return Step::Progress;
        }
        let end = self.cursor;
        if self.consume_if(input, '>') {
            self.emit_pending_comment_range(input, end);
            self.transition_to(TokenizerState::Data);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    fn emit_pending_comment_range(&mut self, input: &Input, end: usize) {
        let start = match self.pending_comment_start.take() {
            Some(start) => start,
            None => return,
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
        self.emit_token(Token::Comment {
            text: TextValue::Span(TextSpan::new(start, end)),
        });
    }

    pub(crate) fn flush_pending_comment_eof(&mut self, input: &Input) {
        let in_comment_family = matches!(
            self.state,
            TokenizerState::CommentStart
                | TokenizerState::CommentStartDash
                | TokenizerState::Comment
                | TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::BogusComment
        );
        if !in_comment_family {
            return;
        }
        let Some(start) = self.pending_comment_start.take() else {
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
            text: TextValue::Span(TextSpan::new(start, end)),
        });
    }
}
