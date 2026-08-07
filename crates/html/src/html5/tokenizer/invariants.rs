use super::machine::Step;
use super::states::TokenizerState;
use super::{Html5Tokenizer, TokenizeResult};
use crate::html5::shared::{AttributeValue, Input, TextSpan, TextValue, Token};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenizerInvariantKind {
    SelfClosingFlagMissingSolidusPosition,
    SolidusPositionWithoutPendingTag,
    SolidusPositionOutsideCurrentPendingTag,
    SolidusPositionDoesNotReferenceConsumedSlash,
    DoctypeNameStartMissingForNameState,
    DoctypeNameStartMissingForTailScan,
    DoctypeNameStartMissingForResourceObservation,
    DoctypeNameStartAfterCursor,
    DoctypeNameRangeInvalid,
    DoctypeTailRangeInvalid,
    AsciiPrefixCandidateRangeInvalid,
    CommentStateMissingPendingStart,
    CommentPendingRangeInvalid,
    CommentPendingDelimiterOutsideCurrentRange,
    CommentPendingDelimiterDoesNotMatchState,
    TextModeEndTagCandidateRangeInvalid,
    TextModeEndTagAttributePositionInvalid,
    TextModeEndTagSolidusPositionInvalid,
    PendingTextRangeInvalid,
    CdataStateMissingPendingTextStart,
    CdataEndDelimiterOutsidePendingTextRange,
    CdataEndDelimiterDoesNotMatchState,
    ProcessingInstructionStateMissingPendingMetadata,
    ProcessingInstructionMetadataOutsideState,
    ProcessingInstructionTargetRangeInvalid,
    ProcessingInstructionDataRangeInvalid,
    ProcessingInstructionTargetStartAfterCursor,
    ProcessingInstructionDataStartAfterCursor,
}

#[cfg(feature = "parser-conformance")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TokenizerFinalAudit {
    pub(crate) eof_lifecycle_complete: bool,
    pub(crate) pending_constructs_flushed: bool,
    pub(crate) output_queue_empty: bool,
    pub(crate) active_text_mode: Option<crate::html5::tokenizer::TextModeSpec>,
}

#[cfg(feature = "parser-conformance")]
impl Html5Tokenizer {
    /// Read-only terminal audit over the tokenizer's authoritative lifecycle
    /// and pending-construction state. EOF emission has no parallel witness:
    /// `finish_with_context` owns the guarded `end_of_stream`/`eof_emitted`
    /// transition inspected here.
    pub(crate) fn final_audit_for_conformance(&self) -> TokenizerFinalAudit {
        TokenizerFinalAudit {
            eof_lifecycle_complete: self.end_of_stream && self.eof_emitted == self.config.emit_eof,
            pending_constructs_flushed: self.pending_text_mode_end_tag_matcher.is_none()
                && self.pending_text_mode_end_tag.is_none()
                && self.pending_text_start.is_none()
                && self.pending_comment_start.is_none()
                && !self.pending_comment_limit_reported
                && self.pending_processing_instruction.is_none()
                && self.pending_doctype_name.is_none()
                && self.pending_doctype_name_start.is_none()
                && self.pending_doctype_public_id.is_none()
                && self.pending_doctype_system_id.is_none()
                && !self.pending_doctype_force_quirks
                && !self.pending_doctype_limit_reported
                && self.tag_name_start.is_none()
                && self.tag_name_end.is_none()
                && !self.tag_name_complete
                && !self.current_tag_is_end
                && !self.current_tag_self_closing
                && self.current_tag_self_closing_solidus_position.is_none()
                && self.current_tag_attrs.is_empty()
                && self.current_attr_name_start.is_none()
                && self.current_attr_name_end.is_none()
                && !self.current_attr_has_value
                && self.current_attr_value_start.is_none()
                && self.current_attr_value_end.is_none()
                && !self.end_tag_prefix_consumed,
            output_queue_empty: self.tokens.is_empty(),
            active_text_mode: self.active_text_mode,
        }
    }
}

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
    SelfClosingFlagMissingSolidusPosition,
    SolidusPositionWithoutPendingTag,
    SolidusPositionOutsideCurrentPendingTag,
    SolidusPositionDoesNotReferenceConsumedSlash,
    DoctypeNameStartMissingForNameState,
    DoctypeNameStartMissingForTailScan,
    DoctypeNameStartMissingForResourceObservation,
    DoctypeNameStartAfterCursor,
    DoctypeNameRangeInvalid,
    DoctypeTailRangeInvalid,
    AsciiPrefixCandidateRangeInvalid,
    CommentStateMissingPendingStart,
    CommentPendingRangeInvalid,
    CommentPendingDelimiterOutsideCurrentRange,
    CommentPendingDelimiterDoesNotMatchState,
    TextModeEndTagCandidateRangeInvalid,
    TextModeEndTagAttributePositionInvalid,
    TextModeEndTagSolidusPositionInvalid,
    PendingTextRangeInvalid,
    CdataStateMissingPendingTextStart,
    CdataEndDelimiterOutsidePendingTextRange,
    CdataEndDelimiterDoesNotMatchState,
    ProcessingInstructionStateMissingPendingMetadata,
    ProcessingInstructionMetadataOutsideState,
    ProcessingInstructionTargetRangeInvalid,
    ProcessingInstructionDataRangeInvalid,
    ProcessingInstructionTargetStartAfterCursor,
    ProcessingInstructionDataStartAfterCursor,
}

