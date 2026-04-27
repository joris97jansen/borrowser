use super::config::{
    SelectorParserFuzzConfig, SelectorParserFuzzSummary, SelectorParserFuzzTermination,
};
use super::error::SelectorFuzzError;
use super::source::parse_selector_source;
use crate::fuzz_support::{
    decode_bytes_lossy_unbounded, digest_snapshot, mix_str, mix_u64, mix_usize,
    synthesize_selector_source,
};
use crate::selectors::{InvalidSelectorReason, SelectorMatchability};
use crate::syntax::SyntaxLimits;

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
            digest_snapshot(config.seed, std::slice::from_ref(&observation.snapshot)),
        );
    }

    summary.digest = digest;
    Ok(summary)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SelectorParseObservation {
    pub(super) matchability: SelectorMatchability,
    pub(super) invalid_reason: Option<InvalidSelectorReason>,
    pub(super) snapshot: String,
}

pub(super) fn observe_selector_parse_case(
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
