use crate::fuzz_support::{
    DomFuzzLimits, decode_bytes_lossy_unbounded, digest_snapshot, mix_str, mix_u64, mix_usize,
    synthesize_dom_from_bytes, synthesize_selector_source,
};
use crate::selectors::{
    InvalidSelectorList, InvalidSelectorReason, SelectorDomIndex, SelectorListMatchOutcome,
    SelectorListParseResult, SelectorMatchability, SelectorMatchingContext, SelectorMatchingLimits,
    parse_selector_list_with_limits,
};
use crate::syntax::{CssRule, ParseOptions, SyntaxLimits, parse_stylesheet_with_options};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorParserFuzzConfig {
    pub seed: u64,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub syntax_limits: SyntaxLimits,
    pub max_selector_cases: usize,
}

impl Default for SelectorParserFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x53_45_4c_50_41_52_46_5a,
            max_input_bytes: 64 * 1024,
            max_decoded_bytes: 256 * 1024,
            syntax_limits: SyntaxLimits::default(),
            max_selector_cases: 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorParserFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
    RejectedMaxSelectorCases,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectorParserFuzzSummary {
    pub seed: u64,
    pub termination: SelectorParserFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub selector_cases_observed: usize,
    pub parsed_cases: usize,
    pub unsupported_cases: usize,
    pub invalid_cases: usize,
    pub resource_limit_invalid_cases: usize,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorMatchingFuzzConfig {
    pub seed: u64,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub syntax_limits: SyntaxLimits,
    pub matching_limits: SelectorMatchingLimits,
    pub dom_limits: DomFuzzLimits,
    pub max_selector_cases: usize,
    pub max_elements_observed: usize,
}

impl Default for SelectorMatchingFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x53_45_4c_4d_41_54_46_5a,
            max_input_bytes: 64 * 1024,
            max_decoded_bytes: 256 * 1024,
            syntax_limits: SyntaxLimits::default(),
            matching_limits: SelectorMatchingLimits::default(),
            dom_limits: DomFuzzLimits::default(),
            max_selector_cases: 2,
            max_elements_observed: 128,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorMatchingFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
    RejectedMaxSelectorCases,
    RejectedMaxElementsObserved,
    SelectorMatchingLimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectorMatchingFuzzSummary {
    pub seed: u64,
    pub termination: SelectorMatchingFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub selector_cases_observed: usize,
    pub elements_observed: usize,
    pub parsed_cases: usize,
    pub unsupported_cases: usize,
    pub invalid_cases: usize,
    pub matched_targets_observed: usize,
    pub unmatched_targets_observed: usize,
    pub unsupported_targets_observed: usize,
    pub invalid_targets_observed: usize,
    pub limit_errors_observed: usize,
    pub digest: u64,
}

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

pub fn run_seeded_selector_parser_fuzz_case(
    bytes: &[u8],
    config: SelectorParserFuzzConfig,
) -> Result<SelectorParserFuzzSummary, SelectorFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(SelectorParserFuzzSummary {
            seed: config.seed,
            termination: SelectorParserFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            selector_cases_observed: 0,
            parsed_cases: 0,
            unsupported_cases: 0,
            invalid_cases: 0,
            resource_limit_invalid_cases: 0,
            digest: 0,
        });
    }

    let decoded = decode_bytes_lossy_unbounded(bytes);
    if decoded.len() > config.max_decoded_bytes {
        return Ok(SelectorParserFuzzSummary {
            seed: config.seed,
            termination: SelectorParserFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            selector_cases_observed: 0,
            parsed_cases: 0,
            unsupported_cases: 0,
            invalid_cases: 0,
            resource_limit_invalid_cases: 0,
            digest: 0,
        });
    }

    let selector_cases = vec![decoded.clone(), synthesize_selector_source(bytes)];
    let mut digest = mix_u64(0, config.seed);
    digest = mix_usize(digest, bytes.len());
    digest = mix_usize(digest, decoded.len());

    let mut summary = SelectorParserFuzzSummary {
        seed: config.seed,
        termination: SelectorParserFuzzTermination::Completed,
        input_bytes: bytes.len(),
        decoded_bytes: decoded.len(),
        selector_cases_observed: 0,
        parsed_cases: 0,
        unsupported_cases: 0,
        invalid_cases: 0,
        resource_limit_invalid_cases: 0,
        digest: 0,
    };

    for selector_source in selector_cases {
        if summary.selector_cases_observed >= config.max_selector_cases {
            summary.termination = SelectorParserFuzzTermination::RejectedMaxSelectorCases;
            summary.digest = digest;
            return Ok(summary);
        }
        digest = mix_str(digest, &selector_source);
        let observation = observe_selector_parse_case(&selector_source, &config.syntax_limits)?;
        accumulate_parser_observation(&mut summary, &observation);
        digest = mix_u64(
            digest,
            digest_snapshot(config.seed, &[observation.snapshot]),
        );
    }

    summary.digest = digest;
    Ok(summary)
}

