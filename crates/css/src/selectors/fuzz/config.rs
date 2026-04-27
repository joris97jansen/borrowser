use crate::fuzz_support::DomFuzzLimits;
use crate::selectors::SelectorMatchingLimits;
use crate::syntax::SyntaxLimits;

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
