use super::machine::Step;
use super::states::TokenizerState;
use super::{Html5Tokenizer, TokenizeResult};
use crate::html5::shared::{AttributeValue, Input, TextSpan, TextValue, Token};

/// Debug/runtime tokenizer hardening checks.
///
/// These checks are enabled in debug/test builds and in release when the
/// `parser_invariants` feature is enabled. They are intentionally scoped to
/// guarantees the tokenizer already relies on today:
/// - adversarial document input must not violate tokenizer invariants when the
///   tokenizer API contracts are respected,
/// - a pump that returns `Progress` must make observable forward progress,
/// - a pump that returns `NeedMoreInput` must not have made observable forward
///   progress in that same call,
/// - internal byte indices stay inside the decoded input buffer and on UTF-8
///   boundaries,
/// - queued borrowed spans remain resolvable against the current `Input`, and
/// - EOF bookkeeping remains internally consistent.
///
/// The tokenizer still permits state-only transitions as part of resumable
/// parsing. Those transitions count as observable machine progress through the
/// `progress_epoch` witness exposed in snapshots below. Internal API misuse and
/// engine invariant breaches may still panic on hard-fail paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TokenizerInvariantSnapshot {
    pub(crate) cursor: usize,
    pub(crate) queued_tokens: usize,
    pub(crate) state: TokenizerState,
    pub(crate) end_of_stream: bool,
    pub(crate) eof_emitted: bool,
    pub(crate) progress_epoch: u64,
}

impl TokenizerInvariantSnapshot {
    pub(crate) fn capture(tokenizer: &Html5Tokenizer) -> Self {
        Self {
            cursor: tokenizer.cursor,
            queued_tokens: tokenizer.tokens.len(),
            state: tokenizer.state,
            end_of_stream: tokenizer.end_of_stream,
            eof_emitted: tokenizer.eof_emitted,
            progress_epoch: tokenizer.progress_epoch,
        }
    }

    pub(crate) fn made_observable_progress(self, after: Self) -> bool {
        self.progress_epoch != after.progress_epoch
            || self.cursor != after.cursor
            || self.queued_tokens != after.queued_tokens
            || self.state != after.state
            || self.end_of_stream != after.end_of_stream
            || self.eof_emitted != after.eof_emitted
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenizerInvariantError {
    InputBindingMismatch {
        tokenizer_input_id: Option<u64>,
        input_id: u64,
    },
    CursorOutOfBounds {
        cursor: usize,
        len: usize,
    },
    CursorNotOnCharBoundary {
        cursor: usize,
        len: usize,
    },
    OffsetOutOfBounds {
        field: &'static str,
        value: usize,
        len: usize,
    },
    OffsetNotOnCharBoundary {
        field: &'static str,
        value: usize,
        len: usize,
    },
    RangeStartMissing {
        field: &'static str,
        start_field: &'static str,
    },
    RangeOutOfBounds {
        field: &'static str,
        start: usize,
        end: usize,
        len: usize,
    },
    RangeNotOnCharBoundary {
        field: &'static str,
        start: usize,
        end: usize,
        len: usize,
    },
    PumpResultMismatch {
        boundary: &'static str,
        result: TokenizeResult,
        before: TokenizerInvariantSnapshot,
        after: TokenizerInvariantSnapshot,
    },
    EofEmittedBeforeEndOfStream,
    DuplicateQueuedEof,
    QueuedEofNotLast {
        position: usize,
        queued_tokens: usize,
    },
    InvalidQueuedSpan {
        field: &'static str,
        span: TextSpan,
        len: usize,
    },
}

impl std::fmt::Display for TokenizerInvariantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputBindingMismatch {
                tokenizer_input_id,
                input_id,
            } => write!(
                f,
                "tokenizer/input binding mismatch: tokenizer={tokenizer_input_id:?} input={input_id}"
            ),
            Self::CursorOutOfBounds { cursor, len } => {
                write!(f, "cursor out of bounds: cursor={cursor} len={len}")
            }
            Self::CursorNotOnCharBoundary { cursor, len } => {
                write!(f, "cursor not on UTF-8 boundary: cursor={cursor} len={len}")
            }
            Self::OffsetOutOfBounds { field, value, len } => {
                write!(f, "{field} out of bounds: value={value} len={len}")
            }
            Self::OffsetNotOnCharBoundary { field, value, len } => {
                write!(f, "{field} not on UTF-8 boundary: value={value} len={len}")
            }
            Self::RangeStartMissing { field, start_field } => {
                write!(f, "{field} set without corresponding {start_field}")
            }
            Self::RangeOutOfBounds {
                field,
                start,
                end,
                len,
            } => write!(
                f,
                "{field} out of bounds: start={start} end={end} len={len}"
            ),
            Self::RangeNotOnCharBoundary {
                field,
                start,
                end,
                len,
            } => write!(
                f,
                "{field} not on UTF-8 boundaries: start={start} end={end} len={len}"
            ),
            Self::PumpResultMismatch {
                boundary,
                result,
                before,
                after,
            } => {
                let made_progress = (*before).made_observable_progress(*after);
                write!(
                    f,
                    "{boundary} returned {result:?} with made_progress={made_progress}: before={before:?} after={after:?}"
                )
            }
            Self::EofEmittedBeforeEndOfStream => {
                f.write_str("EOF cannot be emitted before end-of-stream is set")
            }
            Self::DuplicateQueuedEof => f.write_str("queued token stream contains duplicate EOF"),
            Self::QueuedEofNotLast {
                position,
                queued_tokens,
            } => write!(
                f,
                "queued EOF must be the final queued token: position={position} queued_tokens={queued_tokens}"
            ),
            Self::InvalidQueuedSpan { field, span, len } => write!(
                f,
                "{field} contains invalid span {}..{} for len={len}",
                span.start, span.end
            ),
        }
    }
}

