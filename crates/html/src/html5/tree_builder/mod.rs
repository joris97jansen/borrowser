//! HTML5 tree builder public API.
//!
//! Consumes HTML5 tokens and emits DOM mutation patches. The builder owns all
//! tree-construction state (insertion modes, stack of open elements, active
//! formatting list, etc.) and is resumable across token boundaries.

use crate::dom_patch::DomPatch;
use crate::html5::shared::{AtomTable, DocumentParseContext, Token};
use crate::html5::tokenizer::TextResolver;

#[derive(Clone, Debug, Default)]
pub struct TreeBuilderConfig {
    /// Whether to coalesce adjacent text nodes within a batch.
    /// Coalescing must be deterministic and purely local (no buffering thresholds).
    pub coalesce_text: bool,
}

/// Tree builder step result.
#[derive(Clone, Debug)]
pub enum TreeBuilderStepResult {
    Continue,
    Suspend(SuspendReason),
}

#[derive(Clone, Debug)]
pub enum SuspendReason {
    Script,
    Other,
}

#[derive(Clone, Debug)]
/// Engine invariant violation (bug/corruption), not a recoverable HTML error.
pub struct EngineInvariantError;

pub type TreeBuilderError = EngineInvariantError;

/// Patch sink for streaming emission.
pub trait PatchSink {
    fn push(&mut self, patch: DomPatch);

    /// Prefer `push_many` / `extend_owned` in hot paths to avoid per-item cloning.
    fn extend(&mut self, patches: &[DomPatch]) {
        for patch in patches {
            self.push(patch.clone());
        }
    }

    fn extend_owned(&mut self, mut patches: Vec<DomPatch>) {
        self.push_many(&mut patches);
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

/// HTML5 tree builder.
pub struct Html5TreeBuilder {
    config: TreeBuilderConfig,
}

impl Html5TreeBuilder {
    pub fn new(config: TreeBuilderConfig, _ctx: &mut DocumentParseContext) -> Self {
        Self { config }
    }

    /// Push a token into the tree builder.
    ///
    /// Tokens are consumed in order; the builder may emit zero or more patches.
    /// The return value indicates whether parsing can continue or must suspend.
    pub fn push_token(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
        _sink: &mut dyn PatchSink,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        self.push_token_impl(_token, _atoms, _text, _sink)
    }

    fn push_token_impl<S: PatchSink>(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
        _sink: &mut S,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        // TODO: implement tree builder insertion modes and patch emission.
        let _ = self.config.coalesce_text;
        Ok(TreeBuilderStepResult::Continue)
    }
}

mod emit;
mod formatting;
mod modes;
mod stack;

#[cfg(test)]
mod tests;
