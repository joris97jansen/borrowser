use super::super::config::{TokenizerFuzzConfig, TokenizerFuzzTermination, derive_fuzz_seed};
use super::super::driver::run_seeded_script_data_fuzz_case;

#[test]
fn seeded_script_data_fuzz_harness_is_reproducible() {
    let bytes = b"</scriX><<<<</script><!--<script>nested</script>//-->";
    let config = TokenizerFuzzConfig {
        seed: 0x5151,
        max_chunk_len: 9,
        ..TokenizerFuzzConfig::default()
    };
    let first = run_seeded_script_data_fuzz_case(bytes, config).expect("first run should complete");
    let second =
        run_seeded_script_data_fuzz_case(bytes, config).expect("second run should complete");
    assert_eq!(first, second);
    assert_eq!(first.termination, TokenizerFuzzTermination::Completed);
    assert!(first.saw_one_byte_chunk);
    assert!(first.tokens_observed > 0);
}

#[test]
fn seeded_script_data_fuzz_harness_handles_dense_lt_near_miss_storm() {
    let mut bytes = Vec::new();
    for _ in 0..2048 {
        bytes.extend_from_slice(b"<<</scriX");
    }
    bytes.extend_from_slice(b"</script>");

    let summary = run_seeded_script_data_fuzz_case(
        &bytes,
        TokenizerFuzzConfig {
            seed: derive_fuzz_seed(&bytes),
            max_chunk_len: 17,
            max_input_bytes: 64 * 1024,
            max_decoded_bytes: 256 * 1024,
            ..TokenizerFuzzConfig::default()
        },
    )
    .expect("hostile script-data close-tag storm should remain bounded");
    assert_eq!(summary.termination, TokenizerFuzzTermination::Completed);
    assert!(summary.saw_one_byte_chunk);
    assert!(summary.chunk_count > 1);
}

#[test]
fn seeded_script_data_fuzz_harness_rejects_inputs_above_explicit_limit() {
    let summary = run_seeded_script_data_fuzz_case(
        b"</script>",
        TokenizerFuzzConfig {
            seed: 0x77,
            max_input_bytes: 4,
            ..TokenizerFuzzConfig::default()
        },
    )
    .expect("oversized input should be rejected, not crash");
    assert_eq!(
        summary.termination,
        TokenizerFuzzTermination::RejectedMaxInputBytes
    );
    assert_eq!(summary.tokens_observed, 0);
}
