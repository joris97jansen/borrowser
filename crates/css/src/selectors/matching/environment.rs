use html::DocumentMode;

/// Immutable document-level semantic inputs for selector evaluation.
///
/// HTML owns selection of [`DocumentMode`]. CSS owns how that parser metadata
/// affects selector matching. Callers may transport and retain this value, but
/// must not reconstruct it from the DOM or substitute a default when parser
/// metadata is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SelectorMatchingEnvironment {
    document_mode: DocumentMode,
}

impl SelectorMatchingEnvironment {
    pub const fn new(document_mode: DocumentMode) -> Self {
        Self { document_mode }
    }

    pub const fn document_mode(self) -> DocumentMode {
        self.document_mode
    }
}
