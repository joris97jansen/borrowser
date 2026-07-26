//! Feature-gated engine-test execution of the production HTML parser.
//!
//! This is the only canonical observation request boundary. It deliberately
//! does not participate in the stable parser facade.

use super::{CanonicalParserResult, IncompleteObservationReason, ObservationState};
use crate::html5::shared::{
    CapturedSurface, DocumentParseContext, ErrorPolicy, ObservationOccurrenceSequence,
    ObservationSurface, ParserObservationCapture, ParserObservationConfig,
    ParserObservationInvariant, SurfaceCaptureRequest,
};
use crate::html5::{ByteStreamDecoder, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use crate::{HtmlParseOptions, HtmlParser};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObservationRequest {
    #[default]
    NotRequested,
    Capture {
        capacity: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationTarget {
    StandaloneTokenizer,
    DocumentParser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationInput<'a> {
    Utf8(&'a str),
    Utf8Chunks(&'a [&'a str]),
    Bytes(&'a [u8]),
    ByteChunks(&'a [&'a [u8]]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserObservationRequest<'a> {
    pub target: ParserObservationTarget,
    pub input: ParserObservationInput<'a>,
    pub tokens: ObservationRequest,
    pub parse_errors: ObservationRequest,
    pub implementation_diagnostics: ObservationRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationExecutionError {
    ParserInvariant,
    TokenizerInvariant(ParserTokenizerInvariantError),
    TokenCanonicalizationInvariant,
    ObservationRecorderMissing,
    ObservationInvariant(ParserObservationInvariantError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserTokenizerInvariantError {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationInvariantError {
    ParseErrorOccurrenceOverflow,
    ImplementationDiagnosticOccurrenceOverflow,
    TokenDroppedCountOverflow,
    ParseErrorDroppedCountOverflow,
    ImplementationDiagnosticDroppedCountOverflow,
    NormalizedPositionOverflow,
    NormalizedPositionIndexDiscontinuity,
    NormalizedPositionIndexMissing,
    InvalidNormalizedPositionOffset,
}

impl std::fmt::Display for ParserObservationExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParserInvariant => formatter.write_str("production HTML parser invariant failed"),
            Self::TokenizerInvariant(invariant) => {
                write!(
                    formatter,
                    "production tokenizer invariant failed: {invariant:?}"
                )
            }
            Self::TokenCanonicalizationInvariant => {
                formatter.write_str("production token could not be resolved at its drain boundary")
            }
            Self::ObservationRecorderMissing => formatter.write_str(
                "parser observation was requested but the production recorder was missing",
            ),
            Self::ObservationInvariant(invariant) => {
                write!(
                    formatter,
                    "parser observation invariant failed: {invariant:?}"
                )
            }
        }
    }
}

impl std::error::Error for ParserObservationExecutionError {}

/// Execute the real production tokenizer or document parser with passive,
/// bounded canonical observation enabled.
pub fn execute_parser_observation(
    request: ParserObservationRequest<'_>,
) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
    let config = ParserObservationConfig {
        tokens: internal_request(request.tokens),
        parse_errors: internal_request(request.parse_errors),
        implementation_diagnostics: internal_request(request.implementation_diagnostics),
    };
    let capture = match request.target {
        ParserObservationTarget::StandaloneTokenizer => {
            execute_standalone_tokenizer(request.input, config)?
        }
        ParserObservationTarget::DocumentParser => execute_document_parser(request.input, config)?,
    };
    canonical_result(capture)
}

fn execute_document_parser(
    input: ParserObservationInput<'_>,
    config: ParserObservationConfig,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    let observation_requested = config.is_requested();
    let mut parser = HtmlParser::new_with_observations(HtmlParseOptions::default(), config)
        .map_err(|_| ParserObservationExecutionError::ParserInvariant)?;
    match input {
        ParserObservationInput::Utf8(text) => {
            push_document_text(&mut parser, text)?;
        }
        ParserObservationInput::Utf8Chunks(chunks) => {
            for chunk in chunks {
                push_document_text(&mut parser, chunk)?;
            }
        }
        ParserObservationInput::Bytes(bytes) => {
            push_document_bytes(&mut parser, bytes)?;
        }
        ParserObservationInput::ByteChunks(chunks) => {
            for chunk in chunks {
                push_document_bytes(&mut parser, chunk)?;
            }
        }
    }
    if parser.finish().is_err() {
        return Err(document_parser_error(&parser));
    }
    let capture = take_document_capture(&mut parser, observation_requested)?;
    // Run the same final materialization path as the stable facade before
    // exposing observations, even though AE13b1 does not capture the tree.
    let _ = parser
        .into_output()
        .map_err(|_| ParserObservationExecutionError::ParserInvariant)?;
    Ok(capture)
}

fn push_document_text(
    parser: &mut HtmlParser,
    text: &str,
) -> Result<(), ParserObservationExecutionError> {
    if parser.push_str(text).and_then(|()| parser.pump()).is_err() {
        return Err(document_parser_error(parser));
    }
    Ok(())
}

fn push_document_bytes(
    parser: &mut HtmlParser,
    bytes: &[u8],
) -> Result<(), ParserObservationExecutionError> {
    if parser
        .push_bytes(bytes)
        .and_then(|()| parser.pump())
        .is_err()
    {
        return Err(document_parser_error(parser));
    }
    Ok(())
}

fn document_parser_error(parser: &HtmlParser) -> ParserObservationExecutionError {
    parser
        .tokenizer_invariant_for_conformance()
        .map(public_tokenizer_invariant)
        .map(ParserObservationExecutionError::TokenizerInvariant)
        .unwrap_or(ParserObservationExecutionError::ParserInvariant)
}

fn execute_standalone_tokenizer(
    source: ParserObservationInput<'_>,
    config: ParserObservationConfig,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    let observation_requested = config.is_requested();
    let mut ctx = DocumentParseContext::with_observations(ErrorPolicy::default(), config);
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut decoder = ByteStreamDecoder::new();

    let byte_input = match source {
        ParserObservationInput::Utf8(text) => {
            push_standalone_text(&mut tokenizer, &mut input, &mut ctx, text)?;
            false
        }
        ParserObservationInput::Utf8Chunks(chunks) => {
            for chunk in chunks {
                push_standalone_text(&mut tokenizer, &mut input, &mut ctx, chunk)?;
            }
            false
        }
        ParserObservationInput::Bytes(bytes) => {
            push_standalone_bytes(&mut tokenizer, &mut decoder, &mut input, &mut ctx, bytes)?;
            true
        }
        ParserObservationInput::ByteChunks(chunks) => {
            for chunk in chunks {
                push_standalone_bytes(&mut tokenizer, &mut decoder, &mut input, &mut ctx, chunk)?;
            }
            true
        }
    };

    if byte_input {
        let _ = decoder.finish_with_context(&mut input, &mut ctx);
    } else {
        let _ = input.finish_preprocessing_observed(ctx.observation_position_index_mut());
    }
    pump_standalone(&mut tokenizer, &mut input, &mut ctx)?;
    let _ = tokenizer.finish_with_context(&input, &mut ctx);
    if let Some(invariant) = tokenizer.invariant_failure_kind() {
        return Err(ParserObservationExecutionError::TokenizerInvariant(
            public_tokenizer_invariant(invariant),
        ));
    }
    drop(tokenizer.next_batch_observed(&mut input, &mut ctx));

    take_standalone_capture(&mut ctx, observation_requested)
}

fn push_standalone_text(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    text: &str,
) -> Result<(), ParserObservationExecutionError> {
    input.push_str_observed(text, ctx.observation_position_index_mut());
    pump_standalone(tokenizer, input, ctx)
}

fn push_standalone_bytes(
    tokenizer: &mut Html5Tokenizer,
    decoder: &mut ByteStreamDecoder,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    bytes: &[u8],
) -> Result<(), ParserObservationExecutionError> {
    let _ = decoder.push_bytes_with_context(bytes, input, ctx);
    pump_standalone(tokenizer, input, ctx)
}

fn pump_standalone(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
) -> Result<(), ParserObservationExecutionError> {
    loop {
        let result = tokenizer.push_input(input, ctx);
        if let Some(invariant) = tokenizer.invariant_failure_kind() {
            return Err(ParserObservationExecutionError::TokenizerInvariant(
                public_tokenizer_invariant(invariant),
            ));
        }
        drop(tokenizer.next_batch_observed(input, ctx));
        if result == TokenizeResult::NeedMoreInput {
            return Ok(());
        }
        if result == TokenizeResult::EmittedEof {
            return Ok(());
        }
    }
}

fn public_tokenizer_invariant(
    invariant: crate::html5::tokenizer::TokenizerInvariantKind,
) -> ParserTokenizerInvariantError {
    use crate::html5::tokenizer::TokenizerInvariantKind;

    match invariant {
        TokenizerInvariantKind::SelfClosingFlagMissingSolidusPosition => {
            ParserTokenizerInvariantError::SelfClosingFlagMissingSolidusPosition
        }
        TokenizerInvariantKind::SolidusPositionWithoutPendingTag => {
            ParserTokenizerInvariantError::SolidusPositionWithoutPendingTag
        }
        TokenizerInvariantKind::SolidusPositionOutsideCurrentPendingTag => {
            ParserTokenizerInvariantError::SolidusPositionOutsideCurrentPendingTag
        }
        TokenizerInvariantKind::SolidusPositionDoesNotReferenceConsumedSlash => {
            ParserTokenizerInvariantError::SolidusPositionDoesNotReferenceConsumedSlash
        }
        TokenizerInvariantKind::DoctypeNameStartMissingForNameState => {
            ParserTokenizerInvariantError::DoctypeNameStartMissingForNameState
        }
        TokenizerInvariantKind::DoctypeNameStartMissingForTailScan => {
            ParserTokenizerInvariantError::DoctypeNameStartMissingForTailScan
        }
        TokenizerInvariantKind::DoctypeNameStartMissingForResourceObservation => {
            ParserTokenizerInvariantError::DoctypeNameStartMissingForResourceObservation
        }
        TokenizerInvariantKind::DoctypeNameStartAfterCursor => {
            ParserTokenizerInvariantError::DoctypeNameStartAfterCursor
        }
        TokenizerInvariantKind::DoctypeNameRangeInvalid => {
            ParserTokenizerInvariantError::DoctypeNameRangeInvalid
        }
        TokenizerInvariantKind::DoctypeTailRangeInvalid => {
            ParserTokenizerInvariantError::DoctypeTailRangeInvalid
        }
        TokenizerInvariantKind::AsciiPrefixCandidateRangeInvalid => {
            ParserTokenizerInvariantError::AsciiPrefixCandidateRangeInvalid
        }
        TokenizerInvariantKind::CommentStateMissingPendingStart => {
            ParserTokenizerInvariantError::CommentStateMissingPendingStart
        }
        TokenizerInvariantKind::CommentPendingRangeInvalid => {
            ParserTokenizerInvariantError::CommentPendingRangeInvalid
        }
        TokenizerInvariantKind::CommentPendingDelimiterOutsideCurrentRange => {
            ParserTokenizerInvariantError::CommentPendingDelimiterOutsideCurrentRange
        }
        TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState => {
            ParserTokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState
        }
        TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid => {
            ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid
        }
        TokenizerInvariantKind::TextModeEndTagAttributePositionInvalid => {
            ParserTokenizerInvariantError::TextModeEndTagAttributePositionInvalid
        }
        TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid => {
            ParserTokenizerInvariantError::TextModeEndTagSolidusPositionInvalid
        }
        TokenizerInvariantKind::PendingTextRangeInvalid => {
            ParserTokenizerInvariantError::PendingTextRangeInvalid
        }
        TokenizerInvariantKind::CdataStateMissingPendingTextStart => {
            ParserTokenizerInvariantError::CdataStateMissingPendingTextStart
        }
        TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange => {
            ParserTokenizerInvariantError::CdataEndDelimiterOutsidePendingTextRange
        }
        TokenizerInvariantKind::CdataEndDelimiterDoesNotMatchState => {
            ParserTokenizerInvariantError::CdataEndDelimiterDoesNotMatchState
        }
        TokenizerInvariantKind::ProcessingInstructionStateMissingPendingMetadata => {
            ParserTokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata
        }
        TokenizerInvariantKind::ProcessingInstructionMetadataOutsideState => {
            ParserTokenizerInvariantError::ProcessingInstructionMetadataOutsideState
        }
        TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid => {
            ParserTokenizerInvariantError::ProcessingInstructionTargetRangeInvalid
        }
        TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid => {
            ParserTokenizerInvariantError::ProcessingInstructionDataRangeInvalid
        }
        TokenizerInvariantKind::ProcessingInstructionTargetStartAfterCursor => {
            ParserTokenizerInvariantError::ProcessingInstructionTargetStartAfterCursor
        }
        TokenizerInvariantKind::ProcessingInstructionDataStartAfterCursor => {
            ParserTokenizerInvariantError::ProcessingInstructionDataStartAfterCursor
        }
    }
}

fn internal_request(request: ObservationRequest) -> SurfaceCaptureRequest {
    match request {
        ObservationRequest::NotRequested => SurfaceCaptureRequest::NotRequested,
        ObservationRequest::Capture { capacity } => SurfaceCaptureRequest::Capture { capacity },
    }
}

fn empty_capture() -> ParserObservationCapture {
    ParserObservationCapture {
        tokens: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        parse_errors: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        implementation_diagnostics: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        token_capture_failed: false,
        invariant: None,
    }
}

fn require_capture(
    capture: Option<ParserObservationCapture>,
    observation_requested: bool,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    match (capture, observation_requested) {
        (Some(capture), _) => Ok(capture),
        (None, false) => Ok(empty_capture()),
        (None, true) => Err(ParserObservationExecutionError::ObservationRecorderMissing),
    }
}

fn take_document_capture(
    parser: &mut HtmlParser,
    observation_requested: bool,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    require_capture(
        parser.take_observations_for_conformance(),
        observation_requested,
    )
}

fn take_standalone_capture(
    ctx: &mut DocumentParseContext,
    observation_requested: bool,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    require_capture(ctx.take_observations(), observation_requested)
}

fn canonical_result(
    capture: ParserObservationCapture,
) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
    if capture.token_capture_failed {
        return Err(ParserObservationExecutionError::TokenCanonicalizationInvariant);
    }
    if let Some(invariant) = capture.invariant {
        return Err(ParserObservationExecutionError::ObservationInvariant(
            public_observation_invariant(invariant),
        ));
    }
    Ok(CanonicalParserResult {
        tokens: finish_surface(capture.tokens),
        parse_errors: finish_surface(capture.parse_errors),
        implementation_diagnostics: finish_surface(capture.implementation_diagnostics),
        document_mode: ObservationState::NotRequested,
        tree: ObservationState::NotRequested,
        patches: ObservationState::NotRequested,
        transitions: ObservationState::NotRequested,
        unsupported_features: ObservationState::NotRequested,
        final_invariants: ObservationState::NotRequested,
    })
}

fn public_observation_invariant(
    invariant: ParserObservationInvariant,
) -> ParserObservationInvariantError {
    match invariant {
        ParserObservationInvariant::OccurrenceSequenceOverflow(
            ObservationOccurrenceSequence::ParseErrors,
        ) => ParserObservationInvariantError::ParseErrorOccurrenceOverflow,
        ParserObservationInvariant::OccurrenceSequenceOverflow(
            ObservationOccurrenceSequence::ImplementationDiagnostics,
        ) => ParserObservationInvariantError::ImplementationDiagnosticOccurrenceOverflow,
        ParserObservationInvariant::DroppedCountOverflow(ObservationSurface::Tokens) => {
            ParserObservationInvariantError::TokenDroppedCountOverflow
        }
        ParserObservationInvariant::DroppedCountOverflow(ObservationSurface::ParseErrors) => {
            ParserObservationInvariantError::ParseErrorDroppedCountOverflow
        }
        ParserObservationInvariant::DroppedCountOverflow(
            ObservationSurface::ImplementationDiagnostics,
        ) => ParserObservationInvariantError::ImplementationDiagnosticDroppedCountOverflow,
        ParserObservationInvariant::NormalizedPositionOverflow => {
            ParserObservationInvariantError::NormalizedPositionOverflow
        }
        ParserObservationInvariant::NormalizedPositionIndexDiscontinuity => {
            ParserObservationInvariantError::NormalizedPositionIndexDiscontinuity
        }
        ParserObservationInvariant::NormalizedPositionIndexMissing => {
            ParserObservationInvariantError::NormalizedPositionIndexMissing
        }
        ParserObservationInvariant::InvalidNormalizedPositionOffset => {
            ParserObservationInvariantError::InvalidNormalizedPositionOffset
        }
    }
}

fn finish_surface<T>(capture: CapturedSurface<T>) -> ObservationState<Vec<T>> {
    if !capture.requested {
        return ObservationState::NotRequested;
    }
    if capture.dropped == 0 {
        return ObservationState::Captured(capture.items);
    }
    let retained = capture.items.len();
    ObservationState::Incomplete {
        partial: capture.items,
        reason: IncompleteObservationReason::StorageLimitExceeded {
            retained,
            dropped: capture.dropped,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::shared::{
        EventPosition, ImplementationDiagnosticCode, InputCoordinateSpace, SourceBytePosition,
        SourcePositionUnavailableReason, Utf8ReplacementReason,
    };
    use crate::html5::shared::{
        ImplementationDiagnosticEvent, ObservedToken, ParserRecoveryAction,
    };
    use std::num::NonZeroU64;

    const DIAGNOSTIC_CAPACITY: usize = 128;
    const TOKEN_CAPACITY: usize = 256;

    fn observe_bytes(input: ParserObservationInput<'_>) -> CanonicalParserResult {
        observe(ParserObservationTarget::StandaloneTokenizer, input)
    }

    fn observe(
        target: ParserObservationTarget,
        input: ParserObservationInput<'_>,
    ) -> CanonicalParserResult {
        execute_parser_observation(ParserObservationRequest {
            target,
            input,
            tokens: ObservationRequest::Capture {
                capacity: TOKEN_CAPACITY,
            },
            parse_errors: ObservationRequest::Capture {
                capacity: DIAGNOSTIC_CAPACITY,
            },
            implementation_diagnostics: ObservationRequest::Capture {
                capacity: DIAGNOSTIC_CAPACITY,
            },
        })
        .expect("production tokenizer observation should succeed")
    }

    fn captured<T: std::fmt::Debug>(surface: &ObservationState<Vec<T>>) -> &[T] {
        match surface {
            ObservationState::Captured(items) => items,
            other => panic!("expected captured observation, got {other:?}"),
        }
    }

    fn observed_scalars(result: &CanonicalParserResult) -> String {
        captured(&result.tokens)
            .iter()
            .filter_map(|token| match token {
                ObservedToken::Character { data } => Some(data.as_str()),
                _ => None,
            })
            .collect()
    }

    fn normalized_offset(position: &EventPosition) -> u64 {
        match position {
            EventPosition::Known(position) => position.normalized.utf8_byte_offset,
            EventPosition::Unavailable(reason) => {
                panic!("expected exact normalized position, got {reason:?}")
            }
        }
    }

    fn normalized_coordinates(position: &EventPosition) -> (u64, u64, u64) {
        match position {
            EventPosition::Known(position) => (
                position.normalized.utf8_byte_offset,
                position.normalized.line.get(),
                position.normalized.column.get(),
            ),
            EventPosition::Unavailable(reason) => {
                panic!("expected exact normalized position, got {reason:?}")
            }
        }
    }

    fn all_partitions(len: usize) -> Vec<Vec<usize>> {
        if len <= 1 {
            return vec![Vec::new()];
        }
        (0usize..(1usize << (len - 1)))
            .map(|mask| {
                (1..len)
                    .filter(|boundary| mask & (1 << (boundary - 1)) != 0)
                    .collect()
            })
            .collect()
    }

    fn byte_chunks<'a>(bytes: &'a [u8], cuts: &[usize]) -> Vec<&'a [u8]> {
        let mut chunks = Vec::with_capacity(cuts.len() + 1);
        let mut start = 0;
        for &end in cuts {
            chunks.push(&bytes[start..end]);
            start = end;
        }
        chunks.push(&bytes[start..]);
        chunks
    }

    #[test]
    fn ae13b1_populates_only_its_three_canonical_surfaces() {
        let result = observe_bytes(ParserObservationInput::Bytes(b"<p>x</p>"));
        assert!(matches!(result.tokens, ObservationState::Captured(_)));
        assert!(matches!(result.parse_errors, ObservationState::Captured(_)));
        assert!(matches!(
            result.implementation_diagnostics,
            ObservationState::Captured(_)
        ));
        assert!(matches!(
            result.document_mode,
            ObservationState::NotRequested
        ));
        assert!(matches!(result.tree, ObservationState::NotRequested));
        assert!(matches!(result.patches, ObservationState::NotRequested));
        assert!(matches!(result.transitions, ObservationState::NotRequested));
        assert!(matches!(
            result.unsupported_features,
            ObservationState::NotRequested
        ));
        assert!(matches!(
            result.final_invariants,
            ObservationState::NotRequested
        ));
    }

    #[test]
    fn all_unrequested_surfaces_run_without_installing_capture() {
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8("<p>x</p>"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
        })
        .expect("unobserved conformance execution still runs production parsing");
        assert!(matches!(result.tokens, ObservationState::NotRequested));
        assert!(matches!(
            result.parse_errors,
            ObservationState::NotRequested
        ));
        assert!(matches!(
            result.implementation_diagnostics,
            ObservationState::NotRequested
        ));
    }

    #[test]
    fn requested_capture_cannot_silently_disappear_for_either_execution_target() {
        let config = ParserObservationConfig {
            tokens: SurfaceCaptureRequest::Capture { capacity: 1 },
            ..ParserObservationConfig::default()
        };
        let mut ctx = DocumentParseContext::with_observations(ErrorPolicy::default(), config);
        assert!(ctx.take_observations().is_some());
        assert_eq!(
            take_standalone_capture(&mut ctx, true),
            Err(ParserObservationExecutionError::ObservationRecorderMissing)
        );

        let mut parser = HtmlParser::new_with_observations(HtmlParseOptions::default(), config)
            .expect("observed document parser");
        assert!(parser.take_observations_for_conformance().is_some());
        assert_eq!(
            take_document_capture(&mut parser, true),
            Err(ParserObservationExecutionError::ObservationRecorderMissing)
        );
        assert_eq!(require_capture(None, false), Ok(empty_capture()));
    }

    #[test]
    fn observation_invariants_are_typed_execution_failures() {
        let mut capture = empty_capture();
        capture.invariant = Some(ParserObservationInvariant::OccurrenceSequenceOverflow(
            ObservationOccurrenceSequence::ParseErrors,
        ));
        assert_eq!(
            canonical_result(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::ParseErrorOccurrenceOverflow
            ))
        );

        let mut capture = empty_capture();
        capture.invariant = Some(ParserObservationInvariant::InvalidNormalizedPositionOffset);
        assert_eq!(
            canonical_result(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::InvalidNormalizedPositionOffset
            ))
        );

        let mut capture = empty_capture();
        capture.invariant = Some(ParserObservationInvariant::NormalizedPositionIndexMissing);
        assert_eq!(
            canonical_result(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::NormalizedPositionIndexMissing
            ))
        );

        let mut ctx = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
                ..ParserObservationConfig::default()
            },
        );
        let mut input = Input::new();
        input.push_str_observed("é", ctx.observation_position_index_mut());
        ctx.record_tokenizer_parse_error(
            &input,
            crate::html5::shared::ParseErrorCode::Standard(
                crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
            ),
            1,
            None,
            Some("test-invalid-normalized-offset"),
            None,
        );
        let capture = ctx.take_observations().expect("requested capture");
        assert!(
            capture.parse_errors.items.is_empty(),
            "an invalid normalized offset must not retain a false unavailable-position event"
        );
        assert_eq!(
            canonical_result(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::InvalidNormalizedPositionOffset
            ))
        );

        let mut ctx = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
                ..ParserObservationConfig::default()
            },
        );
        let mut input = Input::new();
        input.push_str_observed("x", ctx.observation_position_index_mut());
        ctx.remove_observation_position_index_for_test();
        ctx.record_tokenizer_parse_error(
            &input,
            crate::html5::shared::ParseErrorCode::Standard(
                crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
            ),
            0,
            None,
            Some("test-missing-normalized-position-index"),
            None,
        );
        let capture = ctx.take_observations().expect("requested capture");
        assert!(
            capture.parse_errors.items.is_empty(),
            "missing-index corruption must not retain a false unavailable event"
        );
        assert_eq!(
            canonical_result(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::NormalizedPositionIndexMissing
            ))
        );
    }

    #[test]
    fn literal_replacement_scalar_is_not_a_decoder_diagnostic() {
        let literal = observe_bytes(ParserObservationInput::Utf8("\u{FFFD}"));
        assert!(captured(&literal.implementation_diagnostics).is_empty());

        let decoded = observe_bytes(ParserObservationInput::Bytes(&[0xFF]));
        assert_eq!(captured(&decoded.implementation_diagnostics).len(), 1);
        assert_eq!(
            captured(&decoded.implementation_diagnostics)[0].code(),
            ImplementationDiagnosticCode::InvalidUtf8Replaced(
                Utf8ReplacementReason::InvalidSequence
            )
        );
    }

    #[test]
    fn malformed_utf8_observations_are_partition_independent() {
        let cases: &[&[u8]] = &[
            &[0xFF, b'f'],
            &[0x80, b'a'],
            &[0xE2, 0x82, b'('],
            &[0xE2, b'(', b'v'],
            &[0xC0, 0xAF],
            &[0xE0, 0x80, 0xAF],
            &[0xED, 0xA0, 0x80],
            &[0xF4, 0x90, 0x80, 0x80],
            &[0xFF, 0xE2, b'(', 0x80, b'z'],
            &[0xE2],
            &[0xE2, 0x82],
            &[0xF0, 0x9F, 0x98],
        ];
        for bytes in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(bytes));
            let expected_scalars = observed_scalars(&whole);
            let expected_diagnostics = captured(&whole.implementation_diagnostics).to_vec();
            for cuts in all_partitions(bytes.len()) {
                let chunks = byte_chunks(bytes, &cuts);
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(
                    observed_scalars(&chunked),
                    expected_scalars,
                    "decoded scalars differ for bytes={bytes:02X?}, cuts={cuts:?}"
                );
                assert_eq!(
                    captured(&chunked.implementation_diagnostics),
                    expected_diagnostics,
                    "diagnostics differ for bytes={bytes:02X?}, cuts={cuts:?}"
                );
            }
        }
    }

    #[test]
    fn utf8_positions_follow_normalized_input_and_never_claim_source_bytes() {
        let result = observe_bytes(ParserObservationInput::Bytes(&[
            b'a', b'\r', b'\n', 0xE2, b'(', 0xFF, 0xE2, 0x82,
        ]));
        let diagnostics = captured(&result.implementation_diagnostics);
        let expected = [
            (Utf8ReplacementReason::InvalidSequence, 1, 2, 2, 1),
            (Utf8ReplacementReason::InvalidSequence, 1, 6, 2, 3),
            (Utf8ReplacementReason::IncompleteSequenceAtEof, 2, 9, 2, 4),
        ];
        assert_eq!(diagnostics.len(), expected.len());
        for (event, (reason, affected, offset, line, column)) in diagnostics.iter().zip(expected) {
            let ImplementationDiagnosticEvent::InvalidUtf8Replaced {
                metadata,
                reason: actual_reason,
                payload,
            } = event
            else {
                panic!("expected UTF-8 replacement diagnostic");
            };
            assert_eq!(*actual_reason, reason);
            assert_eq!(
                payload.affected_byte_count,
                NonZeroU64::new(affected).unwrap()
            );
            let EventPosition::Known(position) = &metadata.position else {
                panic!("UTF-8 replacement position should be exact");
            };
            assert_eq!(
                position.normalized.space,
                InputCoordinateSpace::NormalizedUtf8
            );
            assert_eq!(position.normalized.utf8_byte_offset, offset);
            assert_eq!(position.normalized.line.get(), line);
            assert_eq!(position.normalized.column.get(), column);
            assert_eq!(
                position.source_bytes,
                SourceBytePosition::Unavailable(
                    SourcePositionUnavailableReason::NoInputProvenanceMap
                )
            );
        }
    }

    #[test]
    fn bounded_capture_retains_occurrences_without_reordering() {
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::StandaloneTokenizer,
            input: ParserObservationInput::Bytes(&[0xFF, 0xFF, 0xFF]),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::Capture { capacity: 1 },
        })
        .unwrap();
        let ObservationState::Incomplete { partial, reason } = result.implementation_diagnostics
        else {
            panic!("capacity overflow must be explicit");
        };
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].occurrence(), 1);
        assert_eq!(
            reason,
            IncompleteObservationReason::StorageLimitExceeded {
                retained: 1,
                dropped: 2
            }
        );
    }

    #[test]
    fn independent_surfaces_do_not_form_a_global_timeline() {
        let result = observe_bytes(ParserObservationInput::Bytes(&[0xFF, b'<']));
        let implementation = captured(&result.implementation_diagnostics);
        let parse_errors = captured(&result.parse_errors);
        assert_eq!(implementation[0].occurrence(), 1);
        assert_eq!(parse_errors[0].occurrence, 1);
    }

    #[test]
    fn production_parse_errors_keep_recording_order_without_sorting() {
        let result = observe_bytes(ParserObservationInput::Utf8("\0\0</"));
        let parse_errors = captured(&result.parse_errors);
        assert_eq!(
            parse_errors
                .iter()
                .map(|event| event.occurrence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            parse_errors
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![
                crate::html5::shared::ParseErrorCode::Standard(
                    crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
                ),
                crate::html5::shared::ParseErrorCode::Standard(
                    crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
                ),
                crate::html5::shared::ParseErrorCode::Standard(
                    crate::html5::shared::WhatwgParseErrorCode::EofBeforeTagName,
                ),
            ]
        );
    }

    #[test]
    fn unsupported_numeric_references_report_standard_conditions_and_literal_preservation() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        let cases = [
            (
                "&#xD800;",
                WhatwgParseErrorCode::SurrogateCharacterReference,
            ),
            (
                "&#55296;",
                WhatwgParseErrorCode::SurrogateCharacterReference,
            ),
            (
                "&#x110000;",
                WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
            ),
            (
                "&#1114112;",
                WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
            ),
        ];
        for (source, standard_code) in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(source.as_bytes()));
            assert_eq!(observed_scalars(&whole), source, "source={source:?}");
            let errors = captured(&whole.parse_errors);
            assert_eq!(errors.len(), 1, "source={source:?}");
            assert_eq!(
                errors[0].code,
                ParseErrorCode::Standard(standard_code),
                "source={source:?}"
            );
            assert_eq!(
                errors[0].recovery,
                Some(ParserRecoveryAction::PreserveCharacterReferenceLiteral),
                "source={source:?}"
            );
            assert!(
                !observed_scalars(&whole).contains('\u{FFFD}'),
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(captured(&chunked.parse_errors), errors);
                assert_eq!(
                    captured(&chunked.implementation_diagnostics),
                    captured(&whole.implementation_diagnostics)
                );
            }
        }
    }

    #[test]
    fn duplicate_attribute_observation_drops_only_the_later_attribute_at_every_split() {
        use crate::html5::shared::{ObservedTokenAttribute, ParseErrorCode, WhatwgParseErrorCode};

        let cases = [
            ("<div a=\"first\" a=\"second\">", "first"),
            ("<div A=\"first\" a=\"second\">", "first"),
            ("<div a a>", ""),
        ];
        for (source, expected_value) in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(source.as_bytes()));
            assert_eq!(
                captured(&whole.tokens),
                &[
                    ObservedToken::StartTag {
                        name: "div".to_owned(),
                        attributes: vec![ObservedTokenAttribute {
                            name: "a".to_owned(),
                            value: expected_value.to_owned(),
                        }],
                        self_closing: false,
                    },
                    ObservedToken::Eof,
                ],
                "source={source:?}"
            );
            let errors = captured(&whole.parse_errors);
            assert_eq!(errors.len(), 1, "source={source:?}");
            assert_eq!(
                errors[0].code,
                ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute)
            );
            assert_eq!(
                errors[0].recovery,
                Some(ParserRecoveryAction::DropDuplicateAttribute)
            );
            assert_ne!(errors[0].recovery, Some(ParserRecoveryAction::IgnoreToken));

            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(captured(&chunked.parse_errors), errors);
            }
        }
    }

    #[test]
    fn invalid_tag_open_positions_name_the_following_scalar_exactly() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        for (source, expected_position) in [
            ("< !", (1, 1, 2)),
            ("<é", (1, 1, 2)),
            ("a\r\n<é", (3, 2, 2)),
        ] {
            let whole = observe_bytes(ParserObservationInput::Bytes(source.as_bytes()));
            let event = captured(&whole.parse_errors)
                .iter()
                .find(|event| {
                    event.code
                        == ParseErrorCode::Standard(
                            WhatwgParseErrorCode::InvalidFirstCharacterOfTagName,
                        )
                })
                .unwrap_or_else(|| panic!("missing invalid opener for source={source:?}"));
            assert_eq!(
                normalized_coordinates(&event.position),
                expected_position,
                "source={source:?}"
            );
            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors)
                );
            }
        }

        for source in ["<!", "<?"] {
            let result = observe_bytes(ParserObservationInput::Utf8(source));
            assert!(!captured(&result.parse_errors).iter().any(|event| {
                event.code
                    == ParseErrorCode::Standard(
                        WhatwgParseErrorCode::InvalidFirstCharacterOfTagName,
                    )
            }));
        }
    }

    #[test]
    fn eof_diagnostics_use_the_terminal_normalized_insertion_point_at_every_split() {
        use crate::html5::shared::{
            ParseErrorCode, TokenizerExtensionParseErrorCode, WhatwgParseErrorCode,
        };

        let cases = [
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (1, 1, 2),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "</",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (2, 1, 3),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<div",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInTag),
                (4, 1, 5),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<!DOCTYPE",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInDoctype),
                (9, 1, 10),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<!--x",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInComment),
                (5, 1, 6),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<?pi x",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInProcessingInstruction),
                (6, 1, 7),
            ),
            (
                ParserObservationTarget::DocumentParser,
                "<svg><![CDATA[x",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInCdata),
                (15, 1, 16),
            ),
            (
                ParserObservationTarget::DocumentParser,
                "<title>x",
                ParseErrorCode::TokenizerExtension(TokenizerExtensionParseErrorCode::EofInTextMode),
                (8, 1, 9),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "é\r\n<",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (4, 2, 2),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "é\r<",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (4, 2, 2),
            ),
        ];

        for (target, source, code, expected_position) in cases {
            let whole = observe(target, ParserObservationInput::Bytes(source.as_bytes()));
            let event = captured(&whole.parse_errors)
                .iter()
                .find(|event| event.code == code)
                .unwrap_or_else(|| panic!("missing EOF event {code:?} for source={source:?}"));
            assert_eq!(
                normalized_coordinates(&event.position),
                expected_position,
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe(target, ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "source={source:?}, split={split}"
                );
            }
        }
    }

    #[test]
    fn tokenizer_taxonomy_separates_standard_extension_and_resource_conditions() {
        use crate::html5::shared::{
            ParseErrorCode, ParserResourceLimit, TokenizerExtensionParseErrorCode,
            WhatwgParseErrorCode,
        };

        let standard_cases = [
            (
                "<?a:b>",
                ParseErrorCode::Standard(WhatwgParseErrorCode::InvalidProcessingInstructionTarget),
            ),
            (
                "<!D",
                ParseErrorCode::Standard(WhatwgParseErrorCode::IncorrectlyOpenedComment),
            ),
            (
                "</div a=1/>",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
            ),
            (
                "&#x110000;",
                ParseErrorCode::Standard(
                    WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
                ),
            ),
        ];
        for (source, expected) in standard_cases {
            let standard = observe_bytes(ParserObservationInput::Utf8(source));
            assert_eq!(
                captured(&standard.parse_errors)
                    .first()
                    .map(|event| event.code),
                Some(expected),
                "source={source:?}"
            );
        }
        let end_tag = observe_bytes(ParserObservationInput::Utf8("</div a=1/>"));
        assert_eq!(
            captured(&end_tag.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::Standard(
                WhatwgParseErrorCode::EndTagWithAttributes
            ),]
        );
        let stray_solidus = observe_bytes(ParserObservationInput::Utf8("</div / >"));
        assert_eq!(
            captured(&stray_solidus.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::Standard(
                WhatwgParseErrorCode::UnexpectedSolidusInTag
            )]
        );
        let attribute_recovery = observe_bytes(ParserObservationInput::Utf8("<div =x>"));
        assert_eq!(
            captured(&attribute_recovery.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::Standard(
                WhatwgParseErrorCode::UnexpectedEqualsSignBeforeAttributeName
            )]
        );
        for (source, expected) in [
            (
                "<div `>",
                TokenizerExtensionParseErrorCode::DroppedGraveAccentBeforeAttributeName,
            ),
            (
                "<div ?>",
                TokenizerExtensionParseErrorCode::DroppedQuestionMarkBeforeAttributeName,
            ),
        ] {
            let result = observe_bytes(ParserObservationInput::Utf8(source));
            assert_eq!(
                captured(&result.parse_errors)
                    .iter()
                    .map(|event| event.code)
                    .collect::<Vec<_>>(),
                vec![ParseErrorCode::TokenizerExtension(expected)],
                "source={source:?}"
            );
        }
        let retained_grave = observe_bytes(ParserObservationInput::Utf8("<div a`b>"));
        assert_eq!(
            captured(&retained_grave.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::TokenizerExtension(
                TokenizerExtensionParseErrorCode::GraveAccentInAttributeName
            )]
        );

        let extension = observe_bytes(ParserObservationInput::Utf8("&#12x;"));
        assert_eq!(
            captured(&extension.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::TokenizerExtension(
                TokenizerExtensionParseErrorCode::MalformedNumericCharacterReference
            )]
        );

        let text_mode = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8("<title>x"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::Capture { capacity: 8 },
            implementation_diagnostics: ObservationRequest::NotRequested,
        })
        .expect("document parser observation");
        assert!(captured(&text_mode.parse_errors).iter().any(|event| {
            event.code
                == ParseErrorCode::TokenizerExtension(
                    TokenizerExtensionParseErrorCode::EofInTextMode,
                )
        }));

        let bounded = observe_bytes(ParserObservationInput::Utf8("&#12345678;"));
        assert!(captured(&bounded.parse_errors).is_empty());
        assert!(
            captured(&bounded.implementation_diagnostics)
                .iter()
                .any(|event| event.code()
                    == ImplementationDiagnosticCode::ParserResourceLimitActivated(
                        ParserResourceLimit::NumericCharacterReferenceDigits
                    ))
        );
    }

    #[test]
    fn comment_diagnostics_follow_exact_standard_transitions_at_every_split() {
        use crate::html5::shared::{ParseErrorCode, ParserRecoveryAction, WhatwgParseErrorCode};

        type ExpectedError = (
            WhatwgParseErrorCode,
            Option<ParserRecoveryAction>,
            (u64, u64, u64),
        );

        struct Case {
            source: &'static str,
            tokens: Vec<ObservedToken>,
            errors: Vec<ExpectedError>,
        }

        let cases = [
            Case {
                source: "<!---x-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "-x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![],
            },
            Case {
                source: "<!--a--x-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "a--x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![],
            },
            Case {
                source: "<!--a--!>",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "a".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyClosedComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAndSwitchToData),
                    (8, 1, 9),
                )],
            },
            Case {
                source: "<!--a--!x-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "a--!x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![],
            },
            Case {
                source: "<!-- <!-- nested --> -->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: " <!-- nested ".to_owned(),
                    },
                    ObservedToken::Character {
                        data: " -->".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::NestedComment,
                    Some(
                        ParserRecoveryAction::
                            RetainNestedCommentDelimiterAndReconsumeInCommentEnd {
                                code_point: ' ',
                            },
                    ),
                    (9, 1, 10),
                )],
            },
            Case {
                source: "<!-- <!-- nested",
                tokens: vec![
                    ObservedToken::Comment {
                        data: " <!-- nested".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![
                    (
                        WhatwgParseErrorCode::NestedComment,
                        Some(
                            ParserRecoveryAction::
                                RetainNestedCommentDelimiterAndReconsumeInCommentEnd {
                                    code_point: ' ',
                                },
                        ),
                        (9, 1, 10),
                    ),
                    (
                        WhatwgParseErrorCode::EofInComment,
                        Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                        (16, 1, 17),
                    ),
                ],
            },
            Case {
                source: "<!-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: String::new(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::AbruptClosingOfEmptyComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAndSwitchToData),
                    (4, 1, 5),
                )],
            },
            Case {
                source: "<!--x",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (5, 1, 6),
                )],
            },
            Case {
                source: "<!--x-",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (6, 1, 7),
                )],
            },
            Case {
                source: "<!--x--",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (7, 1, 8),
                )],
            },
            Case {
                source: "<!--x--!",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (8, 1, 9),
                )],
            },
            Case {
                source: "<!oops",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "oops".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyOpenedComment,
                    Some(ParserRecoveryAction::StartBogusComment),
                    (2, 1, 3),
                )],
            },
            Case {
                source: "<!",
                tokens: vec![
                    ObservedToken::Comment {
                        data: String::new(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyOpenedComment,
                    Some(ParserRecoveryAction::StartBogusComment),
                    (2, 1, 3),
                )],
            },
            Case {
                source: "é\r\n<!--a--!>",
                tokens: vec![
                    ObservedToken::Character {
                        data: "é\n".to_owned(),
                    },
                    ObservedToken::Comment {
                        data: "a".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyClosedComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAndSwitchToData),
                    (11, 2, 9),
                )],
            },
        ];

        for case in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(case.source.as_bytes()));
            assert_eq!(
                captured(&whole.tokens),
                case.tokens,
                "source={:?}",
                case.source
            );
            let actual_errors = captured(&whole.parse_errors)
                .iter()
                .map(|event| {
                    let ParseErrorCode::Standard(code) = event.code else {
                        panic!("comment event must use exact Standard identity: {event:?}");
                    };
                    (
                        code,
                        event.recovery,
                        normalized_coordinates(&event.position),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual_errors, case.errors, "source={:?}", case.source);

            for split in 1..case.source.len() {
                let chunks = [
                    &case.source.as_bytes()[..split],
                    &case.source.as_bytes()[split..],
                ];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(
                    captured(&chunked.tokens),
                    captured(&whole.tokens),
                    "source={:?}, split={split}",
                    case.source
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "source={:?}, split={split}",
                    case.source
                );
            }
        }
    }

    #[test]
    fn emitted_end_tag_diagnostics_come_from_semantic_token_state_at_every_split() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        let cases: &[(&str, &[(ParseErrorCode, u64)])] = &[
            (
                "</div a=1>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    9,
                )],
            ),
            (
                "</div/>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                    5,
                )],
            ),
            (
                "</div a=1/>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    10,
                )],
            ),
            (
                "</div a=\"/path\">",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    15,
                )],
            ),
            (
                "</div a='a/b'>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    13,
                )],
            ),
            (
                "</div a=a/b>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    11,
                )],
            ),
            (
                "</div / >",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedSolidusInTag),
                    7,
                )],
            ),
            (
                "</div //>",
                &[
                    (
                        ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedSolidusInTag),
                        7,
                    ),
                    (
                        ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                        7,
                    ),
                ],
            ),
        ];

        for (source, expected) in cases {
            let whole = observe_bytes(ParserObservationInput::Utf8(source));
            let whole_tokens = captured(&whole.tokens).to_vec();
            let whole_errors = captured(&whole.parse_errors).to_vec();
            assert_eq!(
                whole_tokens,
                vec![
                    ObservedToken::EndTag {
                        name: "div".to_owned()
                    },
                    ObservedToken::Eof,
                ],
                "source={source:?}"
            );
            assert_eq!(
                whole_errors
                    .iter()
                    .map(|event| (event.code, normalized_offset(&event.position)))
                    .collect::<Vec<_>>(),
                *expected,
                "source={source:?}"
            );
            assert_eq!(
                whole_errors
                    .iter()
                    .map(|event| event.occurrence)
                    .collect::<Vec<_>>(),
                (1..=whole_errors.len() as u64).collect::<Vec<_>>(),
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source[..split], &source[split..]];
                let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
                assert_eq!(
                    captured(&chunked.tokens),
                    whole_tokens,
                    "production tokens changed for source={source:?}, split={split}"
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    whole_errors,
                    "diagnostics changed for source={source:?}, split={split}"
                );
            }
        }
    }

    #[test]
    fn text_mode_end_tag_diagnostics_retain_emission_and_solidus_positions() {
        use crate::html5::shared::{ParseErrorCode, ParserRecoveryAction, WhatwgParseErrorCode};

        struct Case {
            source: &'static str,
            tag: &'static str,
            prefix_text: Option<&'static str>,
            errors: Vec<(WhatwgParseErrorCode, ParserRecoveryAction, (u64, u64, u64))>,
        }

        let cases = [
            Case {
                source: "<title>x</title a=1>",
                tag: "title",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithAttributes,
                    ParserRecoveryAction::DropEndTagAttributes,
                    (19, 1, 20),
                )],
            },
            Case {
                source: "<title>x</title />",
                tag: "title",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                    ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                    (16, 1, 17),
                )],
            },
            Case {
                source: "<title>x</title a=1 />",
                tag: "title",
                prefix_text: None,
                errors: vec![
                    (
                        WhatwgParseErrorCode::EndTagWithAttributes,
                        ParserRecoveryAction::DropEndTagAttributes,
                        (21, 1, 22),
                    ),
                    (
                        WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                        ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                        (20, 1, 21),
                    ),
                ],
            },
            Case {
                source: "<style>x</style a=1>",
                tag: "style",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithAttributes,
                    ParserRecoveryAction::DropEndTagAttributes,
                    (19, 1, 20),
                )],
            },
            Case {
                source: "<style>x</style />",
                tag: "style",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                    ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                    (16, 1, 17),
                )],
            },
            Case {
                source: "<script>x</script a=1>",
                tag: "script",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithAttributes,
                    ParserRecoveryAction::DropEndTagAttributes,
                    (21, 1, 22),
                )],
            },
            Case {
                source: "<script>x</script />",
                tag: "script",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                    ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                    (18, 1, 19),
                )],
            },
            Case {
                source: "é\r\n<title>x</title a=1 />",
                tag: "title",
                prefix_text: Some("é\n"),
                errors: vec![
                    (
                        WhatwgParseErrorCode::EndTagWithAttributes,
                        ParserRecoveryAction::DropEndTagAttributes,
                        (24, 2, 22),
                    ),
                    (
                        WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                        ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                        (23, 2, 21),
                    ),
                ],
            },
        ];

        for case in cases {
            let whole = observe(
                ParserObservationTarget::DocumentParser,
                ParserObservationInput::Bytes(case.source.as_bytes()),
            );
            let mut expected_tokens = Vec::new();
            if let Some(prefix) = case.prefix_text {
                expected_tokens.push(ObservedToken::Character {
                    data: prefix.to_owned(),
                });
            }
            expected_tokens.extend([
                ObservedToken::StartTag {
                    name: case.tag.to_owned(),
                    attributes: Vec::new(),
                    self_closing: false,
                },
                ObservedToken::Character {
                    data: "x".to_owned(),
                },
                ObservedToken::EndTag {
                    name: case.tag.to_owned(),
                },
                ObservedToken::Eof,
            ]);
            assert_eq!(
                captured(&whole.tokens),
                expected_tokens,
                "source={:?}",
                case.source
            );
            let actual_errors = captured(&whole.parse_errors)
                .iter()
                .map(|event| {
                    let ParseErrorCode::Standard(code) = event.code else {
                        panic!("text-mode end tag must use Standard identity: {event:?}");
                    };
                    (
                        code,
                        event.recovery.expect("text-mode recovery is known"),
                        normalized_coordinates(&event.position),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual_errors, case.errors, "source={:?}", case.source);
            let opening_lt = case
                .source
                .find(&format!("</{}", case.tag))
                .expect("end-tag opener") as u64
                - u64::from(case.source.starts_with('é'));
            assert!(
                actual_errors
                    .iter()
                    .all(|(_, _, position)| position.0 != opening_lt),
                "no text-mode diagnostic may use the opening '<': source={:?}",
                case.source
            );

            for split in 1..case.source.len() {
                let chunks = [
                    &case.source.as_bytes()[..split],
                    &case.source.as_bytes()[split..],
                ];
                let chunked = observe(
                    ParserObservationTarget::DocumentParser,
                    ParserObservationInput::ByteChunks(&chunks),
                );
                assert_eq!(
                    captured(&chunked.tokens),
                    captured(&whole.tokens),
                    "source={:?}, split={split}",
                    case.source
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "source={:?}, split={split}",
                    case.source
                );
            }
        }
    }

    #[test]
    fn adjacent_end_tags_do_not_reuse_a_prior_solidus_position() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        let source = "</div //></span/>";
        let whole = observe_bytes(ParserObservationInput::Utf8(source));
        let whole_tokens = captured(&whole.tokens).to_vec();
        let whole_errors = captured(&whole.parse_errors).to_vec();
        assert_eq!(
            whole_tokens,
            vec![
                ObservedToken::EndTag {
                    name: "div".to_owned()
                },
                ObservedToken::EndTag {
                    name: "span".to_owned()
                },
                ObservedToken::Eof,
            ]
        );
        assert_eq!(
            whole_errors
                .iter()
                .map(|event| (event.code, normalized_offset(&event.position)))
                .collect::<Vec<_>>(),
            vec![
                (
                    ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedSolidusInTag),
                    7,
                ),
                (
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                    7,
                ),
                (
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                    15,
                ),
            ]
        );
        for split in 1..source.len() {
            let chunks = [&source[..split], &source[split..]];
            let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
            assert_eq!(captured(&chunked.tokens), whole_tokens, "split={split}");
            assert_eq!(
                captured(&chunked.parse_errors),
                whole_errors,
                "split={split}"
            );
        }
    }

    #[test]
    fn start_tag_unquoted_solidus_semantics_are_canonical_at_every_split() {
        let cases = [
            ("<div a=b/>", "div", "b/", false),
            ("<div a=b />", "div", "b", true),
            ("<img src=x/>", "img", "x/", false),
            ("<img src=x />", "img", "x", true),
            ("<div a=/path>", "div", "/path", false),
            ("<div a=/path />", "div", "/path", true),
        ];

        for (source, expected_name, expected_value, expected_self_closing) in cases {
            let whole = observe_bytes(ParserObservationInput::Utf8(source));
            let whole_tokens = captured(&whole.tokens).to_vec();
            let [
                ObservedToken::StartTag {
                    name,
                    attributes,
                    self_closing,
                },
                ObservedToken::Eof,
            ] = whole_tokens.as_slice()
            else {
                panic!("expected one start tag and EOF for source={source:?}");
            };
            assert_eq!(name, expected_name, "source={source:?}");
            assert_eq!(*self_closing, expected_self_closing, "source={source:?}");
            assert_eq!(attributes.len(), 1, "source={source:?}");
            assert_eq!(
                attributes[0].name,
                if expected_name == "img" { "src" } else { "a" },
                "source={source:?}"
            );
            assert_eq!(attributes[0].value, expected_value, "source={source:?}");
            assert!(
                captured(&whole.parse_errors).is_empty(),
                "source={source:?}"
            );
            assert!(
                captured(&whole.implementation_diagnostics).is_empty(),
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source[..split], &source[split..]];
                let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
                assert_eq!(captured(&chunked.tokens), whole_tokens, "split={split}");
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "split={split}"
                );
                assert_eq!(
                    captured(&chunked.implementation_diagnostics),
                    captured(&whole.implementation_diagnostics),
                    "split={split}"
                );
            }
        }
    }

    #[test]
    fn inconsistent_solidus_state_maps_to_exact_conformance_invariant() {
        let mut parser = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
            },
        )
        .expect("observed parser");
        parser.push_str("<div").expect("partial tag");
        parser.pump().expect("park in tag-name state");
        parser.force_self_closing_flag_without_solidus_for_test();
        parser.push_str(">").expect("tag terminator");

        assert_eq!(parser.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&parser),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::SelfClosingFlagMissingSolidusPosition
            )
        );
    }

    #[test]
    fn missing_doctype_name_start_uses_generic_stable_and_exact_conformance_invariants() {
        for (prefix, suffix, expected) in [
            (
                "<!DOCTYPE html",
                ">",
                ParserTokenizerInvariantError::DoctypeNameStartMissingForNameState,
            ),
            (
                "<!DOCTYPE html ",
                "PUBLIC \"x\">",
                ParserTokenizerInvariantError::DoctypeNameStartMissingForTailScan,
            ),
        ] {
            let mut parser = HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                },
            )
            .expect("observed parser");
            parser.push_str(prefix).expect("partial doctype push");
            parser.pump().expect("partial doctype pump");
            parser.force_missing_doctype_name_start_for_test();
            parser
                .push_str(suffix)
                .expect("corrupt doctype continuation push");

            assert_eq!(parser.pump(), Err(crate::HtmlParseError::Invariant));
            assert_eq!(
                document_parser_error(&parser),
                ParserObservationExecutionError::TokenizerInvariant(expected)
            );
            let capture = parser
                .take_observations_for_conformance()
                .expect("requested capture");
            assert!(
                capture.implementation_diagnostics.items.is_empty(),
                "corrupt doctype metadata must not retain a cursor-positioned diagnostic"
            );
        }
    }

    #[test]
    fn invalid_doctype_name_start_order_uses_generic_stable_and_exact_conformance_invariants() {
        for (prefix, suffix) in [
            ("<!DOCTYPE html", ">"),
            ("<!DOCTYPE html ", "PUBLIC \"x\">"),
        ] {
            let mut parser = HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                },
            )
            .expect("observed parser");
            parser.push_str(prefix).expect("partial doctype push");
            parser.pump().expect("partial doctype pump");
            parser.force_doctype_name_start_after_cursor_for_test();
            parser
                .push_str(suffix)
                .expect("corrupt doctype continuation push");

            assert_eq!(parser.pump(), Err(crate::HtmlParseError::Invariant));
            assert_eq!(
                document_parser_error(&parser),
                ParserObservationExecutionError::TokenizerInvariant(
                    ParserTokenizerInvariantError::DoctypeNameStartAfterCursor
                )
            );
            let capture = parser
                .take_observations_for_conformance()
                .expect("requested capture");
            assert!(capture.tokens.items.is_empty());
            assert!(capture.implementation_diagnostics.items.is_empty());
        }

        let mut parser = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
            },
        )
        .expect("observed parser");
        parser
            .push_str("<!DOCTYPE html")
            .expect("partial doctype push");
        parser.pump().expect("partial doctype pump");
        parser.force_doctype_resource_start_after_cursor_for_test();
        assert_eq!(parser.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&parser),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::DoctypeNameStartAfterCursor
            )
        );
        let capture = parser
            .take_observations_for_conformance()
            .expect("requested capture");
        assert!(capture.tokens.items.is_empty());
        assert!(capture.implementation_diagnostics.items.is_empty());
    }

    #[test]
    fn comment_delimiter_and_text_mode_evidence_invariants_propagate_exactly() {
        let mut comment = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
            },
        )
        .expect("observed parser");
        comment.push_str("<!--xx-").expect("partial comment");
        comment.pump().expect("park comment at EOF");
        comment.force_comment_end_bang_state_for_test();
        assert_eq!(comment.finish(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&comment),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState
            )
        );
        let capture = comment
            .take_observations_for_conformance()
            .expect("requested capture");
        assert!(capture.tokens.items.is_empty());
        assert!(capture.parse_errors.items.is_empty());

        for (attribute, solidus, expected) in [
            (
                Some(0),
                None,
                ParserTokenizerInvariantError::TextModeEndTagAttributePositionInvalid,
            ),
            (
                None,
                Some(0),
                ParserTokenizerInvariantError::TextModeEndTagSolidusPositionInvalid,
            ),
        ] {
            let mut parser = HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                },
            )
            .expect("observed parser");
            parser.push_str("</title>").expect("candidate input");
            parser.force_text_mode_end_tag_evidence_for_test(
                0,
                "</title>".len(),
                attribute,
                solidus,
            );
            assert_eq!(parser.pump(), Err(crate::HtmlParseError::Invariant));
            assert_eq!(
                document_parser_error(&parser),
                ParserObservationExecutionError::TokenizerInvariant(expected)
            );
            let capture = parser
                .take_observations_for_conformance()
                .expect("requested capture");
            assert!(capture.parse_errors.items.is_empty());
        }
    }

    #[test]
    fn final_tokenizer_corruption_invariants_use_generic_stable_and_exact_conformance_errors() {
        fn parser() -> HtmlParser {
            HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                },
            )
            .expect("observed parser")
        }

        let mut comment = parser();
        comment.push_str("<!--x--").expect("partial comment");
        comment.pump().expect("park comment end");
        comment.force_comment_state_without_pending_start_for_test(
            crate::html5::tokenizer::TokenizerState::CommentEnd,
        );
        comment.push_str(">").expect("comment close");
        assert_eq!(comment.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&comment),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CommentStateMissingPendingStart
            )
        );
        let capture = comment
            .take_observations_for_conformance()
            .expect("requested capture");
        assert!(capture.tokens.items.is_empty());
        assert!(capture.parse_errors.items.is_empty());
        assert!(capture.implementation_diagnostics.items.is_empty());

        let mut comment_range = parser();
        comment_range
            .push_str("<!--x")
            .expect("partial comment range");
        comment_range.pump().expect("park comment at input end");
        comment_range.force_comment_start_after_cursor_for_test();
        assert_eq!(
            comment_range.finish(),
            Err(crate::HtmlParseError::Invariant)
        );
        assert_eq!(
            document_parser_error(&comment_range),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CommentPendingRangeInvalid
            )
        );
        let capture = comment_range
            .take_observations_for_conformance()
            .expect("requested capture");
        assert!(capture.tokens.items.is_empty());
        assert!(capture.parse_errors.items.is_empty());
        assert!(capture.implementation_diagnostics.items.is_empty());

        let mut cdata = parser();
        cdata.push_str("xx>").expect("CDATA corruption input");
        cdata.force_cdata_end_state_for_test(Some(0), 2);
        assert_eq!(cdata.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&cdata),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CdataEndDelimiterDoesNotMatchState
            )
        );
        let capture = cdata
            .take_observations_for_conformance()
            .expect("requested capture");
        assert!(capture.tokens.items.is_empty());
        assert!(capture.parse_errors.items.is_empty());

        let mut missing_cdata = parser();
        missing_cdata
            .push_str("]]>")
            .expect("missing CDATA ownership input");
        missing_cdata.force_cdata_end_state_for_test(None, 2);
        assert_eq!(missing_cdata.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&missing_cdata),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CdataStateMissingPendingTextStart
            )
        );
        let capture = missing_cdata
            .take_observations_for_conformance()
            .expect("requested capture");
        assert!(capture.tokens.items.is_empty());
        assert!(capture.parse_errors.items.is_empty());
        assert!(capture.implementation_diagnostics.items.is_empty());

        let mut doctype_range = parser();
        doctype_range.force_empty_doctype_name_range_for_test();
        assert_eq!(
            doctype_range.finish(),
            Err(crate::HtmlParseError::Invariant)
        );
        assert_eq!(
            document_parser_error(&doctype_range),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::DoctypeNameRangeInvalid
            )
        );
        let capture = doctype_range
            .take_observations_for_conformance()
            .expect("requested capture");
        assert!(capture.tokens.items.is_empty());
        assert!(capture.parse_errors.items.is_empty());
        assert!(capture.implementation_diagnostics.items.is_empty());

        let mut candidate = parser();
        candidate.push_str("</title>").expect("candidate input");
        candidate.force_text_mode_end_tag_evidence_for_test(1, 8, None, None);
        assert_eq!(candidate.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&candidate),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid
            )
        );

        let mut invalid_opener = parser();
        invalid_opener
            .push_str("<xtitle>")
            .expect("invalid retained opener input");
        invalid_opener.force_text_mode_end_tag_evidence_for_test(0, 8, None, None);
        assert_eq!(invalid_opener.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&invalid_opener),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid
            )
        );

        let mut ascii_scan = parser();
        ascii_scan
            .push_str("<!DOCTYPE html PUBLIC")
            .expect("doctype input");
        ascii_scan.force_doctype_ascii_prefix_range_invalid_for_test();
        assert_eq!(ascii_scan.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&ascii_scan),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::AsciiPrefixCandidateRangeInvalid
            )
        );

        let mut quoted_tail = parser();
        quoted_tail.push_str("\"x\"").expect("quoted tail input");
        quoted_tail.force_doctype_quoted_tail_range_invalid_for_test();
        assert_eq!(quoted_tail.pump(), Err(crate::HtmlParseError::Invariant));
        assert_eq!(
            document_parser_error(&quoted_tail),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::DoctypeTailRangeInvalid
            )
        );

        let mut processing_instruction = parser();
        processing_instruction
            .push_str("<?x")
            .expect("processing-instruction input");
        processing_instruction.force_processing_instruction_metadata_missing_for_test();
        assert_eq!(
            processing_instruction.pump(),
            Err(crate::HtmlParseError::Invariant)
        );
        assert_eq!(
            document_parser_error(&processing_instruction),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata
            )
        );
    }

    #[test]
    fn every_ae13b1_tokenizer_invariant_has_an_exact_public_conformance_identity() {
        use crate::html5::tokenizer::TokenizerInvariantKind;

        let cases = [
            (
                TokenizerInvariantKind::SelfClosingFlagMissingSolidusPosition,
                ParserTokenizerInvariantError::SelfClosingFlagMissingSolidusPosition,
            ),
            (
                TokenizerInvariantKind::SolidusPositionWithoutPendingTag,
                ParserTokenizerInvariantError::SolidusPositionWithoutPendingTag,
            ),
            (
                TokenizerInvariantKind::SolidusPositionOutsideCurrentPendingTag,
                ParserTokenizerInvariantError::SolidusPositionOutsideCurrentPendingTag,
            ),
            (
                TokenizerInvariantKind::SolidusPositionDoesNotReferenceConsumedSlash,
                ParserTokenizerInvariantError::SolidusPositionDoesNotReferenceConsumedSlash,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartMissingForNameState,
                ParserTokenizerInvariantError::DoctypeNameStartMissingForNameState,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartMissingForTailScan,
                ParserTokenizerInvariantError::DoctypeNameStartMissingForTailScan,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartMissingForResourceObservation,
                ParserTokenizerInvariantError::DoctypeNameStartMissingForResourceObservation,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartAfterCursor,
                ParserTokenizerInvariantError::DoctypeNameStartAfterCursor,
            ),
            (
                TokenizerInvariantKind::DoctypeNameRangeInvalid,
                ParserTokenizerInvariantError::DoctypeNameRangeInvalid,
            ),
            (
                TokenizerInvariantKind::DoctypeTailRangeInvalid,
                ParserTokenizerInvariantError::DoctypeTailRangeInvalid,
            ),
            (
                TokenizerInvariantKind::AsciiPrefixCandidateRangeInvalid,
                ParserTokenizerInvariantError::AsciiPrefixCandidateRangeInvalid,
            ),
            (
                TokenizerInvariantKind::CommentStateMissingPendingStart,
                ParserTokenizerInvariantError::CommentStateMissingPendingStart,
            ),
            (
                TokenizerInvariantKind::CommentPendingRangeInvalid,
                ParserTokenizerInvariantError::CommentPendingRangeInvalid,
            ),
            (
                TokenizerInvariantKind::CommentPendingDelimiterOutsideCurrentRange,
                ParserTokenizerInvariantError::CommentPendingDelimiterOutsideCurrentRange,
            ),
            (
                TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState,
                ParserTokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState,
            ),
            (
                TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid,
                ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid,
            ),
            (
                TokenizerInvariantKind::TextModeEndTagAttributePositionInvalid,
                ParserTokenizerInvariantError::TextModeEndTagAttributePositionInvalid,
            ),
            (
                TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid,
                ParserTokenizerInvariantError::TextModeEndTagSolidusPositionInvalid,
            ),
            (
                TokenizerInvariantKind::PendingTextRangeInvalid,
                ParserTokenizerInvariantError::PendingTextRangeInvalid,
            ),
            (
                TokenizerInvariantKind::CdataStateMissingPendingTextStart,
                ParserTokenizerInvariantError::CdataStateMissingPendingTextStart,
            ),
            (
                TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange,
                ParserTokenizerInvariantError::CdataEndDelimiterOutsidePendingTextRange,
            ),
            (
                TokenizerInvariantKind::CdataEndDelimiterDoesNotMatchState,
                ParserTokenizerInvariantError::CdataEndDelimiterDoesNotMatchState,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionStateMissingPendingMetadata,
                ParserTokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionMetadataOutsideState,
                ParserTokenizerInvariantError::ProcessingInstructionMetadataOutsideState,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid,
                ParserTokenizerInvariantError::ProcessingInstructionTargetRangeInvalid,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid,
                ParserTokenizerInvariantError::ProcessingInstructionDataRangeInvalid,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionTargetStartAfterCursor,
                ParserTokenizerInvariantError::ProcessingInstructionTargetStartAfterCursor,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionDataStartAfterCursor,
                ParserTokenizerInvariantError::ProcessingInstructionDataStartAfterCursor,
            ),
        ];

        for (internal, public) in cases {
            assert_eq!(public_tokenizer_invariant(internal), public);
        }
    }

    #[test]
    fn core_v0_attribute_recovery_reports_standard_condition_and_actual_action() {
        use crate::html5::shared::{
            ParseErrorCode, TokenizerExtensionParseErrorCode, WhatwgParseErrorCode,
        };

        let cases = [
            (
                "<div =x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedEqualsSignBeforeAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '=' },
                    5,
                )),
            ),
            (
                "<div \"x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '"' },
                    5,
                )),
            ),
            (
                "<div 'x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '\'' },
                    5,
                )),
            ),
            (
                "<div <x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '<' },
                    5,
                )),
            ),
            (
                "<div `x>",
                "x",
                Some((
                    ParseErrorCode::TokenizerExtension(
                        TokenizerExtensionParseErrorCode::DroppedGraveAccentBeforeAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '`' },
                    5,
                )),
            ),
            (
                "<div ?x>",
                "x",
                Some((
                    ParseErrorCode::TokenizerExtension(
                        TokenizerExtensionParseErrorCode::DroppedQuestionMarkBeforeAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '?' },
                    5,
                )),
            ),
            ("<div a?b>", "a?b", None),
        ];

        for (source, expected_attribute_name, expected) in cases {
            let whole = observe_bytes(ParserObservationInput::Utf8(source));
            let whole_tokens = captured(&whole.tokens).to_vec();
            let whole_errors = captured(&whole.parse_errors).to_vec();
            let [
                ObservedToken::StartTag {
                    name,
                    attributes,
                    self_closing,
                },
                ObservedToken::Eof,
            ] = whole_tokens.as_slice()
            else {
                panic!("expected one start tag and EOF for source={source:?}");
            };
            assert_eq!(name, "div", "source={source:?}");
            assert!(!self_closing, "source={source:?}");
            assert_eq!(attributes.len(), 1, "source={source:?}");
            assert_eq!(
                attributes[0].name, expected_attribute_name,
                "source={source:?}"
            );
            assert_eq!(attributes[0].value, "", "source={source:?}");
            match expected {
                Some((code, recovery, offset)) => {
                    assert_eq!(whole_errors.len(), 1, "source={source:?}");
                    assert_eq!(whole_errors[0].code, code, "source={source:?}");
                    assert_eq!(
                        whole_errors[0].recovery,
                        Some(recovery),
                        "source={source:?}"
                    );
                    assert_eq!(
                        normalized_offset(&whole_errors[0].position),
                        offset,
                        "source={source:?}"
                    );
                }
                None => assert!(whole_errors.is_empty(), "source={source:?}"),
            }
            for split in 1..source.len() {
                let chunks = [&source[..split], &source[split..]];
                let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
                assert_eq!(
                    captured(&chunked.tokens),
                    whole_tokens,
                    "production tokens changed for source={source:?}, split={split}"
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    whole_errors,
                    "diagnostics changed for source={source:?}, split={split}"
                );
            }
        }
    }

    #[test]
    fn tokenizer_error_occurrences_and_positions_match_across_text_chunks() {
        let text = "a\r\n\0<div a='x' a='y'></";
        let whole = observe_bytes(ParserObservationInput::Utf8(text));
        let expected = captured(&whole.parse_errors).to_vec();
        for split in text
            .char_indices()
            .map(|(offset, _)| offset)
            .filter(|offset| *offset > 0)
        {
            let chunks = [&text[..split], &text[split..]];
            let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
            assert_eq!(
                captured(&chunked.parse_errors),
                expected,
                "parse errors differ at text split {split}"
            );
        }
    }
}
