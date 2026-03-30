use super::super::config::{
    Html5PipelineFuzzError, Html5PipelineFuzzSummary, Html5PipelineFuzzTermination,
};
use super::super::digest::{PipelineDigestTail, PipelineFuzzDigest};
use crate::html5::shared::{AtomTable, Input, Token};
use crate::html5::tokenizer::{
    Html5Tokenizer, ObserveError, TextResolver, TokenObserver, TokenizerFuzzError,
};
use crate::html5::tree_builder::{
    DomInvariantState, Html5TreeBuilder, TreeBuilderControlFlow, TreeBuilderProgressWitness,
    VecPatchSink, check_dom_invariants, check_patch_invariants,
};
use crate::test_harness::PatchValidationArena;

pub(super) struct PipelineRunState {
    pub(super) observer: TokenObserver,
    pub(super) dom_state: DomInvariantState,
    patch_arena: PatchValidationArena,
    patches_emitted: usize,
    tokenizer_controls_applied: usize,
    max_patches_observed: usize,
    max_patches_per_flush: usize,
    max_patches_for_input: usize,
    pipeline_steps: usize,
    max_pipeline_steps: usize,
    tokens_without_builder_progress: usize,
    max_tokens_without_builder_progress: usize,
    last_builder_progress_witness: TreeBuilderProgressWitness,
    pub(super) digest: PipelineFuzzDigest,
}

