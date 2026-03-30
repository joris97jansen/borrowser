use super::super::config::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzError, Html5PipelineFuzzSummary,
    Html5PipelineFuzzTermination,
};
use super::pump::{finish_and_drain, pump_until_blocked};
use super::state::PipelineRunState;
use crate::html5::shared::{ByteStreamDecoder, DecodeResult, DocumentParseContext, Input};
use crate::html5::tokenizer::{
    Html5Tokenizer, TokenizerConfig, TokenizerFuzzError, next_chunk_len,
};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, check_dom_invariants};

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
        config.max_patches_per_flush,
        bytes
            .len()
            .max(1)
            .saturating_mul(config.max_patches_per_input_byte),
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