impl std::error::Error for TokenizerInvariantError {}

impl Html5Tokenizer {
    pub(crate) fn capture_invariant_snapshot(&self) -> TokenizerInvariantSnapshot {
        TokenizerInvariantSnapshot::capture(self)
    }

    pub(crate) fn check_invariants(&self, input: &Input) -> Result<(), TokenizerInvariantError> {
        let len = input.as_str().len();

        if self.input_id.is_some() && self.input_id != Some(input.id()) {
            return Err(TokenizerInvariantError::InputBindingMismatch {
                tokenizer_input_id: self.input_id,
                input_id: input.id(),
            });
        }

        check_offset(input, "cursor", self.cursor, true)?;
        check_optional_offset(input, "pending_text_start", self.pending_text_start)?;
        check_optional_offset(input, "pending_comment_start", self.pending_comment_start)?;
        check_optional_offset(
            input,
            "pending_doctype_name_start",
            self.pending_doctype_name_start,
        )?;
        if let Some(matcher) = self.pending_text_mode_end_tag_matcher {
            check_offset(
                input,
                "pending_text_mode_end_tag_matcher.start",
                matcher.start(),
                false,
            )?;
            let matcher_cursor = matcher.cursor();
            let len = input.as_str().len();
            if matcher_cursor > len {
                return Err(TokenizerInvariantError::OffsetOutOfBounds {
                    field: "pending_text_mode_end_tag_matcher.cursor",
                    value: matcher_cursor,
                    len,
                });
            }
            if matcher.start() > matcher_cursor {
                return Err(TokenizerInvariantError::RangeOutOfBounds {
                    field: "pending_text_mode_end_tag_matcher.range",
                    start: matcher.start(),
                    end: matcher_cursor,
                    len,
                });
            }
        }
        if let Some(pending_end_tag) = self.pending_text_mode_end_tag
            && pending_end_tag.cursor_after > len
        {
            return Err(TokenizerInvariantError::OffsetOutOfBounds {
                field: "pending_text_mode_end_tag.cursor_after",
                value: pending_end_tag.cursor_after,
                len,
            });
        }
        check_optional_offset(input, "tag_name_start", self.tag_name_start)?;
        check_optional_offset(input, "tag_name_end", self.tag_name_end)?;
        check_optional_offset(
            input,
            "current_attr_name_start",
            self.current_attr_name_start,
        )?;
        check_optional_offset(input, "current_attr_name_end", self.current_attr_name_end)?;
        check_optional_offset(
            input,
            "current_attr_value_start",
            self.current_attr_value_start,
        )?;
        check_optional_offset(input, "current_attr_value_end", self.current_attr_value_end)?;

        check_optional_range(
            input,
            "tag_name_range",
            "tag_name_start",
            self.tag_name_start,
            self.tag_name_end,
        )?;
        check_optional_range(
            input,
            "current_attr_name_range",
            "current_attr_name_start",
            self.current_attr_name_start,
            self.current_attr_name_end,
        )?;
        check_optional_range(
            input,
            "current_attr_value_range",
            "current_attr_value_start",
            self.current_attr_value_start,
            self.current_attr_value_end,
        )?;

        if self.eof_emitted && !self.end_of_stream {
            return Err(TokenizerInvariantError::EofEmittedBeforeEndOfStream);
        }

        let mut eof_position = None;
        for (index, token) in self.tokens.iter().enumerate() {
            match token {
                Token::Doctype { .. } | Token::EndTag { .. } => {}
                Token::StartTag { attrs, .. } => {
                    for attr in attrs {
                        if let Some(AttributeValue::Span(span)) = &attr.value {
                            check_span(input, "start_tag.attr_value", *span)?;
                        }
                    }
                }
                Token::Comment { text } => {
                    check_text_value(input, "comment.text", text)?;
                }
                Token::Text { text } => {
                    check_text_value(input, "text.text", text)?;
                }
                Token::Eof => {
                    if eof_position.replace(index).is_some() {
                        return Err(TokenizerInvariantError::DuplicateQueuedEof);
                    }
                    if !self.end_of_stream {
                        return Err(TokenizerInvariantError::EofEmittedBeforeEndOfStream);
                    }
                }
            }
        }

        if let Some(position) = eof_position
            && position + 1 != self.tokens.len()
        {
            return Err(TokenizerInvariantError::QueuedEofNotLast {
                position,
                queued_tokens: self.tokens.len(),
            });
        }

        debug_assert!(self.cursor <= len);
        Ok(())
    }

