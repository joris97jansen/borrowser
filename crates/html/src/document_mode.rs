//! Parser-selected HTML document mode.
//!
//! This type is owned by the HTML subsystem rather than the tree builder so
//! parser output, conformance observations, and tree-construction state share
//! one semantic representation.

/// The document mode selected by HTML tree construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DocumentMode {
    #[default]
    NoQuirks,
    LimitedQuirks,
    Quirks,
}

impl DocumentMode {
    pub const fn as_fixture_str(self) -> &'static str {
        match self {
            Self::NoQuirks => "no-quirks",
            Self::LimitedQuirks => "limited-quirks",
            Self::Quirks => "quirks",
        }
    }
}

impl std::fmt::Display for DocumentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_fixture_str())
    }
}

impl std::str::FromStr for DocumentMode {
    type Err = ParseDocumentModeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "no-quirks" => Ok(Self::NoQuirks),
            "limited-quirks" => Ok(Self::LimitedQuirks),
            "quirks" => Ok(Self::Quirks),
            _ => Err(ParseDocumentModeError),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseDocumentModeError;

impl std::fmt::Display for ParseDocumentModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("expected no-quirks, limited-quirks, or quirks")
    }
}

impl std::error::Error for ParseDocumentModeError {}

#[cfg(test)]
mod tests {
    use super::DocumentMode;

    #[test]
    fn fixture_names_round_trip_the_single_shared_document_mode_type() {
        for mode in [
            DocumentMode::NoQuirks,
            DocumentMode::LimitedQuirks,
            DocumentMode::Quirks,
        ] {
            assert_eq!(mode.as_fixture_str().parse::<DocumentMode>(), Ok(mode));
        }
        assert!("unresolved".parse::<DocumentMode>().is_err());
    }
}
