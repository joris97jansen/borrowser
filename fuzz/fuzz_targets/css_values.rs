#![no_main]

use css::computed::fuzz::{CssValueFuzzConfig, run_seeded_value_fuzz_case};
use css::specified::SpecifiedValueLimits;
use css::syntax::{SyntaxLimits, derive_css_fuzz_seed};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_css_fuzz_seed(data);
    let config = CssValueFuzzConfig {
        seed,
        max_input_bytes: 4 * 1024,
        max_decoded_bytes: 16 * 1024,
        max_property_cases: 8,
        max_value_cases_per_property: 3,
        syntax_limits: SyntaxLimits {
            max_stylesheet_input_bytes: 16 * 1024,
            max_declaration_list_input_bytes: 16 * 1024,
            max_lexical_tokens: 16 * 1024,
            max_rules: 32,
            max_selectors_per_rule: 8,
            max_selector_component_values: 128,
            max_selector_segments_per_selector: 16,
            max_simple_selectors_per_compound: 16,
            max_declarations_per_rule: 32,
            max_component_values_per_container: 256,
            max_component_nesting_depth: 32,
            max_diagnostics: 32,
        },
        specified_value_limits: SpecifiedValueLimits {
            max_components_per_value: 64,
        },
    };
    if let Err(err) = run_seeded_value_fuzz_case(data, config) {
        panic!("css values fuzz invariant failed: {err}");
    }
});
