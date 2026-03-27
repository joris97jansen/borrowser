#![no_main]

use html::html5::tree_builder::{
    TreeBuilderFuzzConfig, derive_tree_builder_fuzz_seed, run_seeded_token_stream_fuzz_case,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let seed = derive_tree_builder_fuzz_seed(data);
    let config = TreeBuilderFuzzConfig {
        seed,
        max_input_bytes: 4 * 1024,
        max_tokens_generated: 512,
        max_attrs_per_tag: 24,
        max_total_attrs: 2 * 1024,
        max_string_bytes_generated: 16 * 1024,
        max_patches_observed: 16 * 1024,
        max_processing_steps: 513,
    };
    if let Err(err) = run_seeded_token_stream_fuzz_case(data, config) {
        panic!("html5 tree-builder token fuzz invariant failed: {err}");
    }
});