impl From<TokenizerInvariantKind> for TokenizerInvariantError {
    fn from(kind: TokenizerInvariantKind) -> Self {
        kind.into_error()
    }
}

impl TokenizerInvariantKind {
    fn into_error(self) -> TokenizerInvariantError {
        match self {
            Self::SelfClosingFlagMissingSolidusPosition => {
                TokenizerInvariantError::SelfClosingFlagMissingSolidusPosition
            }
            Self::SolidusPositionWithoutPendingTag => {
                TokenizerInvariantError::SolidusPositionWithoutPendingTag
            }
            Self::SolidusPositionOutsideCurrentPendingTag => {
                TokenizerInvariantError::SolidusPositionOutsideCurrentPendingTag
            }
            Self::SolidusPositionDoesNotReferenceConsumedSlash => {
                TokenizerInvariantError::SolidusPositionDoesNotReferenceConsumedSlash
            }
            Self::DoctypeNameStartMissingForNameState => {
                TokenizerInvariantError::DoctypeNameStartMissingForNameState
            }
            Self::DoctypeNameStartMissingForTailScan => {
                TokenizerInvariantError::DoctypeNameStartMissingForTailScan
            }
            Self::DoctypeNameStartMissingForResourceObservation => {
                TokenizerInvariantError::DoctypeNameStartMissingForResourceObservation
            }
            Self::DoctypeNameStartAfterCursor => {
                TokenizerInvariantError::DoctypeNameStartAfterCursor
            }
            Self::DoctypeNameRangeInvalid => TokenizerInvariantError::DoctypeNameRangeInvalid,
            Self::DoctypeTailRangeInvalid => TokenizerInvariantError::DoctypeTailRangeInvalid,
            Self::AsciiPrefixCandidateRangeInvalid => {
                TokenizerInvariantError::AsciiPrefixCandidateRangeInvalid
            }
            Self::CommentStateMissingPendingStart => {
                TokenizerInvariantError::CommentStateMissingPendingStart
            }
            Self::CommentPendingRangeInvalid => TokenizerInvariantError::CommentPendingRangeInvalid,
            Self::CommentPendingDelimiterOutsideCurrentRange => {
                TokenizerInvariantError::CommentPendingDelimiterOutsideCurrentRange
            }
            Self::CommentPendingDelimiterDoesNotMatchState => {
                TokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState
            }
            Self::TextModeEndTagCandidateRangeInvalid => {
                TokenizerInvariantError::TextModeEndTagCandidateRangeInvalid
            }
            Self::TextModeEndTagAttributePositionInvalid => {
                TokenizerInvariantError::TextModeEndTagAttributePositionInvalid
            }
            Self::TextModeEndTagSolidusPositionInvalid => {
                TokenizerInvariantError::TextModeEndTagSolidusPositionInvalid
            }
            Self::PendingTextRangeInvalid => TokenizerInvariantError::PendingTextRangeInvalid,
            Self::CdataStateMissingPendingTextStart => {
                TokenizerInvariantError::CdataStateMissingPendingTextStart
            }
            Self::CdataEndDelimiterOutsidePendingTextRange => {
                TokenizerInvariantError::CdataEndDelimiterOutsidePendingTextRange
            }
            Self::CdataEndDelimiterDoesNotMatchState => {
                TokenizerInvariantError::CdataEndDelimiterDoesNotMatchState
            }
            Self::ProcessingInstructionStateMissingPendingMetadata => {
                TokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata
            }
            Self::ProcessingInstructionMetadataOutsideState => {
                TokenizerInvariantError::ProcessingInstructionMetadataOutsideState
            }
            Self::ProcessingInstructionTargetRangeInvalid => {
                TokenizerInvariantError::ProcessingInstructionTargetRangeInvalid
            }
            Self::ProcessingInstructionDataRangeInvalid => {
                TokenizerInvariantError::ProcessingInstructionDataRangeInvalid
            }
            Self::ProcessingInstructionTargetStartAfterCursor => {
                TokenizerInvariantError::ProcessingInstructionTargetStartAfterCursor
            }
            Self::ProcessingInstructionDataStartAfterCursor => {
                TokenizerInvariantError::ProcessingInstructionDataStartAfterCursor
            }
        }
    }
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
            Self::SelfClosingFlagMissingSolidusPosition => {
                f.write_str("current tag self-closing flag requires the consumed solidus position")
            }
            Self::SolidusPositionWithoutPendingTag => {
                f.write_str("current tag solidus position exists without a pending tag")
            }
            Self::SolidusPositionOutsideCurrentPendingTag => f.write_str(
                "current tag solidus position precedes the current pending tag name",
            ),
            Self::SolidusPositionDoesNotReferenceConsumedSlash => f.write_str(
                "current tag solidus position does not reference a consumed slash before the cursor",
            ),
            Self::DoctypeNameStartMissingForNameState => {
                f.write_str("doctype-name state requires its retained start offset")
            }
            Self::DoctypeNameStartMissingForTailScan => {
                f.write_str("doctype tail scanning requires the retained doctype-name start offset")
            }
            Self::DoctypeNameStartMissingForResourceObservation => f.write_str(
                "doctype resource-limit observation requires the retained doctype-name start offset",
            ),
            Self::DoctypeNameStartAfterCursor => {
                f.write_str("retained doctype-name start offset is after the tokenizer cursor")
            }
            Self::DoctypeNameRangeInvalid => {
                f.write_str("doctype-name range is internally invalid")
            }
            Self::DoctypeTailRangeInvalid => {
                f.write_str("doctype quoted-tail range is internally invalid")
            }
            Self::AsciiPrefixCandidateRangeInvalid => {
                f.write_str("ASCII-prefix candidate range is internally invalid")
            }
            Self::CommentStateMissingPendingStart => {
                f.write_str("comment state is missing its pending comment start")
            }
            Self::CommentPendingRangeInvalid => {
                f.write_str("pending comment range is internally invalid")
            }
            Self::CommentPendingDelimiterOutsideCurrentRange => f.write_str(
                "comment EOF recovery delimiter is outside the current pending comment range",
            ),
            Self::CommentPendingDelimiterDoesNotMatchState => f.write_str(
                "comment EOF recovery delimiter does not match the active comment state",
            ),
            Self::TextModeEndTagCandidateRangeInvalid => {
                f.write_str("text-mode end-tag candidate range is internally invalid")
            }
            Self::TextModeEndTagAttributePositionInvalid => f.write_str(
                "text-mode end-tag attribute diagnostic position is not the candidate closing greater-than sign",
            ),
            Self::TextModeEndTagSolidusPositionInvalid => f.write_str(
                "text-mode end-tag trailing-solidus diagnostic position is not the accepted slash inside the current candidate",
            ),
            Self::PendingTextRangeInvalid => {
                f.write_str("pending tokenizer text range is internally invalid")
            }
            Self::CdataStateMissingPendingTextStart => {
                f.write_str("CDATA state is missing its pending text start")
            }
            Self::CdataEndDelimiterOutsidePendingTextRange => f.write_str(
                "CDATA closing delimiter is outside the current pending text range",
            ),
            Self::CdataEndDelimiterDoesNotMatchState => {
                f.write_str("CDATA closing delimiter does not match the active CDATA-end state")
            }
            Self::ProcessingInstructionStateMissingPendingMetadata => {
                f.write_str("processing-instruction state is missing pending metadata")
            }
            Self::ProcessingInstructionMetadataOutsideState => {
                f.write_str("processing-instruction metadata exists outside its state family")
            }
            Self::ProcessingInstructionTargetRangeInvalid => {
                f.write_str("processing-instruction target range is internally invalid")
            }
            Self::ProcessingInstructionDataRangeInvalid => {
                f.write_str("processing-instruction data range is internally invalid")
            }
            Self::ProcessingInstructionTargetStartAfterCursor => f.write_str(
                "processing-instruction target start is after the tokenizer cursor",
            ),
            Self::ProcessingInstructionDataStartAfterCursor => f.write_str(
                "processing-instruction data start is after the tokenizer cursor",
            ),
        }
    }
}

