mod chunk_plan;
mod fuzz;
mod patch_materialize;
mod runner;

pub use crate::patch_validation::{PatchValidationArena, PatchValidationError};
pub use chunk_plan::{BoundaryPolicy, ChunkPlan, default_chunk_plans, deterministic_chunk_plans};
pub use fuzz::{
    FuzzChunkPlan, FuzzMode, ShrinkStats, random_chunk_plan, shrink_chunk_plan,
    shrink_chunk_plan_with_stats,
};
pub use patch_materialize::{materialize_patch_batches, materialize_patches};
pub use runner::{run_chunked, run_chunked_with_tokens, run_full};

pub(crate) use chunk_plan::filter_boundaries_by_policy;

#[cfg(test)]
pub use runner::run_chunked_bytes_with_tokens;

#[cfg(test)]
mod tests;
