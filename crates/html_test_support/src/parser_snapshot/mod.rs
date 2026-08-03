macro_rules! define_snapshot_types {
    ($parsed:ident, $canonical:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) struct $parsed(super::SnapshotData);

        impl $parsed {
            fn new(data: super::SnapshotData) -> Self {
                Self(data)
            }

            pub(super) fn data(&self) -> &super::SnapshotData {
                &self.0
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) struct $canonical(super::SnapshotData);

        impl $canonical {
            fn new(data: super::SnapshotData) -> Self {
                Self(data)
            }

            pub(super) fn data(&self) -> &super::SnapshotData {
                &self.0
            }
        }
    };
}

mod document_mode;
mod implementation_diagnostics;
mod lexical;
mod parse_errors;
mod patches;
mod token_v2;
mod transitions;
mod tree;
mod unsupported_features;

use crate::parser_fixture::ExpectationSurface;
use html::conformance::CanonicalParserResult;
pub use lexical::SnapshotReadError;
use lexical::{SnapshotRecord, StoredSnapshotRecord};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SnapshotFormat {
    TokenV2,
    ParseErrorsV1,
    ImplementationDiagnosticsV1,
    DocumentModeV1,
    DomV3,
    DomPatchV3,
    TreeTransitionsV1,
    UnsupportedFeaturesV1,
}

impl SnapshotFormat {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::TokenV2 => "html5-token-v2",
            Self::ParseErrorsV1 => "html5-parse-errors-v1",
            Self::ImplementationDiagnosticsV1 => "html5-implementation-diagnostics-v1",
            Self::DocumentModeV1 => "html5-document-mode-v1",
            Self::DomV3 => "html5-dom-v3",
            Self::DomPatchV3 => "html5-dompatch-v3",
            Self::TreeTransitionsV1 => "html5-tree-transitions-v1",
            Self::UnsupportedFeaturesV1 => "html5-unsupported-features-v1",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SnapshotData {
    bytes: String,
    records: Vec<StoredSnapshotRecord>,
}

impl SnapshotData {
    fn new(bytes: String, records: Vec<SnapshotRecord>) -> Self {
        let mut cursor = bytes.find('\n').map_or(bytes.len(), |index| index + 1);
        let mut stored = Vec::with_capacity(records.len());
        for record in records {
            let end = cursor
                .checked_add(record.line.len())
                .expect("validated snapshot record range");
            debug_assert_eq!(bytes.get(cursor..end), Some(record.line.as_str()));
            stored.push(StoredSnapshotRecord {
                location: record.location,
                line: cursor..end,
            });
            cursor = end
                .checked_add(1)
                .expect("validated snapshot terminal LF range");
        }
        Self {
            bytes,
            records: stored,
        }
    }

    #[cfg(test)]
    pub(crate) fn bytes(&self) -> &str {
        &self.bytes
    }

