use super::super::{
    CssParserFuzzConfig, CssParserFuzzTermination, CssTokenizerFuzzConfig,
    CssTokenizerFuzzTermination, derive_css_fuzz_seed, run_seeded_parser_fuzz_case,
    run_seeded_tokenizer_fuzz_case,
};

#[test]
fn css_fuzz_seed_is_stable_for_same_bytes() {
    let bytes = b"body { color: red; }\xF0\x9F\x98\x80";
    assert_eq!(derive_css_fuzz_seed(bytes), derive_css_fuzz_seed(bytes));
}

#[test]
fn seeded_css_tokenizer_fuzz_harness_is_reproducible() {
    let bytes = b"div, #hero { color: red; width: calc(100% - 2px); }";
    let config = CssTokenizerFuzzConfig {
        seed: 0x4242,
        ..CssTokenizerFuzzConfig::default()
    };
    let first =
        run_seeded_tokenizer_fuzz_case(bytes, config).expect("first tokenizer run should pass");
    let second =
        run_seeded_tokenizer_fuzz_case(bytes, config).expect("second tokenizer run should pass");
    assert_eq!(first, second);
    assert_eq!(first.termination, CssTokenizerFuzzTermination::Completed);
    assert!(first.tokens_observed > 0);
}

#[test]
fn seeded_css_tokenizer_fuzz_harness_handles_invalid_utf8() {
    let bytes = [0xFFu8, b'{', b'c', b':', 0xC3, b';', b'}'];
    let summary = run_seeded_tokenizer_fuzz_case(
        &bytes,
        CssTokenizerFuzzConfig {
            seed: 0x99,
            ..CssTokenizerFuzzConfig::default()
        },
    )
    .expect("invalid utf-8 tokenizer input should remain recoverable");
    assert_eq!(summary.termination, CssTokenizerFuzzTermination::Completed);
    assert!(summary.decoded_bytes >= 3);
}

#[test]
fn seeded_css_parser_fuzz_harness_is_reproducible() {
    let bytes = b"@media screen { .hero { color: red; } } div { width: 10px; }";
    let config = CssParserFuzzConfig {
        seed: 0x1234,
        ..CssParserFuzzConfig::default()
    };
    let first =
        run_seeded_parser_fuzz_case(bytes, config.clone()).expect("first parser run should pass");
    let second = run_seeded_parser_fuzz_case(bytes, config).expect("second parser run should pass");
    assert_eq!(first, second);
    assert_eq!(first.termination, CssParserFuzzTermination::Completed);
    assert!(first.rules_observed > 0);
}

#[test]
fn seeded_css_parser_fuzz_harness_handles_malformed_input() {
    let bytes = b"div { color: red; broken( ; @media { width: calc(1px; }";
    let summary = run_seeded_parser_fuzz_case(
        bytes,
        CssParserFuzzConfig {
            seed: derive_css_fuzz_seed(bytes),
            ..CssParserFuzzConfig::default()
        },
    )
    .expect("malformed parser input should remain recoverable");
    assert_eq!(summary.termination, CssParserFuzzTermination::Completed);
    assert!(summary.component_values_observed > 0 || summary.diagnostics_observed > 0);
}
