#![no_main]

use css::syntax::{
    CssParserFuzzConfig, SyntaxLimits, derive_css_fuzz_seed, run_seeded_parser_fuzz_case,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_css_fuzz_seed(data);
    let config = CssParserFuzzConfig {
        seed,
        max_input_bytes: 4 * 1024,
        max_decoded_bytes: 16 * 1024,
        max_rules_observed: 256,
        max_declarations_observed: 2_048,
        max_component_values_observed: 8_192,
        max_diagnostics_observed: 64,
        syntax_limits: SyntaxLimits {
            max_stylesheet_input_bytes: 16 * 1024,
            max_declaration_list_input_bytes: 16 * 1024,
            max_lexical_tokens: 16 * 1024,
            max_rules: 256,
            max_selectors_per_rule: 128,
            max_selector_component_values: 1_024,
            max_selector_segments_per_selector: 64,
            max_simple_selectors_per_compound: 64,
            max_declarations_per_rule: 256,
            max_component_values_per_container: 1_024,
            max_component_nesting_depth: 64,
            max_diagnostics: 64,
        },
    };
    if let Err(err) = run_seeded_parser_fuzz_case(data, config) {
        panic!("css parser fuzz invariant failed: {err}");
    }
});
