#![no_main]

use html::html5::tokenizer::{
    TokenizerFuzzConfig, derive_fuzz_seed, run_seeded_textarea_rcdata_fuzz_case,
    run_seeded_title_rcdata_fuzz_case,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_fuzz_seed(data);
    let config = TokenizerFuzzConfig {
        seed,
        max_chunk_len: 32,
        max_input_bytes: 4 * 1024,
        max_decoded_bytes: 16 * 1024,
        max_tokens_observed: 64 * 1024,
        finish_drain_budget: 32,
    };
    if let Err(err) = run_seeded_title_rcdata_fuzz_case(data, config) {
        panic!("html5 tokenizer rcdata-title fuzz invariant failed (seed={seed}): {err}");
    }
    if let Err(err) = run_seeded_textarea_rcdata_fuzz_case(data, config) {
        panic!("html5 tokenizer rcdata-textarea fuzz invariant failed (seed={seed}): {err}");
    }
});
