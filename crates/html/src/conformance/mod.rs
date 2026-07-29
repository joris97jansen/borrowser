//! Typed, parser-owned semantic models for HTML conformance observations.
//!
//! AE13a defines these passive result shapes without wiring observation hooks
//! into the tokenizer or tree builder. Snapshot serialization and integrated
//! parser capture belong to later AE13 slices.

mod execution;
mod model;

pub use crate::html5::shared::{
    DiagnosticEventMetadata, EventPosition, ImplementationDiagnosticCode,
    ImplementationDiagnosticEvent, InputCoordinateSpace, InputPosition, InputPreprocessingStage,
    NormalizedInputPosition, NormalizedLineNumber, NormalizedScalarColumn, ObservedInsertionMode,
    ObservedToken, ObservedTokenAttribute, ParseErrorCode, ParseErrorEvent, ParserContextSummary,
    ParserGuardrail, ParserGuardrailPayload, ParserRecoveryAction, ParserResourceLimit,
    ParserResourceLimitPayload, ParserStage, ParserTokenKind, PositionUnavailableReason,
    SourceBytePosition, SourcePositionUnavailableReason, TokenizerExtensionParseErrorCode,
    TreeConstructionImplementationDiagnosticCode, TreeConstructionParseErrorCode,
    Utf8ReplacementPayload, Utf8ReplacementReason, WhatwgParseErrorCode,
};
pub use execution::{
    ObservationRequest, ParserObservationExecutionError, ParserObservationInput,
    ParserObservationInvariantError, ParserObservationRequest, ParserObservationTarget,
    ParserTokenizerInvariantError, ScalarObservationRequest, execute_parser_observation,
};
pub use model::*;
