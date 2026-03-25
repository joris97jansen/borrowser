use super::super::{Html5Tokenizer, TokenizeResult, TokenizerConfig};
use super::config::{
    MIN_PUMP_BUDGET, PUMP_BUDGET_FACTOR, TokenizerFuzzConfig, TokenizerFuzzError,
    TokenizerFuzzSummary, TokenizerFuzzTermination,
};
use super::observe::{ObserveError, TokenObserver};
use super::progress::{PumpDecision, ensure_pump_progress};
use super::rng::{HarnessRng, next_chunk_len};
use crate::html5::shared::{AtomTable, ByteStreamDecoder, DocumentParseContext, Input};

/// Run a single deterministic byte-stream fuzz case against the HTML5 tokenizer.
///
/// Contract:
/// - bytes are decoded incrementally with UTF-8 carry + U+FFFD replacement,
/// - chunks are randomized from `seed` but reproducible,
/// - tokens are drained immediately and never accumulated,
/// - every emitted span is resolved before the batch is dropped, and
/// - the driver fails if pumping can no longer make observable progress.
pub fn run_seeded_byte_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(TokenizerFuzzSummary {
            seed: config.seed,
            termination: TokenizerFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            decoded_bytes: 0,
            chunk_count: 0,
            saw_one_byte_chunk: false,
            tokens_observed: 0,
            span_resolve_count: 0,
            digest: 0,
        });
    }

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut decoder = ByteStreamDecoder::new();
    let mut input = Input::new();
    let mut rng = HarnessRng::new(config.seed);
    let mut observer = TokenObserver::new(config.max_tokens_observed);
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
                &input,
                &observer,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                TokenizerFuzzTermination::RejectedMaxDecodedBytes,
            ));
        }
        if let Some(termination) = pump_until_blocked(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut observer,
            "streaming",
        )? {
            return Ok(rejected_summary(
                &input,
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
                &input,
                &observer,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                TokenizerFuzzTermination::RejectedMaxDecodedBytes,
            ));
        }
        if let Some(termination) = pump_until_blocked(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut observer,
            "decoder-finish",
        )? {
            return Ok(rejected_summary(
                &input,
                &observer,
                config.seed,
                bytes.len(),
                chunk_count,
                saw_one_byte_chunk,
                termination,
            ));
        }
    }

    if let Some(termination) = finish_and_drain(
        &mut tokenizer,
        &mut input,
        &ctx.atoms,
        &mut observer,
        config.finish_drain_budget.max(1),
    )? {
        return Ok(rejected_summary(
            &input,
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

fn pump_until_blocked(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    observer: &mut TokenObserver,
    phase: &'static str,
) -> Result<Option<TokenizerFuzzTermination>, TokenizerFuzzError> {
    if let Err(source) = tokenizer.check_invariants(input) {
        return Err(TokenizerFuzzError::InvariantViolation {
            phase,
            pump_index: 0,
            detail: source.to_string(),
        });
    }
    let budget = phase_pump_budget(input.as_str().len().saturating_sub(tokenizer.cursor));
    for pump_index in 0..budget {
        let before = tokenizer.capture_invariant_snapshot();
        let result = tokenizer.push_input_until_token(input, ctx);
        let drain = drain_queued_tokens(tokenizer, input, &ctx.atoms, observer, phase, pump_index)?;
        if let Some(termination) = drain.termination {
            return Ok(Some(termination));
        }
        if let Err(source) = tokenizer.check_invariants(input) {
            return Err(TokenizerFuzzError::InvariantViolation {
                phase,
                pump_index,
                detail: source.to_string(),
            });
        }
        let after = tokenizer.capture_invariant_snapshot();
        if let PumpDecision::Fail(err) = ensure_pump_progress(
            phase,
            pump_index,
            result,
            before,
            after,
            drain.drained_tokens,
        ) {
            return Err(err);
        }
        if result == TokenizeResult::NeedMoreInput {
            return Ok(None);
        }
    }

    Err(TokenizerFuzzError::PumpBudgetExceeded {
        phase,
        budget,
        cursor: tokenizer.cursor,
        queued_tokens: tokenizer.tokens.len(),
        detail: format!("state={:?}", tokenizer.state),
    })
}

fn finish_and_drain(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    atoms: &AtomTable,
    observer: &mut TokenObserver,
    budget: usize,
) -> Result<Option<TokenizerFuzzTermination>, TokenizerFuzzError> {
    let _ = tokenizer.finish(input);
    if let Err(source) = tokenizer.check_invariants(input) {
        return Err(TokenizerFuzzError::InvariantViolation {
            phase: "tokenizer-finish",
            pump_index: 0,
            detail: source.to_string(),
        });
    }
    for drain_index in 0..budget {
        let drain = drain_queued_tokens(
            tokenizer,
            input,
            atoms,
            observer,
            "tokenizer-finish",
            drain_index,
        )?;
        if let Some(termination) = drain.termination {
            return Ok(Some(termination));
        }
        if drain.drained_tokens == 0 || observer.saw_eof {
            return Ok(None);
        }
    }

    Err(TokenizerFuzzError::PumpBudgetExceeded {
        phase: "tokenizer-finish",
        budget,
        cursor: tokenizer.cursor,
        queued_tokens: tokenizer.tokens.len(),
        detail: format!("state={:?}", tokenizer.state),
    })
}

fn drain_queued_tokens(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    atoms: &AtomTable,
    observer: &mut TokenObserver,
    phase: &'static str,
    pump_index: usize,
) -> Result<DrainResult, TokenizerFuzzError> {
    let batch = tokenizer.next_batch(input);
    if batch.tokens().is_empty() {
        return Ok(DrainResult {
            drained_tokens: 0,
            termination: None,
        });
    }

    let resolver = batch.resolver();
    let mut drained = 0usize;
    for token in batch.iter() {
        match observer.observe(token, atoms, &resolver) {
            Ok(()) => {}
            Err(ObserveError::TokenBudgetReached) => {
                return Ok(DrainResult {
                    drained_tokens: drained,
                    termination: Some(TokenizerFuzzTermination::RejectedMaxTokensObserved),
                });
            }
            Err(ObserveError::InvalidSpan(source)) => {
                return Err(TokenizerFuzzError::InvalidSpan {
                    phase,
                    pump_index,
                    source,
                });
            }
            Err(ObserveError::DuplicateEof) => return Err(TokenizerFuzzError::DuplicateEof),
        }
        drained = drained.saturating_add(1);
    }
    Ok(DrainResult {
        drained_tokens: drained,
        termination: None,
    })
}

fn phase_pump_budget(remaining_decoded_bytes: usize) -> usize {
    remaining_decoded_bytes
        .saturating_mul(PUMP_BUDGET_FACTOR)
        .saturating_add(MIN_PUMP_BUDGET)
}

fn rejected_summary(
    input: &Input,
    observer: &TokenObserver,
    seed: u64,
    input_bytes: usize,
    chunk_count: usize,
    saw_one_byte_chunk: bool,
    termination: TokenizerFuzzTermination,
) -> TokenizerFuzzSummary {
    TokenizerFuzzSummary {
        seed,
        termination,
        input_bytes,
        decoded_bytes: input.as_str().len(),
        chunk_count,
        saw_one_byte_chunk,
        tokens_observed: observer.tokens_observed,
        span_resolve_count: observer.span_resolve_count,
        digest: observer.digest,
    }
}

struct DrainResult {
    drained_tokens: usize,
    termination: Option<TokenizerFuzzTermination>,
}
