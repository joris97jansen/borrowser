#![no_main]

use css::cascade::fuzz::{CssCascadeFuzzConfig, run_seeded_cascade_fuzz_case};
use css::cascade::StyleResolutionLimits;
use css::selectors::SelectorMatchingLimits;
use css::syntax::{SyntaxLimits, derive_css_fuzz_seed};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_css_fuzz_seed(data);
    let config = CssCascadeFuzzConfig {
        seed,
        max_input_bytes: 4 * 1024,
        max_decoded_bytes: 16 * 1024,
        max_stylesheet_cases: 2,
        max_resolved_elements_observed: 256,
        max_computed_elements_observed: 256,
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
        style_resolution_limits: StyleResolutionLimits {
            max_stylesheets_per_style_pass: 8,
            max_style_rules_per_document: 512,
            max_matched_rules_per_element: 128,
            max_declaration_inputs_per_element: 512,
            max_inline_style_bytes: 512,
            max_inline_declarations_per_element: 32,
            max_styled_elements_per_document: 256,
            selector_matching: SelectorMatchingLimits {
                max_axis_steps_per_match: 2_048,
            },
        },
        ..CssCascadeFuzzConfig::default()
    };
    if let Err(err) = run_seeded_cascade_fuzz_case(data, config) {
        panic!("css cascade fuzz invariant failed: {err}");
    }
});
