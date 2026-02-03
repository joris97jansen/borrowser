//! HTML5 tree builder public API.
//!
//! Consumes HTML5 tokens and emits DOM mutation patches. The builder owns all
//! tree-construction state (insertion modes, stack of open elements, active
//! formatting list, etc.) and is resumable across token boundaries.

use crate::dom_patch::DomPatch;
use crate::html5::shared::{AtomTable, DocumentParseContext, EngineInvariantError, Token};
use crate::html5::tokenizer::TextResolver;

#[derive(Clone, Debug, Default)]
pub struct TreeBuilderConfig {
    /// Whether to coalesce adjacent text nodes within a batch.
    /// Coalescing must be deterministic and purely local (no buffering thresholds).
    pub coalesce_text: bool,
}

/// Tree builder step result.
#[must_use]
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

/// Tree building should not fail on malformed HTML; invariants are the only error surface for now.
pub type TreeBuilderError = EngineInvariantError;

/// Patch sink for streaming emission.
pub trait PatchSink {
    fn push(&mut self, patch: DomPatch);

    /// Prefer move-based extension to avoid per-item cloning.
    fn extend_cloned(&mut self, patches: &[DomPatch]) {
        for patch in patches {
            self.push(patch.clone());
        }
    }

    fn extend_owned(&mut self, patches: Vec<DomPatch>) {
        for patch in patches {
            self.push(patch);
        }
    }

    /// Drains `patches` to enable caller-owned buffer reuse without reallocating.
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
    max_open_elements_depth: u32,
    max_active_formatting_depth: u32,
}

impl Html5TreeBuilder {
    pub fn new(config: TreeBuilderConfig, _ctx: &mut DocumentParseContext) -> Self {
        Self {
            config,
            max_open_elements_depth: 0,
            max_active_formatting_depth: 0,
        }
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

    fn push_token_impl<S: PatchSink + ?Sized>(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
        _sink: &mut S,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        // TODO: implement tree builder insertion modes and patch emission.
        let _ = self.config.coalesce_text;
        let _ = (
            self.max_open_elements_depth,
            self.max_active_formatting_depth,
        );
        Ok(TreeBuilderStepResult::Continue)
    }

    /// Internal metric: max open elements depth observed since session start.
    pub(crate) fn max_open_elements_depth(&self) -> u32 {
        self.max_open_elements_depth
    }

    /// Internal metric: max active formatting depth observed since session start.
    pub(crate) fn max_active_formatting_depth(&self) -> u32 {
        self.max_active_formatting_depth
    }
}

mod emit;
mod formatting;
mod modes;
mod stack;

#[cfg(test)]
mod tests;
