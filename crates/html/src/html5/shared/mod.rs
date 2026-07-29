//! Shared types for the HTML5 parsing path.
//!
//! This module is `pub(crate)`; downstream consumers must import these types via
//! `html::html5::{Token, Span, ParseError, ...}` to preserve API flexibility.

mod atom;
mod context;
mod counters;
mod diagnostics;
mod error;
mod input;
mod observation;
mod observation_model;
mod reservation;
mod span;
mod token;

pub use atom::{AtomError, AtomId, AtomTable};
pub use context::DocumentParseContext;
pub use counters::Counters;
pub(crate) use diagnostics::{LegacyDiagnosticProjection, ParserDiagnosticSink};
#[allow(unused_imports)]
pub use error::{
    EngineInvariantError, ErrorOrigin, ErrorPolicy, Html5SessionError, LegacyParseErrorCode,
    ParseError, ParserFatalError, ParserReservationSite, ParserResourceExhaustion,
};
#[allow(unused_imports)]
pub use input::{ByteStreamDecoder, DecodeResult, Input};
#[cfg(any(test, feature = "parser-conformance"))]
pub(crate) use observation::{
    CapturedSurface, ObservationOccurrenceSequence, ObservationSurface, ParserObservationCapture,
    ParserObservationInvariant, SurfaceCaptureRequest,
};
pub(crate) use observation::{
    NormalizedPositionIndex, ObservationPositionResolution, ObservationPositionSource,
    ParserObservationConfig, ParserObservationRecorder,
};
pub use observation_model::*;
#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
pub use reservation::ParserFailureInjection;
pub(crate) use reservation::ParserReservationController;
pub use span::{Span, TextSpan};
pub use token::{Attribute, AttributeValue, ProcessingInstructionToken, TextValue, Token};
