use super::super::config::{
    TreeBuilderFuzzConfig, TreeBuilderFuzzTermination, derive_tree_builder_fuzz_seed,
};
use super::super::driver::run_seeded_token_stream_fuzz_case;

#[test]
fn tree_builder_fuzz_seed_is_stable_for_same_bytes() {
    let bytes = b"tree-builder-fuzz-seed";
    assert_eq!(
        derive_tree_builder_fuzz_seed(bytes),
        derive_tree_builder_fuzz_seed(bytes)
    );
}

#[test]
fn seeded_tree_builder_fuzz_harness_is_reproducible() {
    let bytes = b"doctype-table-formatting-stress";
    let config = TreeBuilderFuzzConfig {
        seed: 0x4242,
        max_tokens_generated: 128,
        ..TreeBuilderFuzzConfig::default()
    };
    let first = run_seeded_token_stream_fuzz_case(bytes, config).expect("first run should pass");
    let second = run_seeded_token_stream_fuzz_case(bytes, config).expect("second run should pass");

    assert_eq!(first, second);
    assert_eq!(first.termination, TreeBuilderFuzzTermination::Completed);
    assert!(first.tokens_generated > 0);
}

#[test]
fn seeded_tree_builder_fuzz_harness_handles_malformed_sequences() {
    let bytes = b"\xff\x00stray-end-tags&&tables";
    let summary = run_seeded_token_stream_fuzz_case(
        bytes,
        TreeBuilderFuzzConfig {
            seed: derive_tree_builder_fuzz_seed(bytes),
            max_tokens_generated: 256,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("malformed synthetic token stream should remain recoverable");

    assert_eq!(summary.termination, TreeBuilderFuzzTermination::Completed);
    assert!(summary.tokens_generated > 0);
}

#[test]
fn seeded_tree_builder_fuzz_harness_replays_self_closing_html_regression() {
    let bytes = [0x83];
    let summary = run_seeded_token_stream_fuzz_case(
        &bytes,
        TreeBuilderFuzzConfig {
            seed: derive_tree_builder_fuzz_seed(&bytes),
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("self-closing <html/> regression input should stay recoverable");

    assert_eq!(summary.termination, TreeBuilderFuzzTermination::Completed);
    assert_eq!(summary.tokens_generated, 1);
}
