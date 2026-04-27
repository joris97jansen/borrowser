use super::config::{
    CssParserFuzzConfig, CssParserFuzzSummary, CssParserFuzzTermination, CssSyntaxFuzzError,
    CssTokenizerFuzzConfig, CssTokenizerFuzzSummary, CssTokenizerFuzzTermination,
};
use super::digest::{mix_bool, mix_usize};
use super::invariants::{ensure_parse_stats_consistent, validate_css_token_stream};
use super::observed_digest::{mix_token, observe_diagnostics};
use super::options::{parser_fuzz_options, tokenizer_fuzz_options};
use super::parser_observer::ParserObserver;
use crate::syntax::{parse_stylesheet_with_options, tokenize_str_with_options};

pub fn run_seeded_tokenizer_fuzz_case(
    bytes: &[u8],
    config: CssTokenizerFuzzConfig,
) -> Result<CssTokenizerFuzzSummary, CssSyntaxFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(CssTokenizerFuzzSummary {
            seed: config.seed,
            termination: CssTokenizerFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            tokens_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let decoded = String::from_utf8_lossy(bytes).into_owned();
    if decoded.len() > config.max_decoded_bytes {
        return Ok(CssTokenizerFuzzSummary {
            seed: config.seed,
            termination: CssTokenizerFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            tokens_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let options = tokenizer_fuzz_options(&config);
    let tokenization = tokenize_str_with_options(&decoded, &options);
    validate_css_token_stream(&tokenization.input, &tokenization.tokens, &options)?;

    let mut digest = mix_usize(0, tokenization.stats.input_bytes);
    digest = mix_usize(digest, tokenization.stats.tokens_emitted);
    digest = mix_usize(digest, tokenization.stats.diagnostics_emitted);
    digest = mix_bool(digest, tokenization.stats.hit_limit);

    let mut tokens_observed = 0usize;
    for token in &tokenization.tokens {
        if tokens_observed >= config.max_tokens_observed {
            return Ok(CssTokenizerFuzzSummary {
                seed: config.seed,
                termination: CssTokenizerFuzzTermination::RejectedMaxTokensObserved,
                input_bytes: bytes.len(),
                decoded_bytes: decoded.len(),
                tokens_observed,
                diagnostics_observed: 0,
                hit_limit: tokenization.stats.hit_limit,
                digest,
            });
        }
        digest = mix_token(digest, &tokenization.input, token, "tokenizer token")?;
        tokens_observed += 1;
    }

    let (diagnostics_observed, digest, termination) = observe_diagnostics(
        &tokenization.input,
        &tokenization.diagnostics,
        config.max_diagnostics_observed,
        digest,
        CssTokenizerFuzzTermination::Completed,
        CssTokenizerFuzzTermination::RejectedMaxDiagnosticsObserved,
        "tokenizer diagnostics",
    )?;

    Ok(CssTokenizerFuzzSummary {
        seed: config.seed,
        termination,
        input_bytes: bytes.len(),
        decoded_bytes: decoded.len(),
        tokens_observed,
        diagnostics_observed,
        hit_limit: tokenization.stats.hit_limit,
        digest,
    })
}

pub fn run_seeded_parser_fuzz_case(
    bytes: &[u8],
    config: CssParserFuzzConfig,
) -> Result<CssParserFuzzSummary, CssSyntaxFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(CssParserFuzzSummary {
            seed: config.seed,
            termination: CssParserFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            rules_observed: 0,
            declarations_observed: 0,
            component_values_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let decoded = String::from_utf8_lossy(bytes).into_owned();
    if decoded.len() > config.max_decoded_bytes {
        return Ok(CssParserFuzzSummary {
            seed: config.seed,
            termination: CssParserFuzzTermination::RejectedMaxDecodedBytes,
            input_bytes: bytes.len(),
            decoded_bytes: decoded.len(),
            rules_observed: 0,
            declarations_observed: 0,
            component_values_observed: 0,
            diagnostics_observed: 0,
            hit_limit: false,
            digest: 0,
        });
    }

    let options = parser_fuzz_options(&config);
    let parse = parse_stylesheet_with_options(&decoded, &options);
    ensure_parse_stats_consistent(&parse, "stylesheet parse")?;

    let mut observer =
        ParserObserver::new(&config, bytes.len(), decoded.len(), parse.stats.hit_limit);
    observer.observe_parse(&parse)?;
    Ok(observer.finish())
}
