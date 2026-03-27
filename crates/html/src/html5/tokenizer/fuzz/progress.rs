use super::super::invariants::check_progress_contract;
use super::super::{TokenizeResult, TokenizerInvariantSnapshot};
use super::config::TokenizerFuzzError;

pub(crate) fn ensure_pump_progress(
    phase: &'static str,
    pump_index: usize,
    result: TokenizeResult,
    before: TokenizerInvariantSnapshot,
    after: TokenizerInvariantSnapshot,
    drained_tokens: usize,
) -> PumpDecision {
    if drained_tokens != 0 {
        return PumpDecision::Ok;
    }
    match check_progress_contract("pump", result, before, after) {
        Ok(()) => PumpDecision::Ok,
        Err(_source) if result == TokenizeResult::Progress => {
            PumpDecision::Fail(TokenizerFuzzError::NoProgress {
                phase,
                pump_index,
                cursor: after.cursor,
                queued_tokens: after.queued_tokens,
                detail: format!(
                    "result={result:?} state_before={:?} state_after={:?} epoch_before={} epoch_after={} queued_before={} queued_after={} eof_before={} eof_after={}",
                    before.state,
                    after.state,
                    before.progress_epoch,
                    after.progress_epoch,
                    before.queued_tokens,
                    after.queued_tokens,
                    before.eof_emitted,
                    after.eof_emitted
                ),
            })
        }
        Err(source) => PumpDecision::Fail(TokenizerFuzzError::InvariantViolation {
            phase,
            pump_index,
            detail: source.to_string(),
        }),
    }
}

pub(crate) enum PumpDecision {
    Ok,
    Fail(TokenizerFuzzError),
}
