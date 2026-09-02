//! Parse errors for tokenization/tree-building.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorOrigin {
    Tokenizer,
    TreeBuilder,
}

/// Deliberately lossy compatibility classification used only by the legacy
/// parser-event facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyParseErrorCode {
    UnexpectedNullCharacter,
    UnexpectedEof,
    InvalidCharacterReference,
    ResourceLimit,
    ImplementationGuardrail,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub origin: ErrorOrigin,
    pub code: LegacyParseErrorCode,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineInvariantError;

/// Semantic parser-owned reservation boundary.
///
/// The identity deliberately does not expose the backing collection type.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParserReservationSite {
    KnownTagAtomStorage,
    KnownTagLookupStorage,
    TemplateChildStorage,
    /// Complete semantic patch-history observation retained by the live parser
    /// before caller-controlled transport drains.
    PatchHistoryObservationStorage,
}

/// Failure of an explicitly fallible parser-owned reservation boundary.
///
/// Stable Rust does not expose the allocator-refusal/capacity-overflow
/// distinction of `TryReserveError`, so this is intentionally not named
/// "out of memory".
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParserResourceExhaustion {
    site: ParserReservationSite,
}

impl ParserResourceExhaustion {
    pub const fn site(self) -> ParserReservationSite {
        self.site
    }

    pub(crate) const fn at(site: ParserReservationSite) -> Self {
        Self { site }
    }
}

impl std::fmt::Display for ParserResourceExhaustion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.site {
            ParserReservationSite::KnownTagAtomStorage => {
                formatter.write_str("HTML parser-owned reservation failed at KnownTagAtomStorage")
            }
            ParserReservationSite::KnownTagLookupStorage => {
                formatter.write_str("HTML parser-owned reservation failed at KnownTagLookupStorage")
            }
            ParserReservationSite::TemplateChildStorage => {
                formatter.write_str("HTML parser-owned reservation failed at TemplateChildStorage")
            }
            ParserReservationSite::PatchHistoryObservationStorage => formatter.write_str(
                "HTML parser-owned reservation failed at PatchHistoryObservationStorage",
            ),
        }
    }
}

impl std::error::Error for ParserResourceExhaustion {}

/// Fatal parser execution failure.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParserFatalError {
    EngineInvariant,
    ResourceExhaustion(ParserResourceExhaustion),
}

impl ParserFatalError {
    /// Typed resource identity owned by HTML. Keeping this match beside the
    /// non-exhaustive enum forces future fatal variants to receive an explicit
    /// owner decision before downstream test tooling can classify them.
    pub const fn is_resource_exhaustion(&self) -> bool {
        match self {
            Self::EngineInvariant => false,
            Self::ResourceExhaustion(_) => true,
        }
    }
}

impl std::fmt::Display for ParserFatalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EngineInvariant => formatter.write_str("HTML parser engine invariant violation"),
            Self::ResourceExhaustion(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParserFatalError {}

impl From<EngineInvariantError> for ParserFatalError {
    fn from(_: EngineInvariantError) -> Self {
        Self::EngineInvariant
    }
}

impl From<ParserResourceExhaustion> for ParserFatalError {
    fn from(error: ParserResourceExhaustion) -> Self {
        Self::ResourceExhaustion(error)
    }
}

/// Session error classification for the HTML5 parsing path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Html5SessionError {
    /// Input/decoding failure (not an engine invariant).
    Decode,
    /// Fatal parser execution failure. Live sessions latch this category.
    Fatal(ParserFatalError),
}

impl std::fmt::Display for Html5SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Html5SessionError::Decode => write!(f, "html5 decode error"),
            Html5SessionError::Fatal(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Html5SessionError {}

impl From<ParserFatalError> for Html5SessionError {
    fn from(error: ParserFatalError) -> Self {
        Self::Fatal(error)
    }
}

impl From<EngineInvariantError> for Html5SessionError {
    fn from(error: EngineInvariantError) -> Self {
        Self::Fatal(error.into())
    }
}

#[cfg(test)]
mod fatal_display_tests {
    use super::{ParserFatalError, ParserReservationSite, ParserResourceExhaustion};

    #[test]
    fn fatal_display_uses_static_semantic_text_for_every_current_identity() {
        assert!(!ParserFatalError::EngineInvariant.is_resource_exhaustion());
        assert_eq!(
            ParserFatalError::EngineInvariant.to_string(),
            "HTML parser engine invariant violation"
        );
        for (site, expected) in [
            (
                ParserReservationSite::KnownTagAtomStorage,
                "HTML parser-owned reservation failed at KnownTagAtomStorage",
            ),
            (
                ParserReservationSite::KnownTagLookupStorage,
                "HTML parser-owned reservation failed at KnownTagLookupStorage",
            ),
            (
                ParserReservationSite::TemplateChildStorage,
                "HTML parser-owned reservation failed at TemplateChildStorage",
            ),
            (
                ParserReservationSite::PatchHistoryObservationStorage,
                "HTML parser-owned reservation failed at PatchHistoryObservationStorage",
            ),
        ] {
            assert!(
                ParserFatalError::ResourceExhaustion(ParserResourceExhaustion::at(site))
                    .is_resource_exhaustion()
            );
            assert_eq!(
                ParserFatalError::ResourceExhaustion(ParserResourceExhaustion::at(site))
                    .to_string(),
                expected
            );
        }
    }
}
