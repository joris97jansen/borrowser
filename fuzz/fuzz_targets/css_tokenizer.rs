#![no_main]

use css::syntax::{
    CssTokenizerFuzzConfig, derive_css_fuzz_seed, run_seeded_tokenizer_fuzz_case,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_css_fuzz_seed(data);
    let config = CssTokenizerFuzzConfig {
        seed,
        max_input_bytes: 4 * 1024,
        max_decoded_bytes: 16 * 1024,
        max_tokens_observed: 32 * 1024,
        max_diagnostics_observed: 64,
    };
    if let Err(err) = run_seeded_tokenizer_fuzz_case(data, config) {
        panic!("css tokenizer fuzz invariant failed: {err}");
    }
});
