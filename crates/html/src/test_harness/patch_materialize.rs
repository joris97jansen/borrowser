use crate::Node;
use crate::patch_validation::PatchValidationArena;

/// Materialize a DOM tree from patch batches.
///
/// This helper applies each batch through the same minimal patch validator used
/// by fuzzing and strict parser tests. Failures therefore carry the same patch
/// ordering and application diagnostics that the hardening lanes rely on.
pub fn materialize_patch_batches(batches: &[Vec<crate::DomPatch>]) -> Result<Node, String> {
    let mut arena = PatchValidationArena::default();
    for batch in batches {
        arena.apply_batch(batch).map_err(|err| err.to_string())?;
    }
    arena.materialize().map_err(|err| err.to_string())
}

/// Backwards-compatible helper: treat a single vector as one batch.
pub fn materialize_patches(patches: &[crate::DomPatch]) -> Result<Node, String> {
    materialize_patch_batches(&[patches.to_vec()])
}