impl std::error::Error for TokenizerInvariantError {}

impl Html5Tokenizer {
    pub(crate) fn capture_invariant_snapshot(&self) -> TokenizerInvariantSnapshot {
        TokenizerInvariantSnapshot::capture(self)
    }

    pub(crate) fn invariant_failure_kind(&self) -> Option<TokenizerInvariantKind> {
        self.invariant_failure
    }

    pub(in crate::html5::tokenizer) fn ensure_current_tag_solidus_invariant(
        &mut self,
        input: &Input,
    ) -> bool {
        if self.invariant_failure.is_some() {
            return false;
        }
        match self.check_current_tag_solidus_invariant(input) {
            Ok(()) => true,
            Err(kind) => {
                self.invariant_failure = Some(kind);
                false
            }
        }
    }

    pub(in crate::html5::tokenizer) fn ensure_text_mode_matcher_invariant(
        &mut self,
        input: &Input,
    ) -> bool {
        if self.invariant_failure.is_some() {
            return false;
        }
        let Some(matcher) = self.pending_text_mode_end_tag_matcher else {
            return true;
        };
        match matcher
            .validate_live_candidate_range(input.as_str().as_bytes())
            .and_then(|()| matcher.validate_live_diagnostic_evidence(input.as_str().as_bytes()))
        {
            Ok(()) => true,
            Err(kind) => {
                self.invariant_failure = Some(kind);
                false
            }
        }
    }

