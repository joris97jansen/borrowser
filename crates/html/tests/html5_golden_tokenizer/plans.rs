use html::chunker::{ChunkPlanCase, ChunkerConfig, build_chunk_plans, utf8_internal_boundaries};
use html::test_harness::ChunkPlan;

const EVERY_BOUNDARY_MAX_BYTES: usize = 256;

pub(crate) fn build_tokenizer_chunk_plans(
    input: &str,
    fuzz_runs: usize,
    fuzz_seed: u64,
) -> Vec<ChunkPlanCase> {
    let mut plans = build_chunk_plans(input, fuzz_runs, fuzz_seed, ChunkerConfig::utf8());
    if let Some(plan) = every_boundary_plan_for_small_input(input) {
        plans.push(plan);
    }
    plans
}

fn every_boundary_plan_for_small_input(input: &str) -> Option<ChunkPlanCase> {
    if input.len() <= 1 || input.len() > EVERY_BOUNDARY_MAX_BYTES {
        return None;
    }
    let boundaries = utf8_internal_boundaries(input);
    if boundaries.is_empty() {
        return None;
    }
    Some(ChunkPlanCase {
        label: format!("every-boundary utf8 count={}", boundaries.len()),
        plan: ChunkPlan::boundaries(boundaries),
    })
}
