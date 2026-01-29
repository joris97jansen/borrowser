//! Document-level parse context (shared resources).

use super::{AtomTable, Counters, ParseError};

/// Document-level parse context shared by tokenizer and tree builder.
///
/// Owns document-lifetime resources such as atom tables, policies, and metrics.
#[derive(Debug, Default)]
pub struct DocumentParseContext {
    pub atoms: AtomTable,
    pub counters: Counters,
    pub errors: Vec<ParseError>,
}

impl DocumentParseContext {
    pub fn new() -> Self {
        Self::default()
    }
}
