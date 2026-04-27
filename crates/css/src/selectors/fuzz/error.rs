use crate::selectors::SelectorMatchability;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectorFuzzError {
    NonDeterministicParseResult {
        selector_source: String,
    },
    NonDeterministicParseSnapshot {
        selector_source: String,
    },
    NonDeterministicMatchSnapshot {
        selector_source: String,
    },
    NonDeterministicMatchOutcome {
        selector_source: String,
    },
    UnexpectedMatchability {
        selector_source: String,
        expected: &'static str,
        actual: &'static str,
    },
    UnsupportedSelectorReachedLimitError {
        selector_source: String,
        matchability: &'static str,
        error: String,
    },
}

impl std::fmt::Display for SelectorFuzzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonDeterministicParseResult { selector_source } => write!(
                f,
                "selector parser produced non-deterministic parse result for {:?}",
                selector_source
            ),
            Self::NonDeterministicParseSnapshot { selector_source } => write!(
                f,
                "selector parser produced non-deterministic debug snapshot for {:?}",
                selector_source
            ),
            Self::NonDeterministicMatchSnapshot { selector_source } => write!(
                f,
                "selector matching produced non-deterministic debug snapshot for {:?}",
                selector_source
            ),
            Self::NonDeterministicMatchOutcome { selector_source } => write!(
                f,
                "selector matching produced non-deterministic structured outcome for {:?}",
                selector_source
            ),
            Self::UnexpectedMatchability {
                selector_source,
                expected,
                actual,
            } => write!(
                f,
                "selector matching for {:?} expected matchability {}, got {}",
                selector_source, expected, actual
            ),
            Self::UnsupportedSelectorReachedLimitError {
                selector_source,
                matchability,
                error,
            } => write!(
                f,
                "selector matching for {} selector {:?} reached unexpected limit error: {}",
                matchability, selector_source, error
            ),
        }
    }
}

impl std::error::Error for SelectorFuzzError {}

pub(super) fn matchability_label(matchability: SelectorMatchability) -> &'static str {
    match matchability {
        SelectorMatchability::Parsed => "parsed",
        SelectorMatchability::Unsupported => "unsupported",
        SelectorMatchability::Invalid => "invalid",
    }
}
