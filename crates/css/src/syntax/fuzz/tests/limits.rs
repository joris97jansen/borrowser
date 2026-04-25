use super::super::{
    CssParserFuzzConfig, CssParserFuzzTermination, CssTokenizerFuzzConfig,
    CssTokenizerFuzzTermination, run_seeded_parser_fuzz_case, run_seeded_tokenizer_fuzz_case,
};

#[test]
fn css_tokenizer_fuzz_harness_rejects_inputs_above_explicit_limit() {
    let summary = run_seeded_tokenizer_fuzz_case(
        b"0123456789",
        CssTokenizerFuzzConfig {
            seed: 0x55,
            max_input_bytes: 4,
            ..CssTokenizerFuzzConfig::default()
        },
    )
    .expect("oversized tokenizer input should be rejected, not crash");
    assert_eq!(
        summary.termination,
        CssTokenizerFuzzTermination::RejectedMaxInputBytes
    );
    assert_eq!(summary.tokens_observed, 0);
}

#[test]
fn css_tokenizer_fuzz_harness_rejects_token_budget_deterministically() {
    let summary = run_seeded_tokenizer_fuzz_case(
        b"a,b,c,d,e,f,g",
        CssTokenizerFuzzConfig {
            seed: 0x77,
            max_tokens_observed: 4,
            ..CssTokenizerFuzzConfig::default()
        },
    )
    .expect("token budget rejection should be deterministic");
    assert_eq!(
        summary.termination,
        CssTokenizerFuzzTermination::RejectedMaxTokensObserved
    );
}

#[test]
fn css_parser_fuzz_harness_rejects_rule_budget_deterministically() {
    let summary = run_seeded_parser_fuzz_case(
        b"a { color: red; } b { color: blue; } c { color: green; }",
        CssParserFuzzConfig {
            max_rules_observed: 2,
            ..CssParserFuzzConfig::default()
        },
    )
    .expect("rule budget rejection should be deterministic");
    assert_eq!(
        summary.termination,
        CssParserFuzzTermination::RejectedMaxRulesObserved
    );
}

#[test]
fn css_parser_fuzz_harness_rejects_declaration_budget_deterministically() {
    let summary = run_seeded_parser_fuzz_case(
        b"a { color: red; width: 1px; height: 2px; }",
        CssParserFuzzConfig {
            max_declarations_observed: 2,
            ..CssParserFuzzConfig::default()
        },
    )
    .expect("declaration budget rejection should be deterministic");
    assert_eq!(
        summary.termination,
        CssParserFuzzTermination::RejectedMaxDeclarationsObserved
    );
}

#[test]
fn css_parser_fuzz_harness_rejects_component_budget_deterministically() {
    let summary = run_seeded_parser_fuzz_case(
        b"a { color: red blue green; }",
        CssParserFuzzConfig {
            max_component_values_observed: 2,
            ..CssParserFuzzConfig::default()
        },
    )
    .expect("component budget rejection should be deterministic");
    assert_eq!(
        summary.termination,
        CssParserFuzzTermination::RejectedMaxComponentValuesObserved
    );
}
