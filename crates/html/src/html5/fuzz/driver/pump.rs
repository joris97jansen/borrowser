use super::super::config::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzError, Html5PipelineFuzzTermination,
};
use super::state::PipelineRunState;
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{
    Html5Tokenizer, PumpDecision, TokenizeResult, TokenizerFuzzError, ensure_pump_progress,
};
use crate::html5::tree_builder::Html5TreeBuilder;

pub(super) fn pump_until_blocked(
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

pub(super) fn finish_and_drain(
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
