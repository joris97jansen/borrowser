use super::Html5Tokenizer;
use super::control::TextModeKind;
use super::machine::Step;
use super::states::TokenizerState;
use crate::html5::shared::{DocumentParseContext, ErrorOrigin, Input, ParseError, ParseErrorCode};

/// External-progress-only stall threshold.
///
/// The current tokenizer state machine is expected not to require this many
/// consecutive `Step::Progress` returns that neither consume input nor queue a
/// token in any legitimate path. If future states add a longer bounded run of
/// internal-only progress, this threshold should be retuned alongside that
/// state-machine change rather than treated as a normal control-flow budget.
pub(crate) const MAX_CONSECUTIVE_STALLED_PROGRESS_STEPS: usize = 8;
const STALL_RECOVERY_DETAIL: &str = "tokenizer-stall-recovery";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StallResponseMode {
    Panic,
    Recover,
}

impl StallResponseMode {
    pub(crate) fn for_current_build() -> Self {
        if cfg!(any(debug_assertions, feature = "parser_invariants", test)) {
            Self::Panic
        } else {
            Self::Recover
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DetectedStepStall {
    pub(crate) consecutive_steps: usize,
    pub(crate) state: TokenizerState,
    pub(crate) cursor: usize,
    pub(crate) queued_tokens: usize,
}

impl std::fmt::Display for DetectedStepStall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tokenizer stalled for {} consecutive progress steps without consuming input or emitting tokens: state={:?} cursor={} queued_tokens={}",
            self.consecutive_steps, self.state, self.cursor, self.queued_tokens
        )
    }
}

impl Html5Tokenizer {
    pub(crate) fn detect_stalled_progress_step(
        &self,
        before_cursor: usize,
        before_tokens: usize,
        step_result: Step,
        consecutive_stalled_steps: &mut usize,
    ) -> Option<DetectedStepStall> {
        // This guardrail is intentionally keyed to external progress only:
        // repeated state-machine bookkeeping without input consumption or token
        // queue growth is treated as suspicious, even if internal witnesses
        // such as `progress_epoch` keep changing.
        let stalled_progress = matches!(step_result, Step::Progress)
            && self.cursor == before_cursor
            && self.tokens.len() == before_tokens;
        if stalled_progress {
            *consecutive_stalled_steps = consecutive_stalled_steps.saturating_add(1);
        } else {
            *consecutive_stalled_steps = 0;
        }
        if *consecutive_stalled_steps >= MAX_CONSECUTIVE_STALLED_PROGRESS_STEPS {
            Some(DetectedStepStall {
                consecutive_steps: *consecutive_stalled_steps,
                state: self.state,
                cursor: self.cursor,
                queued_tokens: self.tokens.len(),
            })
        } else {
            None
        }
    }

    pub(crate) fn handle_detected_step_stall(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        stall: DetectedStepStall,
        mode: StallResponseMode,
    ) -> Step {
        match mode {
            StallResponseMode::Panic => panic!("{stall}"),
            StallResponseMode::Recover => self.recover_from_detected_step_stall(input, ctx, stall),
        }
    }

    fn recover_from_detected_step_stall(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        stall: DetectedStepStall,
    ) -> Step {
        ctx.record_error(ParseError {
            origin: ErrorOrigin::Tokenizer,
            code: ParseErrorCode::ImplementationGuardrail,
            position: stall.cursor,
            detail: Some(STALL_RECOVERY_DETAIL),
            aux: Some(stall.consecutive_steps.min(u32::MAX as usize) as u32),
        });

        self.pending_text_mode_end_tag_matcher = None;
        self.pending_text_mode_end_tag = None;
        self.pending_comment_start = None;
        self.pending_comment_limit_reported = false;
        self.pending_doctype_name = None;
        self.pending_doctype_name_start = None;
        self.pending_doctype_public_id = None;
        self.pending_doctype_system_id = None;
        self.pending_doctype_force_quirks = false;
        self.pending_doctype_limit_reported = false;
        self.tag_name_start = None;
        self.tag_name_end = None;
        self.tag_name_complete = false;
        self.current_tag_is_end = false;
        self.current_tag_self_closing = false;
        self.current_tag_attrs.clear();
        self.current_attr_name_start = None;
        self.current_attr_name_end = None;
        self.current_attr_has_value = false;
        self.current_attr_value_start = None;
        self.current_attr_value_end = None;
        self.end_tag_prefix_consumed = false;

        let target_state = match self.active_text_mode.map(|mode| mode.kind) {
            Some(TextModeKind::RawText) => TokenizerState::RawText,
            Some(TextModeKind::Rcdata) => TokenizerState::Rcdata,
            Some(TextModeKind::ScriptData) => TokenizerState::ScriptData,
            None => TokenizerState::Data,
        };
        self.transition_to(target_state);

        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        if self.pending_text_start.is_none() {
            self.pending_text_start = Some(self.cursor);
        }
        let _ = self.consume(input);
        Step::Progress
    }

    #[cfg(test)]
    pub(crate) fn inject_step_stall_for_test(&mut self, steps: usize) {
        self.test_forced_stall_steps_remaining = steps;
    }

    #[cfg(test)]
    pub(crate) fn recover_from_step_stall_for_test(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        consecutive_steps: usize,
    ) -> Step {
        self.handle_detected_step_stall(
            input,
            ctx,
            DetectedStepStall {
                consecutive_steps,
                state: self.state,
                cursor: self.cursor,
                queued_tokens: self.tokens.len(),
            },
            StallResponseMode::Recover,
        )
    }
}
