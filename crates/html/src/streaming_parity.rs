//! Streaming parity tests for UTF-8 carry vs byte-stream tokenizer.
//!
//! Fast CI mode: default seeds and budget when `CI` is set.
//! Extended local mode: set `BORROWSER_STREAMING_PARITY_SEEDS` and
//! `BORROWSER_STREAMING_PARITY_BUDGET` to increase coverage.

use crate::dom_snapshot::{DomSnapshotOptions, compare_dom};
use crate::test_harness::run_chunked_bytes_with_tokens;
use crate::{build_owned_dom, tokenize};
use tools::utf8::{finish_utf8, push_utf8_chunk};

const DEFAULT_BUDGET_CI: usize = 300;
const DEFAULT_BUDGET_LOCAL: usize = 1_500;
const DEFAULT_SEEDS_CI: usize = 50;
const DEFAULT_SEEDS_LOCAL: usize = 200;
const SEED_MIX: u64 = 0x9e3779b97f4a7c15;

#[test]
fn streaming_parity_utf8_carry_matches_byte_stream() {
    let cases = [
        "plain ascii",
        "café",
        "e\u{0301}",
        "👨\u{200D}👩\u{200D}👧\u{200D}👦",
        "é<script>😀</script>ö",
        "&amp; café 😀",
    ];
    let seeds = seed_count();
    let budget = run_budget();

    let case_budget = (budget / cases.len()).max(1);
    assert!(
        budget >= cases.len(),
        "streaming parity budget must be >= number of cases; increase BORROWSER_STREAMING_PARITY_BUDGET"
    );
    for (case_idx, input) in cases.iter().enumerate() {
        let mut runs_case = 0usize;
        let bytes = input.as_bytes();
        let base_seed = 0x4f6f726f6d207574 ^ case_idx as u64;

        let mut remaining_budget = case_budget;

        if remaining_budget > 0 {
            let boundaries = Vec::new();
            assert_parity(case_idx, None, bytes, &boundaries);
            runs_case += 1;
            remaining_budget = remaining_budget.saturating_sub(1);
        }

        if remaining_budget > 0 && seeds > 0 {
            let iter_seed = base_seed ^ SEED_MIX;
            let mut rng = LcgRng::new(iter_seed);
            let boundaries = random_boundaries(&mut rng, bytes.len());
            assert_parity(case_idx, Some(iter_seed), bytes, &boundaries);
            runs_case += 1;
            remaining_budget = remaining_budget.saturating_sub(1);
        }

        if bytes.len() > 1 {
            for boundaries in [vec![1], vec![bytes.len() - 1]] {
                if remaining_budget == 0 {
                    break;
                }
                assert_parity(case_idx, None, bytes, &boundaries);
                runs_case += 1;
                remaining_budget = remaining_budget.saturating_sub(1);
            }
        }

        for size in [1usize, 2, 3, 4, 7, 16] {
            if remaining_budget == 0 {
                break;
            }
            let mut boundaries = Vec::new();
            let mut offset = size;
            while offset < bytes.len() {
                boundaries.push(offset);
                offset += size;
            }
            assert_parity(case_idx, None, bytes, &boundaries);
            runs_case += 1;
            remaining_budget = remaining_budget.saturating_sub(1);
        }

        for iter in 1..seeds {
            if remaining_budget == 0 {
                break;
            }
            let iter_seed = base_seed ^ (iter as u64).wrapping_mul(SEED_MIX);
            let mut rng = LcgRng::new(iter_seed);
            let boundaries = random_boundaries(&mut rng, bytes.len());
            assert_parity(case_idx, Some(iter_seed), bytes, &boundaries);
            runs_case += 1;
            remaining_budget = remaining_budget.saturating_sub(1);
        }
        assert!(
            runs_case > 0,
            "streaming parity case {case_idx} produced no runs; check budget or inputs"
        );
    }
}

#[test]
fn streaming_parity_handles_invalid_utf8_lossily() {
    // Contract: invalid UTF-8 bytes are replaced with U+FFFD.
    let bytes = [0xFFu8, b'f', 0xC3];
    let boundaries = vec![1, 2];
    let assembled = assemble_utf8_from_bytes(&bytes, &boundaries);
    assert_eq!(assembled, "�f�");
}

fn assert_parity(case_idx: usize, seed: Option<u64>, bytes: &[u8], boundaries: &[usize]) {
    let assembled = assemble_utf8_from_bytes(bytes, boundaries);
    let runtime_stream = tokenize(&assembled);
    let runtime_dom = build_owned_dom(&runtime_stream);
    let (harness_dom, harness_stream) = run_chunked_bytes_with_tokens(bytes, boundaries);
    compare_dom(&runtime_dom, &harness_dom, DomSnapshotOptions::default()).unwrap_or_else(|err| {
        let payload = if assembled.len() <= 128 {
            format!("assembled={assembled:?}")
        } else {
            format!("assembled_len={}", assembled.len())
        };
        let seed_label = seed
            .map(|seed| format!("seed=0x{seed:016x}"))
            .unwrap_or_else(|| "seed=explicit".to_string());
        panic!(
            "streaming parity mismatch for case={case_idx} {seed_label} boundaries={boundaries:?}: {err}\n{payload}\nruntime_tokens: {}\nharness_tokens: {}",
            snapshot_preview(&runtime_stream),
            snapshot_preview(&harness_stream)
        )
    });
}

fn assemble_utf8_from_bytes(bytes: &[u8], boundaries: &[usize]) -> String {
    let mut text = String::new();
    let mut carry = Vec::new();
    let mut last = 0usize;
    for &idx in boundaries {
        assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
        push_utf8_chunk(&mut text, &mut carry, &bytes[last..idx]);
        last = idx;
    }
    if last < bytes.len() {
        push_utf8_chunk(&mut text, &mut carry, &bytes[last..]);
    }
    finish_utf8(&mut text, &mut carry);
    text
}

fn snapshot_preview(stream: &crate::TokenStream) -> String {
    let snapshot = crate::test_utils::token_snapshot(stream);
    let head = snapshot.iter().take(20).cloned().collect::<Vec<_>>();
    format!("len={} head=[{}]", snapshot.len(), head.join(", "))
}

fn seed_count() -> usize {
    if let Ok(value) = std::env::var("BORROWSER_STREAMING_PARITY_SEEDS")
        && let Ok(parsed) = value.parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    if std::env::var("CI").is_ok() {
        DEFAULT_SEEDS_CI
    } else {
        DEFAULT_SEEDS_LOCAL
    }
}

fn run_budget() -> usize {
    if let Ok(value) = std::env::var("BORROWSER_STREAMING_PARITY_BUDGET")
        && let Ok(parsed) = value.parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    if std::env::var("CI").is_ok() {
        DEFAULT_BUDGET_CI
    } else {
        DEFAULT_BUDGET_LOCAL
    }
}

fn random_boundaries(rng: &mut LcgRng, len: usize) -> Vec<usize> {
    if len <= 1 {
        return Vec::new();
    }
    let max_points = (len - 1).min(64);
    let count = rng.gen_range_usize(0, max_points + 1);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let idx = rng.gen_range_usize(1, len);
        out.push(idx);
    }
    out.sort_unstable();
    out.dedup();
    out
}

struct LcgRng {
    state: u64,
}

impl LcgRng {
    fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn gen_range_usize(&mut self, start: usize, end: usize) -> usize {
        assert!(start < end, "invalid range: {start}..{end}");
        let span = (end - start) as u64;
        (self.next_u64() % span) as usize + start
    }
}