    pub(crate) fn record_count(&self) -> usize {
        self.records.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub(crate) fn record(&self, index: usize) -> Option<SnapshotRecordRef<'_>> {
        self.records.get(index).map(|record| SnapshotRecordRef {
            location: &record.location,
            line: &self.bytes[record.line.clone()],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SnapshotRecordRef<'a> {
    pub(crate) location: &'a str,
    pub(crate) line: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParsedSnapshot {
    Tokens(token_v2::ParsedTokenSnapshot),
    ParseErrors(parse_errors::ParsedParseErrorsSnapshot),
    ImplementationDiagnostics(implementation_diagnostics::ParsedImplementationDiagnosticsSnapshot),
    DocumentMode(document_mode::ParsedDocumentModeSnapshot),
    Tree(tree::ParsedTreeSnapshot),
    Patches(patches::ParsedPatchesSnapshot),
    Transitions(transitions::ParsedTransitionsSnapshot),
    UnsupportedFeatures(unsupported_features::ParsedUnsupportedFeaturesSnapshot),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CanonicalSnapshot {
    Tokens(token_v2::CanonicalTokenSnapshot),
    ParseErrors(parse_errors::CanonicalParseErrorsSnapshot),
    ImplementationDiagnostics(
        implementation_diagnostics::CanonicalImplementationDiagnosticsSnapshot,
    ),
    DocumentMode(document_mode::CanonicalDocumentModeSnapshot),
    Tree(tree::CanonicalTreeSnapshot),
    Patches(patches::CanonicalPatchesSnapshot),
    Transitions(transitions::CanonicalTransitionsSnapshot),
    UnsupportedFeatures(unsupported_features::CanonicalUnsupportedFeaturesSnapshot),
}

macro_rules! snapshot_accessors {
    ($ty:ty) => {
        impl $ty {
            pub(crate) fn surface(&self) -> ExpectationSurface {
                match self {
                    Self::Tokens(_) => ExpectationSurface::Tokens,
                    Self::ParseErrors(_) => ExpectationSurface::ParseErrors,
                    Self::ImplementationDiagnostics(_) => {
                        ExpectationSurface::ImplementationDiagnostics
                    }
                    Self::DocumentMode(_) => ExpectationSurface::DocumentMode,
                    Self::Tree(_) => ExpectationSurface::Tree,
                    Self::Patches(_) => ExpectationSurface::Patches,
                    Self::Transitions(_) => ExpectationSurface::Transitions,
                    Self::UnsupportedFeatures(_) => ExpectationSurface::UnsupportedFeatures,
                }
            }

            pub(crate) fn format(&self) -> SnapshotFormat {
                match self {
                    Self::Tokens(_) => SnapshotFormat::TokenV2,
                    Self::ParseErrors(_) => SnapshotFormat::ParseErrorsV1,
                    Self::ImplementationDiagnostics(_) => {
                        SnapshotFormat::ImplementationDiagnosticsV1
                    }
                    Self::DocumentMode(_) => SnapshotFormat::DocumentModeV1,
                    Self::Tree(_) => SnapshotFormat::DomV3,
                    Self::Patches(_) => SnapshotFormat::DomPatchV3,
                    Self::Transitions(_) => SnapshotFormat::TreeTransitionsV1,
                    Self::UnsupportedFeatures(_) => SnapshotFormat::UnsupportedFeaturesV1,
                }
            }

            pub(crate) fn snapshot(&self) -> &SnapshotData {
                match self {
                    Self::Tokens(value) => value.data(),
                    Self::ParseErrors(value) => value.data(),
                    Self::ImplementationDiagnostics(value) => value.data(),
                    Self::DocumentMode(value) => value.data(),
                    Self::Tree(value) => value.data(),
                    Self::Patches(value) => value.data(),
                    Self::Transitions(value) => value.data(),
                    Self::UnsupportedFeatures(value) => value.data(),
                }
            }
        }
    };
}

snapshot_accessors!(ParsedSnapshot);
snapshot_accessors!(CanonicalSnapshot);

pub(crate) fn read_snapshot(
    surface: ExpectationSurface,
    bytes: &[u8],
) -> Result<ParsedSnapshot, SnapshotReadError> {
    match surface {
        ExpectationSurface::Tokens => token_v2::read(bytes).map(ParsedSnapshot::Tokens),
        ExpectationSurface::ParseErrors => {
            parse_errors::read(bytes).map(ParsedSnapshot::ParseErrors)
        }
        ExpectationSurface::ImplementationDiagnostics => {
            implementation_diagnostics::read(bytes).map(ParsedSnapshot::ImplementationDiagnostics)
        }
        ExpectationSurface::DocumentMode => {
            document_mode::read(bytes).map(ParsedSnapshot::DocumentMode)
        }
        ExpectationSurface::Tree => tree::read(bytes).map(ParsedSnapshot::Tree),
        ExpectationSurface::Patches => patches::read(bytes).map(ParsedSnapshot::Patches),
        ExpectationSurface::Transitions => {
            transitions::read(bytes).map(ParsedSnapshot::Transitions)
        }
        ExpectationSurface::UnsupportedFeatures => {
            unsupported_features::read(bytes).map(ParsedSnapshot::UnsupportedFeatures)
        }
        ExpectationSurface::FinalInvariants => Err(SnapshotReadError::InvalidHeader),
    }
}

pub(crate) fn serialize_snapshot(
    surface: ExpectationSurface,
    result: &CanonicalParserResult,
) -> Result<CanonicalSnapshot, ()> {
    match surface {
        ExpectationSurface::Tokens => {
            token_v2::write(&result.tokens).map(CanonicalSnapshot::Tokens)
        }
        ExpectationSurface::ParseErrors => {
            parse_errors::write(&result.parse_errors).map(CanonicalSnapshot::ParseErrors)
        }
        ExpectationSurface::ImplementationDiagnostics => {
            implementation_diagnostics::write(&result.implementation_diagnostics)
                .map(CanonicalSnapshot::ImplementationDiagnostics)
        }
        ExpectationSurface::DocumentMode => {
            document_mode::write(&result.document_mode).map(CanonicalSnapshot::DocumentMode)
        }
        ExpectationSurface::Tree => tree::write(&result.tree).map(CanonicalSnapshot::Tree),
        ExpectationSurface::Patches => {
            patches::write(&result.patches).map(CanonicalSnapshot::Patches)
        }
        ExpectationSurface::Transitions => {
            transitions::write(&result.transitions).map(CanonicalSnapshot::Transitions)
        }
        ExpectationSurface::UnsupportedFeatures => {
            unsupported_features::write(&result.unsupported_features)
                .map(CanonicalSnapshot::UnsupportedFeatures)
        }
        ExpectationSurface::FinalInvariants => Err(()),
    }
}

#[cfg(test)]
mod tests;
