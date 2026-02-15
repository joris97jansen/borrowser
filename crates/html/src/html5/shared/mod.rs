//! Shared types for the HTML5 parsing path.
//!
//! This module is `pub(crate)`; downstream consumers must import these types via
//! `html::html5::{Token, Span, ParseError, ...}` to preserve API flexibility.

mod atom;
mod context;
mod counters;
mod error;
mod input;
mod span;
mod token;

pub use atom::{AtomError, AtomId, AtomTable};
pub use context::DocumentParseContext;
pub use counters::Counters;
#[allow(unused_imports)]
pub use error::{
    EngineInvariantError, ErrorOrigin, ErrorPolicy, Html5SessionError, ParseError, ParseErrorCode,
};
#[allow(unused_imports)]
pub use input::{ByteStreamDecoder, DecodeResult, Input};
pub use span::{Span, TextSpan};
pub use token::{Attribute, AttributeValue, TextValue, Token};
