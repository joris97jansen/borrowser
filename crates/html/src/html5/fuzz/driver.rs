use super::config::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzError, Html5PipelineFuzzSummary,
    Html5PipelineFuzzTermination,
};
use super::digest::{PipelineDigestTail, PipelineFuzzDigest};
use crate::html5::shared::{ByteStreamDecoder, DecodeResult, DocumentParseContext, Input};
use crate::html5::tokenizer::{
    Html5Tokenizer, ObserveError, PumpDecision, TokenObserver, TokenizeResult, TokenizerConfig,
    TokenizerFuzzError, ensure_pump_progress, next_chunk_len,
};
use crate::html5::tree_builder::{
    DomInvariantState, Html5TreeBuilder, TreeBuilderConfig, TreeBuilderControlFlow,
    TreeBuilderProgressWitness, VecPatchSink, check_dom_invariants, check_patch_invariants,
};

pub fn run_seeded_html5_pipeline_fuzz_case(
    bytes: &[u8],
    config: Html5PipelineFuzzConfig,
) -> Result<Html5PipelineFuzzSummary, Html5PipelineFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(Html5PipelineFuzzSummary {
            seed: config.seed,
            termination: Html5PipelineFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            chunk_count: 0,
            saw_one_byte_chunk: false,
            tokens_streamed: 0,
            span_resolve_count: 0,
            patches_emitted: 0,
            tokenizer_controls_applied: 0,
            digest: 0,
        });
    }

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).map_err(|err| {
            Html5PipelineFuzzError::TreeBuilderFailure {
                token_index: 0,
                detail: format!("failed to initialize tree builder: {err:?}"),
            }
        })?;
    let mut decoder = ByteStreamDecoder::new();
    let mut input = Input::new();
    let mut rng = crate::html5::tokenizer::HarnessRng::new(config.seed);
    let mut state = PipelineRunState::new(
        config.seed,
        config.max_tokens_streamed,
        config.max_patches_observed,
        config.max_pipeline_steps,
        config.max_tokens_without_builder_progress,
        builder.progress_witness(),
    );
    let max_chunk_len = config.max_chunk_len.max(1);
    let mut chunk_count = 0usize;
    let mut offset = 0usize;
    let mut saw_one_byte_chunk = false;

    check_dom_invariants(&state.dom_state).map_err(|err| {
        Html5PipelineFuzzError::DomInvariantViolation {
            token_index: 0,
            detail: err.to_string(),
        }
    })?;

    while offset < bytes.len() {
        let chunk_len = next_chunk_len(bytes.len() - offset, chunk_count, max_chunk_len, &mut rng);
        saw_one_byte_chunk |= chunk_len == 1;
        state.digest.record_chunk_len(chunk_len);
        decoder.push_bytes(&bytes[offset..offset + chunk_len], &mut input);
        chunk_count = chunk_count.saturating_add(1);
        offset += chunk_len;

        if input.as_str().len() > config.max_decoded_bytes {
            return Ok(state.rejected_summary(
                &input,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                Html5PipelineFuzzTermination::RejectedMaxDecodedBytes,
            ));
        }

        if let Some(termination) = pump_until_blocked(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut builder,
            &mut state,
            "streaming",
        )? {
            return Ok(state.rejected_summary(
                &input,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                termination,
            ));
        }
    }

    if matches!(decoder.finish(&mut input), DecodeResult::Progress) {
        if input.as_str().len() > config.max_decoded_bytes {
            return Ok(state.rejected_summary(
                &input,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                Html5PipelineFuzzTermination::RejectedMaxDecodedBytes,
            ));
        }

        if let Some(termination) = pump_until_blocked(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut builder,
            &mut state,
            "decoder-finish",
        )? {
            return Ok(state.rejected_summary(
                &input,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                termination,
            ));
        }
    }

    // Exhausted decoded input without EOF is an expected pipeline boundary for
    // partial constructs at EOF. Finalize through `finish()` rather than
    // treating the last `NeedMoreInput` as a stall.
    if let Some(termination) = finish_and_drain(
        &mut tokenizer,
        &mut input,
        &ctx,
        &mut builder,
        &mut state,
        config,
    )? {
        return Ok(state.rejected_summary(
            &input,
            config.seed,
            bytes.len(),
            chunk_count,
            saw_one_byte_chunk,
            termination,
        ));
    }

    if !state.observer.saw_eof {
        return Err(Html5PipelineFuzzError::Tokenizer(
            TokenizerFuzzError::MissingEof,
        ));
    }

    let live_state = builder.dom_invariant_state();
    check_dom_invariants(&live_state).map_err(|err| {
        Html5PipelineFuzzError::DomInvariantViolation {
            token_index: state.observer.tokens_observed,
            detail: err.to_string(),
        }
    })?;
    if live_state != state.dom_state {
        return Err(Html5PipelineFuzzError::LiveStateMismatch {
            token_index: state.observer.tokens_observed,
        });
    }

    Ok(state.completed_summary(
        &input,
        config.seed,
        bytes.len(),
        chunk_count,
        saw_one_byte_chunk,
    ))
}

