#![no_main]

use html::html5::{
    Html5PipelineFuzzConfig, derive_html5_pipeline_fuzz_seed, run_seeded_html5_pipeline_fuzz_case,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_html5_pipeline_fuzz_seed(data);
    let config = Html5PipelineFuzzConfig {
        seed,
        max_chunk_len: 32,
        max_input_bytes: 4 * 1024,
        max_decoded_bytes: 16 * 1024,
        max_tokens_streamed: 8 * 1024,
        max_patches_observed: 16 * 1024,
        finish_drain_budget: 32,
    };
    if let Err(err) = run_seeded_html5_pipeline_fuzz_case(data, config) {
        panic!("html5 end-to-end pipeline fuzz invariant failed: {err}");
    }
});