    pub(in crate::html5::tokenizer) fn latch_invariant(&mut self, kind: TokenizerInvariantKind) {
        if self.invariant_failure.is_none() {
            self.invariant_failure = Some(kind);
        }
    }

    fn check_current_tag_solidus_invariant(
        &self,
        input: &Input,
    ) -> Result<(), TokenizerInvariantKind> {
        let position = self.current_tag_self_closing_solidus_position;
        if self.current_tag_self_closing && position.is_none() {
            return Err(TokenizerInvariantKind::SelfClosingFlagMissingSolidusPosition);
        }
        let Some(position) = position else {
            return Ok(());
        };
        let Some(tag_name_start) = self.tag_name_start else {
            return Err(TokenizerInvariantKind::SolidusPositionWithoutPendingTag);
        };
        if position < tag_name_start {
            return Err(TokenizerInvariantKind::SolidusPositionOutsideCurrentPendingTag);
        }
        if position >= self.cursor || input.as_str().as_bytes().get(position) != Some(&b'/') {
            return Err(TokenizerInvariantKind::SolidusPositionDoesNotReferenceConsumedSlash);
        }
        Ok(())
    }

    pub(crate) fn check_invariants(&self, input: &Input) -> Result<(), TokenizerInvariantError> {
        let len = input.as_str().len();

        if self.input_id.is_some() && self.input_id != Some(input.id()) {
            return Err(TokenizerInvariantError::InputBindingMismatch {
                tokenizer_input_id: self.input_id,
                input_id: input.id(),
            });
        }

        if self.state.owns_pending_comment() {
            let Some(start) = self.pending_comment_start else {
                return Err(TokenizerInvariantError::CommentStateMissingPendingStart);
            };
            if start > self.cursor
                || self.cursor > len
                || !input.as_str().is_char_boundary(start)
                || !input.as_str().is_char_boundary(self.cursor)
            {
                return Err(TokenizerInvariantError::CommentPendingRangeInvalid);
            }
        }
        check_offset(input, "cursor", self.cursor, true)?;
        check_optional_offset(input, "pending_text_start", self.pending_text_start)?;
        if !self.state.owns_pending_comment() {
            check_optional_offset(input, "pending_comment_start", self.pending_comment_start)?;
        }
        self.classify_processing_instruction_invariant(input)
            .map_err(TokenizerInvariantError::from)?;
        check_optional_offset(
            input,
            "pending_doctype_name_start",
            self.pending_doctype_name_start,
        )?;
        if let Some(matcher) = self.pending_text_mode_end_tag_matcher {
            matcher
                .validate_live_candidate_range(input.as_str().as_bytes())
                .and_then(|()| matcher.validate_live_diagnostic_evidence(input.as_str().as_bytes()))
                .map_err(TokenizerInvariantError::from)?;
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
            "current_tag_self_closing_solidus_position",
            self.current_tag_self_closing_solidus_position,
        )?;
        self.check_current_tag_solidus_invariant(input)
            .map_err(TokenizerInvariantError::from)?;
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
                        if let AttributeValue::Span(span) = &attr.value {
                            check_span(input, "start_tag.attr_value", *span)?;
                        }
                    }
                }
                Token::Comment { text } => {
                    check_text_value(input, "comment.text", text)?;
                }
                Token::ProcessingInstruction(processing_instruction) => {
                    check_text_value(
                        input,
                        "processing_instruction.data",
                        &processing_instruction.data,
                    )?;
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