pub fn run_seeded_selector_matching_fuzz_case(
    bytes: &[u8],
    config: SelectorMatchingFuzzConfig,
) -> Result<SelectorMatchingFuzzSummary, SelectorFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(SelectorMatchingFuzzSummary {
            seed: config.seed,
            termination: SelectorMatchingFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            selector_cases_observed: 0,
            elements_observed: 0,
            parsed_cases: 0,
            unsupported_cases: 0,
            invalid_cases: 0,
            matched_targets_observed: 0,
            unmatched_targets_observed: 0,
            unsupported_targets_observed: 0,
            invalid_targets_observed: 0,
            limit_errors_observed: 0,
            digest: 0,
        });
    }

    let decoded = decode_bytes_lossy_unbounded(bytes);
    if decoded.len() > config.max_decoded_bytes {
        return Ok(SelectorMatchingFuzzSummary {
            seed: config.seed,
            termination: SelectorMatchingFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            selector_cases_observed: 0,
            elements_observed: 0,
            parsed_cases: 0,
            unsupported_cases: 0,
            invalid_cases: 0,
            matched_targets_observed: 0,
            unmatched_targets_observed: 0,
            unsupported_targets_observed: 0,
            invalid_targets_observed: 0,
            limit_errors_observed: 0,
            digest: 0,
        });
    }

    let (dom, dom_summary) = synthesize_dom_from_bytes(bytes, &config.dom_limits);
    let index = SelectorDomIndex::from_root(&dom);
    if index.len() > config.max_elements_observed {
        return Ok(SelectorMatchingFuzzSummary {
            seed: config.seed,
            termination: SelectorMatchingFuzzTermination::RejectedMaxElementsObserved,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            selector_cases_observed: 0,
            elements_observed: index.len(),
            parsed_cases: 0,
            unsupported_cases: 0,
            invalid_cases: 0,
            matched_targets_observed: 0,
            unmatched_targets_observed: 0,
            unsupported_targets_observed: 0,
            invalid_targets_observed: 0,
            limit_errors_observed: 0,
            digest: mix_usize(mix_u64(0, config.seed), dom_summary.element_count),
        });
    }

    let selector_cases = vec![decoded.clone(), synthesize_selector_source(bytes)];
    let mut digest = mix_u64(0, config.seed);
    digest = mix_usize(digest, bytes.len());
    digest = mix_usize(digest, decoded.len());
    digest = mix_usize(digest, dom_summary.element_count);
    digest = mix_usize(digest, dom_summary.inline_style_attributes);

    let mut summary = SelectorMatchingFuzzSummary {
        seed: config.seed,
        termination: SelectorMatchingFuzzTermination::Completed,
        input_bytes: bytes.len(),
        decoded_bytes: decoded.len(),
        selector_cases_observed: 0,
        elements_observed: index.len(),
        parsed_cases: 0,
        unsupported_cases: 0,
        invalid_cases: 0,
        matched_targets_observed: 0,
        unmatched_targets_observed: 0,
        unsupported_targets_observed: 0,
        invalid_targets_observed: 0,
        limit_errors_observed: 0,
        digest: 0,
    };

    for selector_source in selector_cases {
        if summary.selector_cases_observed >= config.max_selector_cases {
            summary.termination = SelectorMatchingFuzzTermination::RejectedMaxSelectorCases;
            summary.digest = digest;
            return Ok(summary);
        }
        digest = mix_str(digest, &selector_source);
        let observation = observe_selector_matching_case(
            &index,
            &selector_source,
            &config.syntax_limits,
            config.matching_limits,
        )?;
        accumulate_matching_observation(&mut summary, &observation);
        digest = mix_u64(
            digest,
            digest_snapshot(config.seed, &[observation.snapshot]),
        );
        if let Some(limit_error) = &observation.limit_error {
            digest = mix_str(digest, limit_error);
        }
    }

    if summary.limit_errors_observed > 0 {
        summary.termination = SelectorMatchingFuzzTermination::SelectorMatchingLimitExceeded;
    }
    summary.digest = digest;
    Ok(summary)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectorParseObservation {
    matchability: SelectorMatchability,
    invalid_reason: Option<InvalidSelectorReason>,
    snapshot: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectorMatchingObservation {
    parse: SelectorParseObservation,
    matched_targets: usize,
    unmatched_targets: usize,
    unsupported_targets: usize,
    invalid_targets: usize,
    limit_error: Option<String>,
    snapshot: String,
}

fn observe_selector_parse_case(
    selector_source: &str,
    limits: &SyntaxLimits,
) -> Result<SelectorParseObservation, SelectorFuzzError> {
    let first = parse_selector_source(selector_source, limits);
    let second = parse_selector_source(selector_source, limits);
    if first.matchability() != second.matchability()
        || first.invalid().map(|invalid| invalid.reason())
            != second.invalid().map(|invalid| invalid.reason())
        || first.parsed().map_or(0, |list| list.len())
            != second.parsed().map_or(0, |list| list.len())
    {
        return Err(SelectorFuzzError::NonDeterministicParseResult {
            selector_source: selector_source.to_string(),
        });
    }

    let first_snapshot = first.to_debug_snapshot();
    let second_snapshot = second.to_debug_snapshot();
    if first_snapshot != second_snapshot {
        return Err(SelectorFuzzError::NonDeterministicParseSnapshot {
            selector_source: selector_source.to_string(),
        });
    }

    Ok(SelectorParseObservation {
        matchability: first.matchability(),
        invalid_reason: first.invalid().map(|invalid| invalid.reason()),
        snapshot: first_snapshot,
    })
}

fn observe_selector_matching_case(
    index: &SelectorDomIndex<'_>,
    selector_source: &str,
    limits: &SyntaxLimits,
    matching_limits: SelectorMatchingLimits,
) -> Result<SelectorMatchingObservation, SelectorFuzzError> {
    let parse = observe_selector_parse_case(selector_source, limits)?;
    let first = evaluate_selector_matching(index, selector_source, limits, matching_limits)?;
    let second = evaluate_selector_matching(index, selector_source, limits, matching_limits)?;
    if first != second {
        return Err(SelectorFuzzError::NonDeterministicMatchOutcome {
            selector_source: selector_source.to_string(),
        });
    }

    let first_snapshot = index.to_matching_debug_snapshot_with_limits(
        &parse_selector_source(selector_source, limits),
        matching_limits,
    );
    let second_snapshot = index.to_matching_debug_snapshot_with_limits(
        &parse_selector_source(selector_source, limits),
        matching_limits,
    );
    if first_snapshot != second_snapshot {
        return Err(SelectorFuzzError::NonDeterministicMatchSnapshot {
            selector_source: selector_source.to_string(),
        });
    }

    Ok(SelectorMatchingObservation {
        parse,
        matched_targets: first.matched_targets,
        unmatched_targets: first.unmatched_targets,
        unsupported_targets: first.unsupported_targets,
        invalid_targets: first.invalid_targets,
        limit_error: first.limit_error,
        snapshot: first_snapshot,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StructuredMatchingOutcome {
    matched_targets: usize,
    unmatched_targets: usize,
    unsupported_targets: usize,
    invalid_targets: usize,
    limit_error: Option<String>,
}

fn evaluate_selector_matching(
    index: &SelectorDomIndex<'_>,
    selector_source: &str,
    limits: &SyntaxLimits,
    matching_limits: SelectorMatchingLimits,
) -> Result<StructuredMatchingOutcome, SelectorFuzzError> {
    let selectors = parse_selector_source(selector_source, limits);
    let expected_matchability = selectors.matchability();
    let context = SelectorMatchingContext::with_limits(index, matching_limits);

    let mut outcome = StructuredMatchingOutcome {
        matched_targets: 0,
        unmatched_targets: 0,
        unsupported_targets: 0,
        invalid_targets: 0,
        limit_error: None,
    };

    for element in index.elements() {
        match context.match_selector_list(element, &selectors) {
            Ok(match_outcome) => {
                ensure_matchability_consistency(
                    selector_source,
                    expected_matchability,
                    &match_outcome,
                )?;
                match match_outcome.matchability() {
                    SelectorMatchability::Parsed => {
                        if match_outcome.matched_any() {
                            outcome.matched_targets += 1;
                        } else {
                            outcome.unmatched_targets += 1;
                        }
                    }
                    SelectorMatchability::Unsupported => outcome.unsupported_targets += 1,
                    SelectorMatchability::Invalid => outcome.invalid_targets += 1,
                }
            }
            Err(error) => {
                if expected_matchability != SelectorMatchability::Parsed {
                    return Err(SelectorFuzzError::UnsupportedSelectorReachedLimitError {
                        selector_source: selector_source.to_string(),
                        matchability: matchability_label(expected_matchability),
                        error: error.to_string(),
                    });
                }
                if outcome.limit_error.is_none() {
                    outcome.limit_error = Some(error.to_string());
                }
            }
        }
    }

    Ok(outcome)
}

fn ensure_matchability_consistency(
    selector_source: &str,
    expected: SelectorMatchability,
    outcome: &SelectorListMatchOutcome,
) -> Result<(), SelectorFuzzError> {
    if outcome.matchability() != expected {
        return Err(SelectorFuzzError::UnexpectedMatchability {
            selector_source: selector_source.to_string(),
            expected: matchability_label(expected),
            actual: matchability_label(outcome.matchability()),
        });
    }
    Ok(())
}

fn matchability_label(matchability: SelectorMatchability) -> &'static str {
    match matchability {
        SelectorMatchability::Parsed => "parsed",
        SelectorMatchability::Unsupported => "unsupported",
        SelectorMatchability::Invalid => "invalid",
    }
}

fn accumulate_parser_observation(
    summary: &mut SelectorParserFuzzSummary,
    observation: &SelectorParseObservation,
) {
    summary.selector_cases_observed += 1;
    match observation.matchability {
        SelectorMatchability::Parsed => summary.parsed_cases += 1,
        SelectorMatchability::Unsupported => summary.unsupported_cases += 1,
        SelectorMatchability::Invalid => {
            summary.invalid_cases += 1;
            if observation.invalid_reason == Some(InvalidSelectorReason::ResourceLimitExceeded) {
                summary.resource_limit_invalid_cases += 1;
            }
        }
    }
}

fn accumulate_matching_observation(
    summary: &mut SelectorMatchingFuzzSummary,
    observation: &SelectorMatchingObservation,
) {
    summary.selector_cases_observed += 1;
    match observation.parse.matchability {
        SelectorMatchability::Parsed => summary.parsed_cases += 1,
        SelectorMatchability::Unsupported => summary.unsupported_cases += 1,
        SelectorMatchability::Invalid => summary.invalid_cases += 1,
    }
    summary.matched_targets_observed += observation.matched_targets;
    summary.unmatched_targets_observed += observation.unmatched_targets;
    summary.unsupported_targets_observed += observation.unsupported_targets;
    summary.invalid_targets_observed += observation.invalid_targets;
    if observation.limit_error.is_some() {
        summary.limit_errors_observed += 1;
    }
}

fn parse_selector_source(source: &str, limits: &SyntaxLimits) -> SelectorListParseResult {
    let stylesheet_source = format!("{source} {{ color: red; }}");
    let parse_options = ParseOptions {
        limits: limits.clone(),
        ..ParseOptions::stylesheet()
    };
    let parse = parse_stylesheet_with_options(&stylesheet_source, &parse_options);
    let Some(rule) = parse.stylesheet.rules.first() else {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            None,
            InvalidSelectorReason::EmptySelectorList,
        ));
    };
    let CssRule::Qualified(rule) = rule else {
        return SelectorListParseResult::Invalid(InvalidSelectorList::new(
            None,
            InvalidSelectorReason::EmptySelectorList,
        ));
    };
    parse_selector_list_with_limits(&parse.input, &rule.prelude, limits)
}

#[cfg(test)]
mod tests;
