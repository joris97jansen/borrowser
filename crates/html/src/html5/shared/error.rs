//! Parse errors for tokenization/tree-building.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorOrigin {
    Tokenizer,
    TreeBuilder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseErrorCode {
    UnexpectedNullCharacter,
    UnexpectedEof,
    InvalidCharacterReference,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub origin: ErrorOrigin,
    pub code: ParseErrorCode,
    /// Byte offset into the decoded Input buffer.
    pub position: usize,
    /// Optional detail for diagnostics (debug-only usage recommended).
    pub detail: Option<&'static str>,
    /// Optional small auxiliary payload (e.g., offending byte/codepoint).
    pub aux: Option<u32>,
}

/// Error tracking policy.
#[derive(Clone, Copy, Debug)]
pub struct ErrorPolicy {
    /// Whether to track and store parse errors.
    pub track: bool,
    /// Maximum number of stored errors (oldest dropped first).
    pub max_stored: usize,
    /// Store errors only in debug builds.
    pub debug_only: bool,
    /// Always increment counters even if storage is disabled.
    pub track_counters: bool,
}

impl Default for ErrorPolicy {
    fn default() -> Self {
        Self {
            track: true,
            max_stored: 128,
            debug_only: true,
            track_counters: true,
        }
    }
}

/// Engine invariant violation (bug/corruption), not a recoverable HTML error.
#[derive(Debug)]
pub struct EngineInvariantError;

/// Session error classification for the HTML5 parsing path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Html5SessionError {
    /// Input/decoding failure (not an engine invariant).
    Decode,
    /// Engine invariant violation (bug/corruption).
    Invariant,
}
