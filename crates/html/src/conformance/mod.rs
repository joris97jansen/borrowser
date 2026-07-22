//! Typed, parser-owned semantic models for HTML conformance observations.
//!
//! AE13a defines these passive result shapes without wiring observation hooks
//! into the tokenizer or tree builder. Snapshot serialization and integrated
//! parser capture belong to later AE13 slices.

mod model;

pub use model::*;
