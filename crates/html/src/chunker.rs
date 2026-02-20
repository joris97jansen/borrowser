//! Shared chunking utilities for test harnesses.
//!
//! Provides deterministic chunk plans plus seeded fuzz plans for reproducible
//! streaming coverage in CI.

use crate::test_harness::{BoundaryPolicy, ChunkPlan, filter_boundaries_by_policy};

#[derive(Clone, Debug)]
pub struct ChunkPlanCase {
    pub label: String,
    pub plan: ChunkPlan,
}

#[derive(Clone, Copy, Debug)]
pub struct ChunkerConfig {
    pub policy: BoundaryPolicy,
}

impl ChunkerConfig {
    pub fn utf8() -> Self {
        Self {
            policy: BoundaryPolicy::Utf8Aligned,
        }
    }

    pub fn byte_stream() -> Self {
        Self {
            policy: BoundaryPolicy::ByteStream,
        }
    }
}

/// Build deterministic + fuzz chunk plans for the given input.
///
/// - Deterministic includes fixed sizes and boundary-aware splits around
///   `<`, `</`, `>`, and quotes.
/// - Fuzz plans are seeded for CI reproducibility.
pub fn build_chunk_plans(
    input: &str,
    fuzz_runs: usize,
    fuzz_seed: u64,
    config: ChunkerConfig,
) -> Vec<ChunkPlanCase> {
    let mut plans = Vec::new();
    let policy = config.policy;

    for size in [1usize, 2, 3, 4, 8, 16, 32, 64] {
        plans.push(ChunkPlanCase {
            label: format!("fixed size={size}"),
            plan: fixed_plan(size, policy),
        });
    }

    let token_boundaries = token_boundary_indices(input, policy);
    if !token_boundaries.is_empty() {
        plans.push(ChunkPlanCase {
            label: format!("token-boundaries count={}", token_boundaries.len()),
            plan: boundaries_plan(token_boundaries, policy),
        });
    }

    if fuzz_runs > 0 {
        let mut candidates = char_boundaries(input, policy);
        let mut token = token_boundary_indices(input, policy);
        if !token.is_empty() {
            candidates.append(&mut token);
            candidates.sort_unstable();
            candidates.dedup();
        }
        for i in 0..fuzz_runs {
            let seed = fuzz_seed.wrapping_add(i as u64);
            let mut rng = Lcg::new(seed);
            let plan = if !candidates.is_empty() {
                let max = candidates.len().clamp(1, 32);
                let mut picks = candidates.clone();
                rng.shuffle(&mut picks);
                let count = 1 + rng.gen_range(max);
                picks.truncate(count);
                picks.sort_unstable();
                picks.dedup();
                boundaries_plan(picks, policy)
            } else {
                // Fallback for empty/1-byte inputs.
                fixed_plan(1, policy)
            };
            plans.push(ChunkPlanCase {
                label: format!("fuzz boundaries seed=0x{seed:016x}"),
                plan,
            });
        }
    }

    plans
}

pub fn build_chunk_plans_utf8(input: &str, fuzz_runs: usize, fuzz_seed: u64) -> Vec<ChunkPlanCase> {
    build_chunk_plans(input, fuzz_runs, fuzz_seed, ChunkerConfig::utf8())
}

pub fn utf8_internal_boundaries(input: &str) -> Vec<usize> {
    char_boundaries(input, BoundaryPolicy::Utf8Aligned)
}

fn fixed_plan(size: usize, policy: BoundaryPolicy) -> ChunkPlan {
    match policy {
        BoundaryPolicy::Utf8Aligned => ChunkPlan::fixed(size),
        BoundaryPolicy::ByteStream => ChunkPlan::fixed_unaligned(size),
    }
}

fn boundaries_plan(indices: Vec<usize>, policy: BoundaryPolicy) -> ChunkPlan {
    match policy {
        BoundaryPolicy::Utf8Aligned => ChunkPlan::boundaries(indices),
        BoundaryPolicy::ByteStream => ChunkPlan::boundaries_unaligned(indices),
    }
}

fn token_boundary_indices(input: &str, policy: BoundaryPolicy) -> Vec<usize> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < len {
        let b = bytes[i];
        if b == b'<' || b == b'>' || b == b'"' || b == b'\'' {
            out.push(i);
            if i + 1 < len {
                out.push(i + 1);
            }
        }
        if b == b'<' && i + 1 < len && bytes[i + 1] == b'/' {
            out.push(i);
            out.push(i + 1);
        }
        i += 1;
    }
    out.sort_unstable();
    out.dedup();
    filter_boundaries_by_policy(input, &out, policy)
}

fn char_boundaries(input: &str, policy: BoundaryPolicy) -> Vec<usize> {
    let mut out = Vec::new();
    let len = input.len();
    for (idx, _) in input.char_indices() {
        if idx != 0 && idx != len {
            out.push(idx);
        }
    }
    filter_boundaries_by_policy(input, &out, policy)
}

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    fn gen_range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        (self.next_u64() >> 32) as usize % upper
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        if items.len() < 2 {
            return;
        }
        for i in (1..items.len()).rev() {
            let j = self.gen_range(i + 1);
            items.swap(i, j);
        }
    }
}
