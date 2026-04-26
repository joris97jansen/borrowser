#![no_main]

use css::selectors::fuzz::{
    SelectorMatchingFuzzConfig, run_seeded_selector_matching_fuzz_case,
};
use css::selectors::SelectorMatchingLimits;
use css::syntax::{SyntaxLimits, derive_css_fuzz_seed};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_css_fuzz_seed(data);
    let config = SelectorMatchingFuzzConfig {
        seed,
        max_input_bytes: 4 * 1024,
        max_decoded_bytes: 16 * 1024,
        syntax_limits: SyntaxLimits {
            max_stylesheet_input_bytes: 16 * 1024,
            max_declaration_list_input_bytes: 16 * 1024,
            max_lexical_tokens: 16 * 1024,
            max_rules: 128,
            max_selectors_per_rule: 128,
            max_selector_component_values: 1_024,
            max_selector_segments_per_selector: 64,
            max_simple_selectors_per_compound: 64,
            max_component_values_per_container: 1_024,
            max_component_nesting_depth: 64,
            max_diagnostics: 64,
            ..SyntaxLimits::default()
        },
        matching_limits: SelectorMatchingLimits {
            max_axis_steps_per_match: 2_048,
        },
        max_selector_cases: 2,
        max_elements_observed: 128,
        ..SelectorMatchingFuzzConfig::default()
    };
    if let Err(err) = run_seeded_selector_matching_fuzz_case(data, config) {
        panic!("css selector matching fuzz invariant failed: {err}");
    }
});
