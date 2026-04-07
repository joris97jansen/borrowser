use crate::html5::shared::{AtomTable, Input};
use crate::html5::tokenizer::fuzz::config::{TokenizerFuzzError, TokenizerFuzzTermination};
use crate::html5::tokenizer::fuzz::observe::{ObserveError, TokenObserver};
use crate::html5::tokenizer::fuzz::progress::{PumpDecision, ensure_pump_progress};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult};

use super::summary::phase_pump_budget;
use super::text_mode::TextModeFuzzController;

pub(super) fn pump_until_blocked(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut crate::html5::shared::DocumentParseContext,
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

pub(super) fn pump_text_mode_until_blocked(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut crate::html5::shared::DocumentParseContext,
    observer: &mut TokenObserver,
    controller: &mut TextModeFuzzController,
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
        let drain = drain_queued_tokens_with_text_mode_control(
            tokenizer, input, &ctx.atoms, observer, controller, phase, pump_index,
        )?;
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

pub(super) fn finish_and_drain(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    atoms: &AtomTable,
    observer: &mut TokenObserver,
    budget: usize,
) -> Result<Option<TokenizerFuzzTermination>, TokenizerFuzzError> {
    let _ = tokenizer.finish(input);
    // EOF finalization emits any remaining text plus EOF synchronously and no
    // further tokenizer work occurs after this point. Because there is no
    // subsequent pump whose semantics could be affected by text-mode control,
    // draining the remaining queue through the generic path is sufficient.
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

fn drain_queued_tokens_with_text_mode_control(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    atoms: &AtomTable,
    observer: &mut TokenObserver,
    controller: &mut TextModeFuzzController,
    phase: &'static str,
    pump_index: usize,
) -> Result<DrainResult, TokenizerFuzzError> {
    let mut pending_control = None;
    let mut drained = 0usize;
    {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            return Ok(DrainResult {
                drained_tokens: 0,
                termination: None,
            });
        }
        if batch.tokens().len() > 1 {
            return Err(TokenizerFuzzError::InvariantViolation {
                phase,
                pump_index,
                detail: format!(
                    "push_input_until_token contract violated: expected at most one newly emitted token, got {}",
                    batch.tokens().len()
                ),
            });
        }

        let resolver = batch.resolver();
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

            pending_control = controller.note_token(token);
            drained = drained.saturating_add(1);
        }
    }
    if let Some(control) = pending_control {
        tokenizer.apply_control(control);
        controller.assert_consistent(tokenizer);
    }
    Ok(DrainResult {
        drained_tokens: drained,
        termination: None,
    })
}

struct DrainResult {
    drained_tokens: usize,
    termination: Option<TokenizerFuzzTermination>,
}