fn pump_until_blocked(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    builder: &mut Html5TreeBuilder,
    state: &mut PipelineRunState,
    phase: &'static str,
) -> Result<Option<Html5PipelineFuzzTermination>, Html5PipelineFuzzError> {
    let current = tokenizer.capture_invariant_snapshot();
    tokenizer.check_invariants(input).map_err(|err| {
        Html5PipelineFuzzError::Tokenizer(TokenizerFuzzError::InvariantViolation {
            phase,
            pump_index: 0,
            detail: err.to_string(),
        })
    })?;

    let budget = phase_pump_budget(input.as_str().len().saturating_sub(current.cursor));
    for pump_index in 0..budget {
        if let Some(termination) = state.note_pipeline_step() {
            return Ok(Some(termination));
        }
        let before = tokenizer.capture_invariant_snapshot();
        let result = tokenizer.push_input_until_token(input, ctx);
        let drain =
            drain_streaming_batch(tokenizer, input, ctx, builder, state, phase, pump_index)?;
        if let Some(termination) = drain.termination {
            return Ok(Some(termination));
        }

        tokenizer.check_invariants(input).map_err(|err| {
            Html5PipelineFuzzError::Tokenizer(TokenizerFuzzError::InvariantViolation {
                phase,
                pump_index,
                detail: err.to_string(),
            })
        })?;

        let after = tokenizer.capture_invariant_snapshot();
        if let PumpDecision::Fail(source) = ensure_pump_progress(
            phase,
            pump_index,
            result,
            before,
            after,
            drain.drained_tokens,
        ) {
            return Err(Html5PipelineFuzzError::Tokenizer(source));
        }
        if result == TokenizeResult::NeedMoreInput {
            return Ok(None);
        }
    }

    let snapshot = tokenizer.capture_invariant_snapshot();
    Err(Html5PipelineFuzzError::Tokenizer(
        TokenizerFuzzError::PumpBudgetExceeded {
            phase,
            budget,
            cursor: snapshot.cursor,
            queued_tokens: snapshot.queued_tokens,
            detail: format!("state={:?}", snapshot.state),
        },
    ))
}

fn finish_and_drain(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
    builder: &mut Html5TreeBuilder,
    state: &mut PipelineRunState,
    config: Html5PipelineFuzzConfig,
) -> Result<Option<Html5PipelineFuzzTermination>, Html5PipelineFuzzError> {
    if let Some(termination) = state.note_pipeline_step() {
        return Ok(Some(termination));
    }
    let finish_result = tokenizer.finish(input);
    if finish_result != TokenizeResult::EmittedEof {
        return Err(Html5PipelineFuzzError::Tokenizer(
            TokenizerFuzzError::InvariantViolation {
                phase: "tokenizer-finish",
                pump_index: 0,
                detail: format!("unexpected finish result: {finish_result:?}"),
            },
        ));
    }
    tokenizer.check_invariants(input).map_err(|err| {
        Html5PipelineFuzzError::Tokenizer(TokenizerFuzzError::InvariantViolation {
            phase: "tokenizer-finish",
            pump_index: 0,
            detail: err.to_string(),
        })
    })?;

    for drain_index in 0..config.finish_drain_budget.max(1) {
        if let Some(termination) = state.note_pipeline_step() {
            return Ok(Some(termination));
        }
        let drain = drain_finish_batch(tokenizer, input, ctx, builder, state, drain_index)?;
        if let Some(termination) = drain.termination {
            return Ok(Some(termination));
        }
        if drain.drained_tokens == 0 || state.observer.saw_eof {
            return Ok(None);
        }
    }

    let snapshot = tokenizer.capture_invariant_snapshot();
    Err(Html5PipelineFuzzError::Tokenizer(
        TokenizerFuzzError::PumpBudgetExceeded {
            phase: "tokenizer-finish",
            budget: config.finish_drain_budget.max(1),
            cursor: snapshot.cursor,
            queued_tokens: snapshot.queued_tokens,
            detail: format!("state={:?}", snapshot.state),
        },
    ))
}

