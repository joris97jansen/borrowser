use crate::dom_patch::PatchKey;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Debug)]
pub(in crate::html5::tree_builder) struct LastTextPatch {
    pub(in crate::html5::tree_builder) parent: PatchKey,
    pub(in crate::html5::tree_builder) before: Option<PatchKey>,
    pub(in crate::html5::tree_builder) text_key: PatchKey,
}

pub(in crate::html5::tree_builder) struct StructuralMutationScope<'a> {
    pub(in crate::html5::tree_builder) tb: &'a mut Html5TreeBuilder,
}

impl Drop for StructuralMutationScope<'_> {
    fn drop(&mut self) {
        self.tb.end_structural_mutation();
    }
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn invalidate_text_coalescing(&mut self) {
        self.perf_text_coalescing_invalidations =
            self.perf_text_coalescing_invalidations.saturating_add(1);
        self.last_text_patch = None;
    }

    pub(in crate::html5::tree_builder) fn begin_structural_mutation(&mut self) {
        if self.structural_mutation_depth == 0 {
            self.invalidate_text_coalescing();
        }
        self.structural_mutation_depth = self
            .structural_mutation_depth
            .checked_add(1)
            .expect("structural mutation depth overflow");
    }

    pub(in crate::html5::tree_builder) fn end_structural_mutation(&mut self) {
        assert!(
            self.structural_mutation_depth > 0,
            "structural mutation depth underflow"
        );
        self.structural_mutation_depth -= 1;
    }

    pub(in crate::html5::tree_builder) fn with_structural_mutation<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, TreeBuilderError>,
    ) -> Result<T, TreeBuilderError> {
        self.begin_structural_mutation();
        let scope = StructuralMutationScope { tb: self };
        let result = f(scope.tb);
        drop(scope);
        result
    }
}
