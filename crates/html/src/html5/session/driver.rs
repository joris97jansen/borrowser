use super::api::{DrainMode, DrainOutcome, Html5ParseSession};
use crate::html5::bridge::PatchEmitterAdapter;
#[cfg(any(test, feature = "parser-conformance"))]
use crate::html5::bridge::PatchHistoryCaptureFailure;
use crate::html5::shared::{
    DocumentParseContext, EngineInvariantError, Html5SessionError, ParserFatalError, Token,
};
use crate::html5::tokenizer::{TextResolver, TokenizeResult, TokenizerControl};
use crate::html5::tree_builder::{
    Html5TreeBuilder, TreeBuilderControlFlow, TreeBuilderProcessContext, TreeBuilderStepResult,
};
#[cfg(any(test, feature = "debug-stats"))]
use log::error;

impl Html5ParseSession {
    pub(super) fn pump_live_input(&mut self) -> Result<(), Html5SessionError> {
        loop {
            self.builder.prepare_tokenizer_pump(&mut self.tokenizer);
            let tokenize_result = self
                .tokenizer
                .push_input_until_token(&mut self.input, &mut self.ctx);
            if self.tokenizer.invariant_failure_kind().is_some() {
                return Err(EngineInvariantError.into());
            }
            if self.drain_emitted_tokens(DrainMode::TokenGranular)? == DrainOutcome::Suspended {
                break;
            }
            if tokenize_result == TokenizeResult::NeedMoreInput {
                break;
            }
        }

        self.finalize_adapter_invariants()
    }

    pub(super) fn drain_emitted_tokens(
        &mut self,
        mode: DrainMode,
    ) -> Result<DrainOutcome, Html5SessionError> {
        match mode {
            DrainMode::TokenGranular => self.drain_token_granular_batch(),
            DrainMode::ExhaustQueuedBatches => self.drain_all_queued_batches(),
        }
    }

    pub(super) fn drain_token_granular_batch(&mut self) -> Result<DrainOutcome, Html5SessionError> {
        let processed = {
            let batch = if self.ctx.observation_enabled() {
                self.tokenizer
                    .next_batch_observed(&mut self.input, &mut self.ctx)
            } else {
                self.tokenizer.next_batch(&mut self.input)
            };
            if batch.tokens().is_empty() {
                return Ok(DrainOutcome::Idle);
            }

            debug_assert_eq!(
                batch.tokens().len(),
                1,
                "token-granular pump must not expose more than one token per drain"
            );

            let resolver = batch.resolver();
            let token = batch
                .iter()
                .next()
                .expect("non-empty token-granular batch must contain one token");
            Self::process_token(
                &mut self.ctx,
                &mut self.builder,
                &mut self.patch_emitter,
                token,
                &resolver,
            )
        };
        let step = self.resolve_processed_token(processed)?;

        Ok(self.apply_tree_builder_step(step))
    }

