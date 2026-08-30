//! Shared types for the HTML5 parsing path.
//!
//! This module is `pub(crate)`; downstream consumers must import these types via
//! `html::html5::{Token, Span, ParseError, ...}` to preserve API flexibility.

mod atom;
mod context;
mod counters;
mod error;
mod event_sink;
mod input;
mod observation;
mod observation_model;
mod reservation;
mod semantic_completeness;
mod span;
mod token;

pub use atom::{AtomError, AtomId, AtomTable};
pub use context::DocumentParseContext;
pub use counters::Counters;
#[allow(unused_imports)]
pub use error::{
    EngineInvariantError, ErrorOrigin, ErrorPolicy, Html5SessionError, LegacyParseErrorCode,
    ParseError, ParserFatalError, ParserReservationSite, ParserResourceExhaustion,
};
pub(crate) use event_sink::{LegacyDiagnosticProjection, ParserEventSink};
#[allow(unused_imports)]
pub use input::{ByteStreamDecoder, DecodeResult, Input};
#[cfg(any(test, feature = "parser-conformance"))]
pub(crate) use observation::{
    CapturedSurface, ObservationOccurrenceSequence, ObservationSurface, ParserObservationCapture,
    ParserObservationCaptureFailure, ParserObservationFailure, ParserObservationInvariant,
    SurfaceCaptureRequest,
};
pub(crate) use observation::{
    NormalizedPositionIndex, ObservationPositionResolution, ObservationPositionSource,
    ParserObservationConfig, ParserObservationRecorder, UnsupportedFeatureObservationFailure,
};
pub use observation_model::*;
#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
pub use reservation::ParserFailureInjection;
pub(crate) use reservation::ParserReservationController;
pub use semantic_completeness::{
    HtmlParseSemanticCompleteness, HtmlParseSemanticDegradationReason,
    HtmlParseSemanticDegradations,
};
pub(crate) use semantic_completeness::{
    HtmlParseSemanticCompletenessTracker, guardrail_degradation, resource_limit_degradation,
};
pub use span::{Span, TextSpan};
pub use token::{Attribute, AttributeValue, ProcessingInstructionToken, TextValue, Token};
