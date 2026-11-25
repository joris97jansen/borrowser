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
mod runtime_diagnostics;
#[cfg(any(test, feature = "internal-api"))]
pub use runtime_diagnostics::DiagnosticCaptureStatus;
pub(crate) use runtime_diagnostics::ParseErrorPositionSource;
#[cfg(any(test, feature = "internal-api"))]
pub use runtime_diagnostics::ParserInputCompletion;
pub use runtime_diagnostics::{DiagnosticCaptureFailure, HtmlParserDiagnosticReport};
mod span;
mod token;

pub use atom::{AtomError, AtomId, AtomTable};
pub use context::DocumentParseContext;
#[cfg(any(test, feature = "internal-api"))]
pub use counters::HtmlDecodeReport;
pub use counters::{Counters, Utf8DecodeCounts};
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
pub use span::{Span, TextSpan};
pub use token::{Attribute, AttributeValue, ProcessingInstructionToken, TextValue, Token};

#[cfg(test)]
pub(crate) use runtime_diagnostics::{
    PATH_WORK, record_decoder_path_for_test, record_token_path_for_test,
};

#[cfg(test)]
thread_local! {pub(crate) static FAIL_AFTER_DECODER_EOF: std::cell::Cell<bool> = const {std::cell::Cell::new(false)};}

#[cfg(test)]
thread_local! {pub(crate) static NEXT_CAPTURE_FAILURE: std::cell::Cell<(bool,bool,Option<usize>)> = const {std::cell::Cell::new((false,false,None))};}

#[cfg(test)]
pub(crate) use runtime_diagnostics::LAST_WORK;
