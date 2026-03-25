use super::super::{BoundaryPolicy, ChunkPlan, shrink_chunk_plan_with_stats};

#[test]
fn shrinker_reduces_boundary_count() {
    let input = "<p>abcd</p>";
    let plan = ChunkPlan::Boundaries {
        indices: vec![1, 2, 3, 4, 5, 6, 7],
        policy: BoundaryPolicy::ByteStream,
    };
    let (minimized, _) = shrink_chunk_plan_with_stats(input, &plan, |candidate| match candidate {
        ChunkPlan::Boundaries { indices, .. } => indices.len() > 2,
        _ => false,
    });
    let minimized_len = match minimized {
        ChunkPlan::Boundaries { indices, .. } => indices.len(),
        _ => 0,
    };
    assert!(
        minimized_len > 0 && minimized_len < 7,
        "expected shrinker to reduce boundary count, got {minimized_len}"
    );
}