impl PipelineRunState {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        seed: u64,
        max_tokens_streamed: usize,
        max_patches_observed: usize,
        max_patches_per_flush: usize,
        max_patches_for_input: usize,
        max_pipeline_steps: usize,
        max_tokens_without_builder_progress: usize,
        initial_builder_progress_witness: TreeBuilderProgressWitness,
    ) -> Self {
        Self {
            observer: TokenObserver::new(max_tokens_streamed),
            dom_state: DomInvariantState::default(),
            patch_arena: PatchValidationArena::default(),
            patches_emitted: 0,
            tokenizer_controls_applied: 0,
            max_patches_observed,
            max_patches_per_flush,
            max_patches_for_input,
            pipeline_steps: 0,
            max_pipeline_steps,
            tokens_without_builder_progress: 0,
            max_tokens_without_builder_progress,
            last_builder_progress_witness: initial_builder_progress_witness,
            digest: PipelineFuzzDigest::new(seed),
        }
    }

    pub(super) fn note_pipeline_step(&mut self) -> Option<Html5PipelineFuzzTermination> {
        self.pipeline_steps = self.pipeline_steps.saturating_add(1);
        if self.pipeline_steps > self.max_pipeline_steps {
            Some(Html5PipelineFuzzTermination::RejectedMaxPipelineSteps)
        } else {
            None
        }
    }

    fn note_builder_progress(
        &mut self,
        made_progress: bool,
        witness: TreeBuilderProgressWitness,
    ) -> Option<Html5PipelineFuzzTermination> {
        self.last_builder_progress_witness = witness;
        if made_progress {
            self.tokens_without_builder_progress = 0;
            return None;
        }

        self.tokens_without_builder_progress =
            self.tokens_without_builder_progress.saturating_add(1);
        if self.tokens_without_builder_progress > self.max_tokens_without_builder_progress {
            Some(Html5PipelineFuzzTermination::RejectedMaxBuilderNoProgressTokens)
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn process_token(
        &mut self,
        tokenizer: &mut Html5Tokenizer,
        builder: &mut Html5TreeBuilder,
        token: &Token,
        atoms: &AtomTable,
        resolver: &dyn TextResolver,
        phase: &'static str,
        pump_index: usize,
    ) -> Result<Option<Html5PipelineFuzzTermination>, Html5PipelineFuzzError> {
        if let Some(termination) = self.note_pipeline_step() {
            return Ok(Some(termination));
        }

        let token_index = self.observer.tokens_observed;
        match self.observer.observe(token, atoms, resolver) {
            Ok(()) => {}
            Err(ObserveError::TokenBudgetReached) => {
                return Ok(Some(
                    Html5PipelineFuzzTermination::RejectedMaxTokensStreamed,
                ));
            }
            Err(ObserveError::InvalidSpan(source)) => {
                return Err(Html5PipelineFuzzError::Tokenizer(
                    TokenizerFuzzError::InvalidSpan {
                        phase,
                        pump_index,
                        source,
                    },
                ));
            }
            Err(ObserveError::DuplicateEof) => {
                return Err(Html5PipelineFuzzError::Tokenizer(
                    TokenizerFuzzError::DuplicateEof,
                ));
            }
        }

        let before_builder_progress = builder.progress_witness();
        let mut patches = Vec::new();
        let mut sink = VecPatchSink(&mut patches);
        let step = builder
            .push_token(token, atoms, resolver, &mut sink)
            .map_err(|err| Html5PipelineFuzzError::TreeBuilderFailure {
                token_index,
                detail: format!("{err:?}"),
            })?;

        if let Some(control) = step.tokenizer_control {
            tokenizer.apply_control(control);
            self.tokenizer_controls_applied = self.tokenizer_controls_applied.saturating_add(1);
            self.digest.record_tokenizer_control(control);
        }
        if let TreeBuilderControlFlow::Suspend(reason) = step.flow {
            return Err(Html5PipelineFuzzError::UnexpectedSuspend {
                token_index,
                reason,
            });
        }

        if !patches.is_empty() {
            self.dom_state = check_patch_invariants(&patches, &self.dom_state).map_err(|err| {
                Html5PipelineFuzzError::PatchInvariantViolation {
                    token_index,
                    detail: err.to_string(),
                }
            })?;
            self.patch_arena.apply_batch(&patches).map_err(|err| {
                Html5PipelineFuzzError::PatchApplicationViolation {
                    token_index,
                    detail: err.to_string(),
                }
            })?;
            self.patches_emitted = self.patches_emitted.saturating_add(patches.len());
            self.digest.record_patches(&patches);
            if patches.len() > self.max_patches_per_flush {
                return Ok(Some(
                    Html5PipelineFuzzTermination::RejectedMaxPatchesPerFlush,
                ));
            }
            if self.patches_emitted > self.max_patches_observed {
                return Ok(Some(
                    Html5PipelineFuzzTermination::RejectedMaxPatchesObserved,
                ));
            }
            if self.patches_emitted > self.max_patches_for_input {
                return Ok(Some(Html5PipelineFuzzTermination::RejectedMaxPatchDensity));
            }
        }

        let after_builder_progress = builder.progress_witness();
        let made_builder_progress =
            !patches.is_empty() || after_builder_progress != before_builder_progress;

        let live_state = builder.dom_invariant_state();
        check_dom_invariants(&live_state).map_err(|err| {
            Html5PipelineFuzzError::DomInvariantViolation {
                token_index,
                detail: err.to_string(),
            }
        })?;
        if live_state != self.dom_state {
            return Err(Html5PipelineFuzzError::LiveStateMismatch { token_index });
        }
        if let Some(termination) =
            self.note_builder_progress(made_builder_progress, after_builder_progress)
        {
            return Ok(Some(termination));
        }

        Ok(None)
    }

    pub(super) fn completed_summary(
        &self,
        input: &Input,
        seed: u64,
        input_bytes: usize,
        chunk_count: usize,
        saw_one_byte_chunk: bool,
    ) -> Html5PipelineFuzzSummary {
        Html5PipelineFuzzSummary {
            seed,
            termination: Html5PipelineFuzzTermination::Completed,
            input_bytes,
            decoded_bytes: input.as_str().len(),
            chunk_count,
            saw_one_byte_chunk,
            tokens_streamed: self.observer.tokens_observed,
            span_resolve_count: self.observer.span_resolve_count,
            patches_emitted: self.patches_emitted,
            tokenizer_controls_applied: self.tokenizer_controls_applied,
            digest: self.digest.finish(PipelineDigestTail {
                token_digest: self.observer.digest,
                tokens_streamed: self.observer.tokens_observed,
                span_resolve_count: self.observer.span_resolve_count,
                patches_emitted: self.patches_emitted,
                tokenizer_controls_applied: self.tokenizer_controls_applied,
                chunk_count,
                decoded_bytes: input.as_str().len(),
            }),
        }
    }

    pub(super) fn rejected_summary(
        &self,
        input: &Input,
        seed: u64,
        input_bytes: usize,
        chunk_count: usize,
        saw_one_byte_chunk: bool,
        termination: Html5PipelineFuzzTermination,
    ) -> Html5PipelineFuzzSummary {
        Html5PipelineFuzzSummary {
            seed,
            termination,
            input_bytes,
            decoded_bytes: input.as_str().len(),
            chunk_count,
            saw_one_byte_chunk,
            tokens_streamed: self.observer.tokens_observed,
            span_resolve_count: self.observer.span_resolve_count,
            patches_emitted: self.patches_emitted,
            tokenizer_controls_applied: self.tokenizer_controls_applied,
            digest: self.digest.finish(PipelineDigestTail {
                token_digest: self.observer.digest,
                tokens_streamed: self.observer.tokens_observed,
                span_resolve_count: self.observer.span_resolve_count,
                patches_emitted: self.patches_emitted,
                tokenizer_controls_applied: self.tokenizer_controls_applied,
                chunk_count,
                decoded_bytes: input.as_str().len(),
            }),
        }
    }
}
