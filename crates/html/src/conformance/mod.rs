//! Typed, parser-owned semantic models for HTML conformance observations.
//!
//! Diagnostic capture is owned by production parser context, complete semantic
//! patch history by the parser-session adapter, and final tree projection by
//! conformance execution after successful materialization. Snapshot
//! serialization remains outside this module.

mod execution;
mod model;
mod projection;

pub use crate::html5::shared::{
    DiagnosticEventMetadata, EventPosition, ImplementationDiagnosticCode,
    ImplementationDiagnosticEvent, InputCoordinateSpace, InputPosition, InputPreprocessingStage,
    NormalizedInputPosition, NormalizedLineNumber, NormalizedScalarColumn, ObservedInsertionMode,
    ObservedToken, ObservedTokenAttribute, ParseErrorCode, ParseErrorEvent, ParserContextSummary,
    ParserGuardrail, ParserGuardrailPayload, ParserRecoveryAction, ParserResourceLimit,
    ParserResourceLimitPayload, ParserStage, ParserTokenKind, PositionUnavailableReason,
    SourceBytePosition, SourcePositionUnavailableReason, TokenizerExtensionParseErrorCode,
    TransitionTokenSummary, TreeConstructionImplementationDiagnosticCode,
    TreeConstructionParseErrorCode, TreeConstructionUnsupportedFeature, TreeDispatchPath,
    TreeTransitionEvent, UnsupportedFeatureEvent, Utf8ReplacementPayload, Utf8ReplacementReason,
    WhatwgParseErrorCode,
};
pub use execution::{
    ObservationRequest, ObservationReservationSite, ObservationResourceExhaustion,
    ParserObservationExecutionError, ParserObservationInput, ParserObservationInvariantError,
    ParserObservationRequest, ParserObservationTarget, ParserTokenizerInvariantError,
    ScalarObservationRequest, UnsupportedFeatureObservationInvariantError,
    execute_parser_observation,
};
pub use model::*;