fn drain_streaming_batch(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
    builder: &mut Html5TreeBuilder,
    state: &mut PipelineRunState,
    phase: &'static str,
    pump_index: usize,
) -> Result<DrainResult, Html5PipelineFuzzError> {
    let batch = tokenizer.next_batch(input);
    if batch.tokens().is_empty() {
        return Ok(DrainResult::default());
    }
    // Streaming pumps must stay token-granular so tree-builder tokenizer
    // controls are applied before the tokenizer is allowed to consume more
    // decoded input.
    if batch.tokens().len() != 1 {
        return Err(Html5PipelineFuzzError::NonTokenGranularBatch {
            phase,
            pump_index,
            batch_len: batch.tokens().len(),
        });
    }

    let resolver = batch.resolver();
    let token = batch
        .iter()
        .next()
        .expect("non-empty token-granular batch must contain one token");
    let termination = state.process_token(
        tokenizer, builder, token, &ctx.atoms, &resolver, phase, pump_index,
    )?;
    Ok(DrainResult {
        drained_tokens: 1,
        termination,
    })
}

fn drain_finish_batch(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
    builder: &mut Html5TreeBuilder,
    state: &mut PipelineRunState,
    drain_index: usize,
) -> Result<DrainResult, Html5PipelineFuzzError> {
    let batch = tokenizer.next_batch(input);
    if batch.tokens().is_empty() {
        return Ok(DrainResult::default());
    }

    let resolver = batch.resolver();
    let mut drained_tokens = 0usize;
    // The finish phase is intentionally looser than streaming: once
    // `finish()` has committed EOF, the tokenizer will not consume more input,
    // so a final batch may legally contain multiple already-queued tokens
    // (for example flushed text/comment data plus EOF). We still validate each
    // token incrementally against the tree-builder and invariant state.
    for token in batch.iter() {
        let termination = state.process_token(
            tokenizer,
            builder,
            token,
            &ctx.atoms,
            &resolver,
            "tokenizer-finish",
            drain_index,
        )?;
        drained_tokens = drained_tokens.saturating_add(1);
        if termination.is_some() {
            return Ok(DrainResult {
                drained_tokens,
                termination,
            });
        }
    }

    Ok(DrainResult {
        drained_tokens,
        termination: None,
    })
}

fn phase_pump_budget(remaining_decoded_bytes: usize) -> usize {
    remaining_decoded_bytes
        .saturating_mul(crate::html5::tokenizer::PUMP_BUDGET_FACTOR)
        .saturating_add(crate::html5::tokenizer::MIN_PUMP_BUDGET)
}

#[derive(Default)]
struct DrainResult {
    drained_tokens: usize,
    termination: Option<Html5PipelineFuzzTermination>,
}

struct PipelineRunState {
    observer: TokenObserver,
    dom_state: DomInvariantState,
    patches_emitted: usize,
    tokenizer_controls_applied: usize,
    max_patches_observed: usize,
    pipeline_steps: usize,
    max_pipeline_steps: usize,
    tokens_without_builder_progress: usize,
    max_tokens_without_builder_progress: usize,
    last_builder_progress_witness: TreeBuilderProgressWitness,
    digest: PipelineFuzzDigest,
}

impl PipelineRunState {
    fn new(
        seed: u64,
        max_tokens_streamed: usize,
        max_patches_observed: usize,
        max_pipeline_steps: usize,
        max_tokens_without_builder_progress: usize,
        initial_builder_progress_witness: TreeBuilderProgressWitness,
    ) -> Self {
        Self {
            observer: TokenObserver::new(max_tokens_streamed),
            dom_state: DomInvariantState::default(),
            patches_emitted: 0,
            tokenizer_controls_applied: 0,
            max_patches_observed,
            pipeline_steps: 0,
            max_pipeline_steps,
            tokens_without_builder_progress: 0,
            max_tokens_without_builder_progress,
            last_builder_progress_witness: initial_builder_progress_witness,
            digest: PipelineFuzzDigest::new(seed),
        }
    }

    fn note_pipeline_step(&mut self) -> Option<Html5PipelineFuzzTermination> {
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
    fn process_token(
        &mut self,
        tokenizer: &mut Html5Tokenizer,
        builder: &mut Html5TreeBuilder,
        token: &crate::html5::shared::Token,
        atoms: &crate::html5::shared::AtomTable,
        resolver: &dyn crate::html5::tokenizer::TextResolver,
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
            self.patches_emitted = self.patches_emitted.saturating_add(patches.len());
            self.digest.record_patches(&patches);
            if self.patches_emitted > self.max_patches_observed {
                return Ok(Some(
                    Html5PipelineFuzzTermination::RejectedMaxPatchesObserved,
                ));
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

    fn completed_summary(
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

    fn rejected_summary(
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
