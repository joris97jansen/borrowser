use crate::html5::tokenizer::fuzz::config::{
    MIN_PUMP_BUDGET, PUMP_BUDGET_FACTOR, TokenizerFuzzSummary, TokenizerFuzzTermination,
};
use crate::html5::tokenizer::fuzz::observe::TokenObserver;

pub(super) fn phase_pump_budget(remaining_decoded_bytes: usize) -> usize {
    remaining_decoded_bytes
        .saturating_mul(PUMP_BUDGET_FACTOR)
        .saturating_add(MIN_PUMP_BUDGET)
}

pub(super) fn rejected_summary(
    decoded_bytes: usize,
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
        decoded_bytes,
        chunk_count,
        saw_one_byte_chunk,
        tokens_observed: observer.tokens_observed,
        span_resolve_count: observer.span_resolve_count,
        digest: observer.digest,
    }
}
