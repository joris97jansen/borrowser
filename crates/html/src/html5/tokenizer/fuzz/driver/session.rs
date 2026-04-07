use crate::html5::shared::{ByteStreamDecoder, DocumentParseContext, Input};
use crate::html5::tokenizer::fuzz::config::{
    TokenizerFuzzConfig, TokenizerFuzzError, TokenizerFuzzSummary, TokenizerFuzzTermination,
};
use crate::html5::tokenizer::fuzz::observe::TokenObserver;
use crate::html5::tokenizer::fuzz::rng::{HarnessRng, next_chunk_len};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};

use super::pump::{finish_and_drain, pump_text_mode_until_blocked, pump_until_blocked};
use super::summary::rejected_summary;
use super::text_mode::{TargetedTextModeHarnessKind, TextModeFuzzController};

pub(super) fn run_seeded_byte_fuzz_case_impl(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_case(bytes, config, None)
}

pub(super) fn run_seeded_controlled_text_mode_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
    mode: TargetedTextModeHarnessKind,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_case(bytes, config, Some(mode))
}

fn run_seeded_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
    mode: Option<TargetedTextModeHarnessKind>,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    let mut observer = TokenObserver::new(config.max_tokens_observed);
    if bytes.len() > config.max_input_bytes {
        return Ok(rejected_summary(
            0,
            &observer,
            config.seed,
            bytes.len(),
            0,
            false,
            TokenizerFuzzTermination::RejectedMaxInputBytes,
        ));
    }

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut controller = mode.map(|mode| {
        let tag_name = ctx
            .atoms
            .intern_ascii_folded(mode.end_tag_name_literal())
            .expect("text-mode atom interning must succeed");
        let mut controller = TextModeFuzzController::new(mode.spec(tag_name));
        controller.enter_initial(&mut tokenizer);
        controller
    });
    let mut decoder = ByteStreamDecoder::new();
    let mut input = Input::new();
    let mut rng = HarnessRng::new(config.seed);
    let mut saw_one_byte_chunk = false;
    let mut chunk_count = 0usize;
    let mut offset = 0usize;
    let max_chunk_len = config.max_chunk_len.max(1);

    while offset < bytes.len() {
        let chunk_len = next_chunk_len(bytes.len() - offset, chunk_count, max_chunk_len, &mut rng);
        saw_one_byte_chunk |= chunk_len == 1;
        decoder.push_bytes(&bytes[offset..offset + chunk_len], &mut input);
        chunk_count = chunk_count.saturating_add(1);
        offset += chunk_len;
        if input.as_str().len() > config.max_decoded_bytes {
            return Ok(rejected_summary(
                input.as_str().len(),
                &observer,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                TokenizerFuzzTermination::RejectedMaxDecodedBytes,
            ));
        }
        if let Some(termination) = pump_phase(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut observer,
            controller.as_mut(),
            "streaming",
        )? {
            return Ok(rejected_summary(
                input.as_str().len(),
                &observer,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                termination,
            ));
        }
    }

    let flush_result = decoder.finish(&mut input);
    if matches!(flush_result, crate::html5::shared::DecodeResult::Progress) {
        if input.as_str().len() > config.max_decoded_bytes {
            return Ok(rejected_summary(
                input.as_str().len(),
                &observer,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                TokenizerFuzzTermination::RejectedMaxDecodedBytes,
            ));
        }
        if let Some(termination) = pump_phase(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut observer,
            controller.as_mut(),
            "decoder-finish",
        )? {
            return Ok(rejected_summary(
                input.as_str().len(),
                &observer,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                termination,
            ));
        }
    }
    if let Some(termination) = pump_phase(
        &mut tokenizer,
        &mut input,
        &mut ctx,
        &mut observer,
        controller.as_mut(),
        "pre-finish",
    )? {
        return Ok(rejected_summary(
            input.as_str().len(),
            &observer,
            config.seed,
            bytes.len(),
            chunk_count,
            saw_one_byte_chunk,
            termination,
        ));
    }

    if let Some(termination) = finish_and_drain(
        &mut tokenizer,
        &mut input,
        &ctx.atoms,
        &mut observer,
        config.finish_drain_budget.max(1),
    )? {
        return Ok(rejected_summary(
            input.as_str().len(),
            &observer,
            config.seed,
            bytes.len(),
            chunk_count,
            saw_one_byte_chunk,
            termination,
        ));
    }

    if !observer.saw_eof {
        return Err(TokenizerFuzzError::MissingEof);
    }

    Ok(TokenizerFuzzSummary {
        seed: config.seed,
        termination: TokenizerFuzzTermination::Completed,
        input_bytes: bytes.len(),
        decoded_bytes: input.as_str().len(),
        chunk_count,
        saw_one_byte_chunk,
        tokens_observed: observer.tokens_observed,
        span_resolve_count: observer.span_resolve_count,
        digest: observer.digest,
    })
}

fn pump_phase(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    observer: &mut TokenObserver,
    controller: Option<&mut TextModeFuzzController>,
    phase: &'static str,
) -> Result<Option<TokenizerFuzzTermination>, TokenizerFuzzError> {
    match controller {
        Some(controller) => {
            pump_text_mode_until_blocked(tokenizer, input, ctx, observer, controller, phase)
        }
        None => pump_until_blocked(tokenizer, input, ctx, observer, phase),
    }
}
