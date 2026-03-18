use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::EngineInvariantError;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};
use std::num::NonZeroU32;

/// Patch sink for streaming emission.
pub trait PatchSink {
    fn push(&mut self, patch: DomPatch);

    fn extend_owned(&mut self, patches: Vec<DomPatch>) {
        for patch in patches {
            self.push(patch);
        }
    }

    fn push_many(&mut self, patches: &mut Vec<DomPatch>) {
        for patch in patches.drain(..) {
            self.push(patch);
        }
    }
}

/// Patch sink that buffers into a Vec.
pub struct VecPatchSink<'a>(pub &'a mut Vec<DomPatch>);

impl<'a> PatchSink for VecPatchSink<'a> {
    fn push(&mut self, patch: DomPatch) {
        self.0.push(patch);
    }
}

/// Patch sink backed by a callback.
pub struct CallbackPatchSink<F>(pub F)
where
    F: FnMut(DomPatch);

impl<F> PatchSink for CallbackPatchSink<F>
where
    F: FnMut(DomPatch),
{
    fn push(&mut self, patch: DomPatch) {
        (self.0)(patch);
    }
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn alloc_patch_key(
        &mut self,
    ) -> Result<PatchKey, TreeBuilderError> {
        let key = PatchKey(self.next_patch_key.get());
        let next = self
            .next_patch_key
            .get()
            .checked_add(1)
            .ok_or(EngineInvariantError)?;
        self.next_patch_key = NonZeroU32::new(next).ok_or(EngineInvariantError)?;
        Ok(key)
    }

    #[track_caller]
    pub(in crate::html5::tree_builder) fn push_structural_patch(&mut self, patch: DomPatch) {
        debug_assert!(self.structural_mutation_depth > 0);
        self.live_tree.apply_structural_patch(&patch);
        self.push_patch(patch);
    }

    pub(in crate::html5::tree_builder) fn push_patch(&mut self, patch: DomPatch) {
        self.perf_patches_emitted = self.perf_patches_emitted.saturating_add(1);
        self.patches.push(patch);
    }
}
