use super::super::config::TokenizerFuzzError;
use super::super::progress::{PumpDecision, ensure_pump_progress};
use crate::html5::tokenizer::states::TokenizerState;
use crate::html5::tokenizer::{TokenizeResult, TokenizerInvariantSnapshot};

#[test]
fn progress_guard_rejects_progress_without_cursor_or_tokens() {
    let before = TokenizerInvariantSnapshot {
        cursor: 7,
        queued_tokens: 0,
        state: TokenizerState::Data,
        end_of_stream: false,
        eof_emitted: false,
        progress_epoch: 11,
    };
    let after = before;
    let decision = ensure_pump_progress("streaming", 3, TokenizeResult::Progress, before, after, 0);
    let PumpDecision::Fail(err) = decision else {
        panic!("expected no-progress failure");
    };
    assert!(matches!(err, TokenizerFuzzError::NoProgress { .. }));
}

#[test]
fn progress_guard_accepts_state_only_progress() {
    let before = TokenizerInvariantSnapshot {
        cursor: 7,
        queued_tokens: 0,
        state: TokenizerState::Data,
        end_of_stream: false,
        eof_emitted: false,
        progress_epoch: 11,
    };
    let after = TokenizerInvariantSnapshot {
        state: TokenizerState::TagOpen,
        ..before
    };
    let decision = ensure_pump_progress("streaming", 1, TokenizeResult::Progress, before, after, 0);
    assert!(matches!(decision, PumpDecision::Ok));
}

#[test]
fn progress_guard_accepts_epoch_only_progress() {
    let before = TokenizerInvariantSnapshot {
        cursor: 7,
        queued_tokens: 0,
        state: TokenizerState::Data,
        end_of_stream: false,
        eof_emitted: false,
        progress_epoch: 11,
    };
    let after = TokenizerInvariantSnapshot {
        progress_epoch: 12,
        ..before
    };
    let decision = ensure_pump_progress("streaming", 2, TokenizeResult::Progress, before, after, 0);
    assert!(matches!(decision, PumpDecision::Ok));
}

#[test]
fn progress_guard_rejects_need_more_input_after_observable_progress() {
    let before = TokenizerInvariantSnapshot {
        cursor: 7,
        queued_tokens: 0,
        state: TokenizerState::Data,
        end_of_stream: false,
        eof_emitted: false,
        progress_epoch: 11,
    };
    let after = TokenizerInvariantSnapshot {
        state: TokenizerState::TagOpen,
        ..before
    };
    let decision = ensure_pump_progress(
        "streaming",
        4,
        TokenizeResult::NeedMoreInput,
        before,
        after,
        0,
    );
    let PumpDecision::Fail(err) = decision else {
        panic!("expected invariant violation for mismatched NeedMoreInput");
    };
    assert!(matches!(err, TokenizerFuzzError::InvariantViolation { .. }));
}
