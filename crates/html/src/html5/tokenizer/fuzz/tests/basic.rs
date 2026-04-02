use super::super::config::{TokenizerFuzzConfig, TokenizerFuzzTermination, derive_fuzz_seed};
use super::super::driver::run_seeded_byte_fuzz_case;
use super::super::rng::{HarnessRng, next_chunk_len};

#[test]
fn fuzz_seed_is_stable_for_same_bytes() {
    let bytes = b"<div>\xF0\x9F\x98\x80</div>";
    assert_eq!(derive_fuzz_seed(bytes), derive_fuzz_seed(bytes));
}

#[test]
fn chunk_planner_is_seeded_and_starts_with_one_byte_chunk() {
    let mut rng_a = HarnessRng::new(0x1234);
    let mut rng_b = HarnessRng::new(0x1234);
    let mut remaining_a = 17usize;
    let mut remaining_b = 17usize;
    let mut sizes_a = Vec::new();
    let mut sizes_b = Vec::new();

    for chunk_index in 0..6 {
        let len_a = next_chunk_len(remaining_a, chunk_index, 8, &mut rng_a);
        let len_b = next_chunk_len(remaining_b, chunk_index, 8, &mut rng_b);
        sizes_a.push(len_a);
        sizes_b.push(len_b);
        remaining_a = remaining_a.saturating_sub(len_a);
        remaining_b = remaining_b.saturating_sub(len_b);
    }

    assert_eq!(sizes_a, sizes_b);
    assert_eq!(sizes_a.first().copied(), Some(1));
}

#[test]
fn seeded_byte_fuzz_harness_is_reproducible() {
    let bytes = b"<!DOCTYPE html><title>caf\xC3\xA9</title><!--x-->";
    let config = TokenizerFuzzConfig {
        seed: 0x4242,
        max_chunk_len: 7,
        ..TokenizerFuzzConfig::default()
    };
    let first = run_seeded_byte_fuzz_case(bytes, config).expect("first run should pass");
    let second = run_seeded_byte_fuzz_case(bytes, config).expect("second run should pass");
    assert_eq!(first, second);
    assert_eq!(first.termination, TokenizerFuzzTermination::Completed);
    assert!(first.saw_one_byte_chunk);
    assert!(first.tokens_observed > 0);
}

#[test]
fn seeded_byte_fuzz_harness_handles_invalid_utf8() {
    let bytes = [0xFFu8, b'<', b'a', b'>', 0xC3];
    let summary = run_seeded_byte_fuzz_case(
        &bytes,
        TokenizerFuzzConfig {
            seed: 0x99,
            max_chunk_len: 4,
            ..TokenizerFuzzConfig::default()
        },
    )
    .expect("invalid UTF-8 case should remain recoverable");
    assert_eq!(summary.termination, TokenizerFuzzTermination::Completed);
    assert!(summary.decoded_bytes >= 3);
    assert!(summary.tokens_observed >= 1);
}

#[test]
fn seeded_byte_fuzz_harness_reaches_finish_boundary_for_lonely_lt() {
    let summary = run_seeded_byte_fuzz_case(
        b"<",
        TokenizerFuzzConfig {
            seed: 0x2718,
            max_chunk_len: 1,
            ..TokenizerFuzzConfig::default()
        },
    )
    .expect("lonely lt case should reach EOF without violating harness invariants");
    assert_eq!(summary.termination, TokenizerFuzzTermination::Completed);
    assert_eq!(summary.input_bytes, 1);
    assert_eq!(summary.decoded_bytes, 1);
    assert!(summary.saw_one_byte_chunk);
    assert_eq!(summary.tokens_observed, 1);
    assert_eq!(summary.span_resolve_count, 0);
}