    pub(super) fn drain_all_queued_batches(&mut self) -> Result<DrainOutcome, Html5SessionError> {
        let (steps, failure) = {
            let batch = if self.ctx.observation_enabled() {
                self.tokenizer
                    .next_batch_observed(&mut self.input, &mut self.ctx)
            } else {
                self.tokenizer.next_batch(&mut self.input)
            };
            if batch.tokens().is_empty() {
                return Ok(DrainOutcome::Idle);
            }

            let resolver = batch.resolver();
            let mut steps = Vec::with_capacity(batch.tokens().len());
            let mut failure = None;
            for token in batch.iter() {
                let processed = Self::process_token(
                    &mut self.ctx,
                    &mut self.builder,
                    &mut self.patch_emitter,
                    token,
                    &resolver,
                );
                match processed.into_outcome() {
                    Ok(step) => steps.push(step),
                    Err(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
            (steps, failure)
        };
        if let Some(failure) = failure {
            return self.resolve_processed_token_failure(failure);
        }

        for step in steps {
            if self.apply_tree_builder_step(step) == DrainOutcome::Suspended {
                return Ok(DrainOutcome::Suspended);
            }
        }

        Ok(DrainOutcome::Continue)
    }

    pub(super) fn drain_post_finish_batches(
        &mut self,
        budget: usize,
    ) -> Result<(), Html5SessionError> {
        for _ in 0..budget.max(1) {
            match self.drain_emitted_tokens(DrainMode::ExhaustQueuedBatches)? {
                DrainOutcome::Idle => return Ok(()),
                DrainOutcome::Continue => {}
                DrainOutcome::Suspended => {
                    return Err(EngineInvariantError.into());
                }
            }
        }
        Err(EngineInvariantError.into())
    }

    pub(super) fn process_token(
        ctx: &mut DocumentParseContext,
        builder: &mut Html5TreeBuilder,
        patch_emitter: &mut PatchEmitterAdapter,
        token: &Token,
        resolver: &dyn TextResolver,
    ) -> ProcessedToken {
        ctx.counters.tokens_processed = ctx.counters.tokens_processed.saturating_add(1);

        let mut process_context = TreeBuilderProcessContext::for_integrated_parser(ctx);
        let builder_result =
            builder.push_token(token, &mut process_context, resolver, patch_emitter);
        #[cfg(any(test, feature = "parser-conformance"))]
        let patch_history_failure = patch_emitter.take_patch_history_failure();
        ProcessedToken {
            builder_result,
            #[cfg(any(test, feature = "parser-conformance"))]
            patch_history_failure,
        }
    }

    fn resolve_processed_token(
        &mut self,
        processed: ProcessedToken,
    ) -> Result<TreeBuilderStepResult, Html5SessionError> {
        processed
            .into_outcome()
            .or_else(|failure| self.resolve_processed_token_failure(failure))
    }

    fn resolve_processed_token_failure<T>(
        &mut self,
        failure: ProcessedTokenFailure,
    ) -> Result<T, Html5SessionError> {
        match failure {
            #[cfg(any(test, feature = "parser-conformance"))]
            ProcessedTokenFailure::PatchHistory(failure) => {
                self.resolve_patch_history_failure(failure)
            }
            ProcessedTokenFailure::TreeBuilder(err) => {
                if matches!(err, ParserFatalError::EngineInvariant) {
                    self.ctx.counters.tree_builder_invariant_errors = self
                        .ctx
                        .counters
                        .tree_builder_invariant_errors
                        .saturating_add(1);
                }
                #[cfg(any(test, feature = "debug-stats"))]
                error!(target: "html5", "tree builder fatal error: {err:?}");
                #[cfg(not(any(test, feature = "debug-stats")))]
                let _ = err;
                Err(Html5SessionError::Fatal(err))
            }
        }
    }

    #[cfg(test)]
    pub(super) fn resolve_patch_history_capture_failure(
        &mut self,
    ) -> Result<(), Html5SessionError> {
        match self.patch_emitter.take_patch_history_failure() {
            Some(failure) => self.resolve_patch_history_failure::<()>(failure),
            None => Ok(()),
        }
    }

    #[cfg(any(test, feature = "parser-conformance"))]
    fn resolve_patch_history_failure<T>(
        &mut self,
        failure: PatchHistoryCaptureFailure,
    ) -> Result<T, Html5SessionError> {
        match failure {
            PatchHistoryCaptureFailure::ResourceExhaustion(error) => Err(Html5SessionError::Fatal(
                ParserFatalError::ResourceExhaustion(error),
            )),
            PatchHistoryCaptureFailure::Invariant(invariant) => {
                if self.patch_history_invariant.is_none() {
                    self.patch_history_invariant = Some(invariant);
                }
                Err(Html5SessionError::Fatal(ParserFatalError::EngineInvariant))
            }
        }
    }

    pub(super) fn apply_tree_builder_step(&mut self, step: TreeBuilderStepResult) -> DrainOutcome {
        self.apply_tokenizer_control(step.tokenizer_control);
        if matches!(step.flow, TreeBuilderControlFlow::Suspend(_)) {
            DrainOutcome::Suspended
        } else {
            DrainOutcome::Continue
        }
    }

    pub(super) fn finalize_adapter_invariants(&mut self) -> Result<(), Html5SessionError> {
        if self.patch_emitter.take_invariant_violation() {
            self.ctx.counters.adapter_invariant_violations = self
                .ctx
                .counters
                .adapter_invariant_violations
                .saturating_add(1);
            #[cfg(any(test, feature = "debug-stats"))]
            error!(target: "html5", "patch emitter invariant violation");
            return Err(EngineInvariantError.into());
        }

        Ok(())
    }

    pub(super) fn apply_tokenizer_control(&mut self, control: Option<TokenizerControl>) {
        if let Some(control) = control {
            #[cfg(all(test, feature = "parser-conformance"))]
            self.applied_tokenizer_controls_for_test.push(control);
            self.tokenizer.apply_control(control);
        }
    }
}

pub(super) struct ProcessedToken {
    builder_result: Result<TreeBuilderStepResult, ParserFatalError>,
    #[cfg(any(test, feature = "parser-conformance"))]
    patch_history_failure: Option<PatchHistoryCaptureFailure>,
}

impl ProcessedToken {
    fn into_outcome(self) -> Result<TreeBuilderStepResult, ProcessedTokenFailure> {
        // A capture failure is emitted synchronously while `push_token` owns
        // the patch sink, so it is the earliest detected failure even when the
        // same call also returns a tree-builder fatal.
        #[cfg(any(test, feature = "parser-conformance"))]
        if let Some(failure) = self.patch_history_failure {
            return Err(ProcessedTokenFailure::PatchHistory(failure));
        }
        self.builder_result
            .map_err(ProcessedTokenFailure::TreeBuilder)
    }
}

enum ProcessedTokenFailure {
    #[cfg(any(test, feature = "parser-conformance"))]
    PatchHistory(PatchHistoryCaptureFailure),
    TreeBuilder(ParserFatalError),
}

#[cfg(all(test, feature = "parser-conformance"))]
mod patch_history_precedence_tests {
    use super::{ProcessedToken, ProcessedTokenFailure};
    use crate::html5::bridge::PatchHistoryCaptureFailure;
    use crate::html5::shared::{ParserFatalError, ParserObservationInvariant};

    #[test]
    fn synchronously_latched_capture_failure_precedes_same_token_builder_fatal() {
        let processed = ProcessedToken {
            builder_result: Err(ParserFatalError::EngineInvariant),
            patch_history_failure: Some(PatchHistoryCaptureFailure::Invariant(
                ParserObservationInvariant::PatchDroppedCountOverflow,
            )),
        };
        assert!(matches!(
            processed.into_outcome(),
            Err(ProcessedTokenFailure::PatchHistory(
                PatchHistoryCaptureFailure::Invariant(
                    ParserObservationInvariant::PatchDroppedCountOverflow
                )
            ))
        ));
    }
}
