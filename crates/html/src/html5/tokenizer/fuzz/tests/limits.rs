use super::super::config::{TokenizerFuzzConfig, TokenizerFuzzTermination};
use super::super::driver::run_seeded_byte_fuzz_case;

#[test]
fn seeded_byte_fuzz_harness_rejects_inputs_above_explicit_limit() {
    let summary = run_seeded_byte_fuzz_case(
        b"0123456789",
        TokenizerFuzzConfig {
            seed: 0x55,
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
