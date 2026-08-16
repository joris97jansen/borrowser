use crate::selectors::{SelectorDomBuildError, SelectorMatchability};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectorFuzzError {
    SelectorDomBuild {
        error: SelectorDomBuildError,
    },
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
            Self::SelectorDomBuild { error } => {
                write!(f, "selector fuzz DOM projection failed: {error}")
            }
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

impl std::error::Error for SelectorFuzzError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SelectorDomBuild { error } => Some(error),
            Self::NonDeterministicParseResult { .. }
            | Self::NonDeterministicParseSnapshot { .. }
            | Self::NonDeterministicMatchSnapshot { .. }
            | Self::NonDeterministicMatchOutcome { .. }
            | Self::UnexpectedMatchability { .. }
            | Self::UnsupportedSelectorReachedLimitError { .. } => None,
        }
    }
}

pub(super) fn matchability_label(matchability: SelectorMatchability) -> &'static str {
    match matchability {
        SelectorMatchability::Parsed => "parsed",
        SelectorMatchability::Unsupported => "unsupported",
        SelectorMatchability::Invalid => "invalid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_dom_build_error_remains_the_typed_error_source() {
        let build_error = SelectorDomBuildError::NestedDocument { depth: 2 };
        let fuzz_error = SelectorFuzzError::SelectorDomBuild { error: build_error };

        let source = std::error::Error::source(&fuzz_error).expect("typed build error source");
        assert_eq!(
            source.downcast_ref::<SelectorDomBuildError>(),
            Some(&build_error)
        );
    }

    #[test]
    fn selector_fuzz_errors_without_typed_causes_have_no_source() {
        let leaf_errors = [
            SelectorFuzzError::NonDeterministicParseResult {
                selector_source: "div".to_string(),
            },
            SelectorFuzzError::NonDeterministicParseSnapshot {
                selector_source: "div".to_string(),
            },
            SelectorFuzzError::NonDeterministicMatchSnapshot {
                selector_source: "div".to_string(),
            },
            SelectorFuzzError::NonDeterministicMatchOutcome {
                selector_source: "div".to_string(),
            },
            SelectorFuzzError::UnexpectedMatchability {
                selector_source: "div".to_string(),
                expected: "parsed",
                actual: "invalid",
            },
            SelectorFuzzError::UnsupportedSelectorReachedLimitError {
                selector_source: ":hover".to_string(),
                matchability: "unsupported",
                error: "limit".to_string(),
            },
        ];

        for error in leaf_errors {
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
