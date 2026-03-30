use super::super::{
    Html5Tokenizer, TextModeSpec, TokenizeResult, TokenizerConfig, TokenizerControl,
};
use super::config::{
    MIN_PUMP_BUDGET, PUMP_BUDGET_FACTOR, TokenizerFuzzConfig, TokenizerFuzzError,
    TokenizerFuzzSummary, TokenizerFuzzTermination,
};
use super::observe::{ObserveError, TokenObserver};
use super::progress::{PumpDecision, ensure_pump_progress};
use super::rng::{HarnessRng, next_chunk_len};
use crate::html5::shared::{AtomTable, ByteStreamDecoder, DocumentParseContext, Input, Token};

/// Run a single deterministic byte-stream fuzz case against the HTML5 tokenizer.
///
/// Contract:
/// - bytes are decoded incrementally with UTF-8 carry + U+FFFD replacement,
/// - chunks are randomized from `seed` but reproducible,
/// - tokens are drained immediately and never accumulated,
/// - every emitted span is resolved before the batch is dropped, and
/// - the driver fails if pumping can no longer make observable progress.
///
/// In `parser_invariants`/debug/test builds, the driver also relies on the
/// tokenizer's internal stall guardrail so repeated non-consuming `Progress`
/// steps fail fast instead of hanging the harness.
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

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into RAWTEXT mode for a `<style>`-family element.
pub fn run_seeded_rawtext_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::RawTextStyle,
    )
}

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into RCDATA mode for a `<title>`-family element.
pub fn run_seeded_title_rcdata_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::RcdataTitle,
    )
}

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into RCDATA mode for a `<textarea>`-family element.
pub fn run_seeded_textarea_rcdata_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::RcdataTextarea,
    )
}

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into script-data text mode.
///
/// This bypasses unrelated tree-builder/parsing setup and exercises the script
/// text-mode machinery itself, including resumable `</script>` detection,
/// escaped-script transitions, and chunk-boundary behavior under incremental
/// UTF-8 decoding.
///
/// This driver intentionally depends on the tokenizer's token-granular pump
/// contract: when the token queue is drained between `push_input_until_token()`
/// calls, `next_batch()` returns at most one newly emitted token before the
/// tokenizer yields. That contract keeps control application aligned to true
/// token boundaries instead of incidental batch sizing.
pub fn run_seeded_script_data_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::ScriptData,
    )
}

fn run_seeded_controlled_text_mode_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
    mode: TargetedTextModeHarnessKind,
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
    let tag_name = ctx
        .atoms
        .intern_ascii_folded(mode.end_tag_name_literal())
        .expect("text-mode atom interning must succeed");
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut controller = TextModeFuzzController::new(mode.spec(tag_name));
    controller.enter_initial(&mut tokenizer);
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
        if let Some(termination) = pump_text_mode_until_blocked(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut observer,
            &mut controller,
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
        if let Some(termination) = pump_text_mode_until_blocked(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut observer,
            &mut controller,
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

fn pump_text_mode_until_blocked(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
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

fn finish_and_drain(
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetedTextModeHarnessKind {
    RawTextStyle,
    RcdataTitle,
    RcdataTextarea,
    ScriptData,
}

impl TargetedTextModeHarnessKind {
    fn end_tag_name_literal(self) -> &'static str {
        match self {
            Self::RawTextStyle => "style",
            Self::RcdataTitle => "title",
            Self::RcdataTextarea => "textarea",
            Self::ScriptData => "script",
        }
    }

    fn spec(self, tag_name: crate::html5::shared::AtomId) -> TextModeSpec {
        match self {
            Self::RawTextStyle => TextModeSpec::rawtext_style(tag_name),
            Self::RcdataTitle => TextModeSpec::rcdata_title(tag_name),
            Self::RcdataTextarea => TextModeSpec::rcdata_textarea(tag_name),
            Self::ScriptData => TextModeSpec::script_data(tag_name),
        }
    }
}

struct TextModeFuzzController {
    spec: TextModeSpec,
    text_mode_active: bool,
}

impl TextModeFuzzController {
    fn new(spec: TextModeSpec) -> Self {
        Self {
            spec,
            text_mode_active: false,
        }
    }

    fn enter_initial(&mut self, tokenizer: &mut Html5Tokenizer) {
        assert!(
            !self.text_mode_active,
            "text-mode fuzz controller cannot enter initial mode twice"
        );
        tokenizer.apply_control(TokenizerControl::EnterTextMode(self.spec));
        self.text_mode_active = true;
        self.assert_consistent(tokenizer);
    }

    fn note_token(&mut self, token: &Token) -> Option<TokenizerControl> {
        match token {
            Token::StartTag { name, .. }
                if *name == self.spec.end_tag_name && !self.text_mode_active =>
            {
                self.text_mode_active = true;
                Some(TokenizerControl::EnterTextMode(self.spec))
            }
            Token::EndTag { name } if *name == self.spec.end_tag_name && self.text_mode_active => {
                self.text_mode_active = false;
                Some(TokenizerControl::ExitTextMode)
            }
            _ => None,
        }
    }

    fn assert_consistent(&self, tokenizer: &Html5Tokenizer) {
        let tokenizer_in_expected_text_mode = tokenizer.active_text_mode == Some(self.spec);
        assert_eq!(
            tokenizer_in_expected_text_mode, self.text_mode_active,
            "text-mode fuzz controller drifted from tokenizer text-mode state"
        );
    }
}