    pub(in crate::html5::tokenizer) fn debug_assert_invariants(&self, input: &Input) {
        if let Err(err) = self.check_invariants(input) {
            panic!("tokenizer invariant failure: {err}");
        }
    }

    pub(in crate::html5::tokenizer) fn debug_assert_step_result(
        &self,
        input: &Input,
        before: TokenizerInvariantSnapshot,
        step: Step,
    ) {
        self.debug_assert_invariants(input);
        if matches!(step, Step::Progress) {
            let after = self.capture_invariant_snapshot();
            if let Err(err) =
                check_progress_contract("step", TokenizeResult::Progress, before, after)
            {
                panic!("tokenizer invariant failure: {err}");
            }
        }
    }

    pub(in crate::html5::tokenizer) fn debug_assert_pump_result(
        &self,
        input: &Input,
        before: TokenizerInvariantSnapshot,
        result: TokenizeResult,
    ) {
        self.debug_assert_invariants(input);
        let after = self.capture_invariant_snapshot();
        if let Err(err) = check_progress_contract("pump", result, before, after) {
            panic!("tokenizer invariant failure: {err}");
        }
    }
}

pub(crate) fn check_progress_contract(
    boundary: &'static str,
    result: TokenizeResult,
    before: TokenizerInvariantSnapshot,
    after: TokenizerInvariantSnapshot,
) -> Result<(), TokenizerInvariantError> {
    let made_progress = before.made_observable_progress(after);
    if matches!(result, TokenizeResult::Progress) && !made_progress {
        return Err(TokenizerInvariantError::PumpResultMismatch {
            boundary,
            result,
            before,
            after,
        });
    }
    if matches!(result, TokenizeResult::NeedMoreInput) && made_progress {
        return Err(TokenizerInvariantError::PumpResultMismatch {
            boundary,
            result,
            before,
            after,
        });
    }
    Ok(())
}

fn check_optional_offset(
    input: &Input,
    field: &'static str,
    value: Option<usize>,
) -> Result<(), TokenizerInvariantError> {
    if let Some(value) = value {
        check_offset(input, field, value, false)?;
    }
    Ok(())
}

fn check_offset(
    input: &Input,
    field: &'static str,
    value: usize,
    cursor_field: bool,
) -> Result<(), TokenizerInvariantError> {
    let len = input.as_str().len();
    if value > len {
        return if cursor_field {
            Err(TokenizerInvariantError::CursorOutOfBounds { cursor: value, len })
        } else {
            Err(TokenizerInvariantError::OffsetOutOfBounds { field, value, len })
        };
    }
    if !input.as_str().is_char_boundary(value) {
        return if cursor_field {
            Err(TokenizerInvariantError::CursorNotOnCharBoundary { cursor: value, len })
        } else {
            Err(TokenizerInvariantError::OffsetNotOnCharBoundary { field, value, len })
        };
    }
    Ok(())
}

fn check_optional_range(
    input: &Input,
    field: &'static str,
    start_field: &'static str,
    start: Option<usize>,
    end: Option<usize>,
) -> Result<(), TokenizerInvariantError> {
    match (start, end) {
        (Some(start), Some(end)) => check_range(input, field, start, end),
        (None, Some(_)) => Err(TokenizerInvariantError::RangeStartMissing { field, start_field }),
        _ => Ok(()),
    }
}

fn check_range(
    input: &Input,
    field: &'static str,
    start: usize,
    end: usize,
) -> Result<(), TokenizerInvariantError> {
    let len = input.as_str().len();
    if start > end || end > len {
        return Err(TokenizerInvariantError::RangeOutOfBounds {
            field,
            start,
            end,
            len,
        });
    }
    if !input.as_str().is_char_boundary(start) || !input.as_str().is_char_boundary(end) {
        return Err(TokenizerInvariantError::RangeNotOnCharBoundary {
            field,
            start,
            end,
            len,
        });
    }
    Ok(())
}

fn check_text_value(
    input: &Input,
    field: &'static str,
    text: &TextValue,
) -> Result<(), TokenizerInvariantError> {
    if let TextValue::Span(span) = text {
        check_span(input, field, *span)?;
    }
    Ok(())
}

fn check_span(
    input: &Input,
    field: &'static str,
    span: TextSpan,
) -> Result<(), TokenizerInvariantError> {
    let len = input.as_str().len();
    if !(span.start <= span.end
        && span.end <= len
        && input.as_str().is_char_boundary(span.start)
        && input.as_str().is_char_boundary(span.end))
    {
        return Err(TokenizerInvariantError::InvalidQueuedSpan { field, span, len });
    }
    Ok(())
}
