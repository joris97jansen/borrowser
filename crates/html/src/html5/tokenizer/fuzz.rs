use super::{Html5Tokenizer, TextResolveError, TextResolver, TokenizeResult, TokenizerConfig};
use crate::html5::shared::{
    AtomId, AtomTable, AttributeValue, ByteStreamDecoder, DocumentParseContext, Input, TextValue,
    Token,
};

const DEFAULT_MAX_CHUNK_LEN: usize = 32;
const DEFAULT_MAX_INPUT_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_DECODED_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_TOKENS_OBSERVED: usize = 128 * 1024;
const MIN_PUMP_BUDGET: usize = 32;
const PUMP_BUDGET_FACTOR: usize = 8;
const DEFAULT_FINISH_DRAIN_BUDGET: usize = 32;

/// Stable seed derivation for byte-oriented fuzz cases.
///
/// This keeps randomized chunking reproducible for a given corpus entry without
/// requiring an out-of-band seed channel.
pub fn derive_fuzz_seed(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash ^ ((bytes.len() as u64).rotate_left(17))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenizerFuzzConfig {
    pub seed: u64,
    pub max_chunk_len: usize,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_tokens_observed: usize,
    pub finish_drain_budget: usize,
}

impl Default for TokenizerFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x54_6f_6b_65_6e_69_7a_72,
            max_chunk_len: DEFAULT_MAX_CHUNK_LEN,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_decoded_bytes: DEFAULT_MAX_DECODED_BYTES,
            max_tokens_observed: DEFAULT_MAX_TOKENS_OBSERVED,
            finish_drain_budget: DEFAULT_FINISH_DRAIN_BUDGET,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenizerFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
    RejectedMaxTokensObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenizerFuzzSummary {
    pub seed: u64,
    pub termination: TokenizerFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub chunk_count: usize,
    pub saw_one_byte_chunk: bool,
    pub tokens_observed: usize,
    pub span_resolve_count: usize,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenizerFuzzError {
    NoProgress {
        phase: &'static str,
        pump_index: usize,
        cursor: usize,
        queued_tokens: usize,
        detail: String,
    },
    PumpBudgetExceeded {
        phase: &'static str,
        budget: usize,
        cursor: usize,
        queued_tokens: usize,
        detail: String,
    },
    InvalidSpan {
        phase: &'static str,
        pump_index: usize,
        source: TextResolveError,
    },
    DuplicateEof,
    MissingEof,
}

impl std::fmt::Display for TokenizerFuzzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoProgress {
                phase,
                pump_index,
                cursor,
                queued_tokens,
                detail,
            } => write!(
                f,
                "tokenizer made no observable progress during {phase} pump={pump_index} cursor={cursor} queued_tokens={queued_tokens}: {detail}"
            ),
            Self::PumpBudgetExceeded {
                phase,
                budget,
                cursor,
                queued_tokens,
                detail,
            } => write!(
                f,
                "tokenizer exceeded harness pump budget during {phase} budget={budget} cursor={cursor} queued_tokens={queued_tokens}: {detail}"
            ),
            Self::InvalidSpan {
                phase,
                pump_index,
                source,
            } => write!(
                f,
                "tokenizer emitted an invalid span during {phase} pump={pump_index}: {source:?}"
            ),
            Self::DuplicateEof => f.write_str("tokenizer emitted duplicate EOF tokens"),
            Self::MissingEof => f.write_str("tokenizer never emitted EOF"),
        }
    }
}

impl std::error::Error for TokenizerFuzzError {}

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
    let budget = phase_pump_budget(input.as_str().len().saturating_sub(tokenizer.cursor));
    for pump_index in 0..budget {
        let before = PumpSnapshot::capture(tokenizer);
        let result = tokenizer.push_input_until_token(input, ctx);
        let drain = drain_queued_tokens(tokenizer, input, &ctx.atoms, observer, phase, pump_index)?;
        if let Some(termination) = drain.termination {
            return Ok(Some(termination));
        }
        let after = PumpSnapshot::capture(tokenizer);
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

fn next_chunk_len(
    remaining: usize,
    chunk_index: usize,
    max_chunk_len: usize,
    rng: &mut HarnessRng,
) -> usize {
    debug_assert!(remaining > 0);
    if chunk_index == 0 {
        return 1;
    }
    let upper = remaining.min(max_chunk_len);
    1 + rng.gen_range(upper)
}

fn phase_pump_budget(remaining_decoded_bytes: usize) -> usize {
    remaining_decoded_bytes
        .saturating_mul(PUMP_BUDGET_FACTOR)
        .saturating_add(MIN_PUMP_BUDGET)
}

fn ensure_pump_progress(
    phase: &'static str,
    pump_index: usize,
    result: TokenizeResult,
    before: PumpSnapshot,
    after: PumpSnapshot,
    drained_tokens: usize,
) -> PumpDecision {
    // Observable forward progress is broader than cursor advance or token drain
    // alone. The tokenizer may legitimately move forward by transitioning
    // states, toggling EOS/EOF flags, mutating queued-token readiness, or
    // incrementing its internal progress witness without immediately emitting a
    // token in the same pump.
    let made_progress = before.progress_epoch != after.progress_epoch
        || before.cursor != after.cursor
        || drained_tokens != 0
        || before.state != after.state
        || before.queued_tokens != after.queued_tokens
        || before.end_of_stream != after.end_of_stream
        || before.eof_emitted != after.eof_emitted;
    if made_progress || result == TokenizeResult::NeedMoreInput {
        return PumpDecision::Ok;
    }

    PumpDecision::Fail(TokenizerFuzzError::NoProgress {
        phase,
        pump_index,
        cursor: after.cursor,
        queued_tokens: after.queued_tokens,
        detail: format!(
            "result={result:?} state_before={} state_after={} epoch_before={} epoch_after={} queued_before={} queued_after={} eof_before={} eof_after={}",
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

enum PumpDecision {
    Ok,
    Fail(TokenizerFuzzError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PumpSnapshot {
    cursor: usize,
    queued_tokens: usize,
    state: &'static str,
    end_of_stream: bool,
    eof_emitted: bool,
    progress_epoch: u64,
}

impl PumpSnapshot {
    fn capture(tokenizer: &Html5Tokenizer) -> Self {
        Self {
            cursor: tokenizer.cursor,
            queued_tokens: tokenizer.tokens.len(),
            state: tokenizer.state.as_str(),
            end_of_stream: tokenizer.end_of_stream,
            eof_emitted: tokenizer.eof_emitted,
            progress_epoch: tokenizer.progress_epoch,
        }
    }
}

struct TokenObserver {
    max_tokens_observed: usize,
    saw_eof: bool,
    tokens_observed: usize,
    span_resolve_count: usize,
    digest: u64,
}

impl TokenObserver {
    fn new(max_tokens_observed: usize) -> Self {
        Self {
            max_tokens_observed: max_tokens_observed.max(1),
            saw_eof: false,
            tokens_observed: 0,
            span_resolve_count: 0,
            digest: 0,
        }
    }

    fn observe(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        resolver: &dyn TextResolver,
    ) -> Result<(), ObserveError> {
        if self.tokens_observed >= self.max_tokens_observed {
            return Err(ObserveError::TokenBudgetReached);
        }
        self.tokens_observed = self.tokens_observed.saturating_add(1);
        self.digest = mix_u64(self.digest, token_discriminant(token));
        match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                if let Some(name) = name {
                    self.digest = mix_atom_name(self.digest, atoms, *name);
                }
                if let Some(public_id) = public_id {
                    self.digest = mix_bytes(self.digest, public_id.as_bytes());
                }
                if let Some(system_id) = system_id {
                    self.digest = mix_bytes(self.digest, system_id.as_bytes());
                }
                self.digest = mix_u64(self.digest, u64::from(*force_quirks));
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                self.digest = mix_atom_name(self.digest, atoms, *name);
                self.digest = mix_u64(self.digest, u64::from(*self_closing));
                for attr in attrs {
                    self.digest = mix_atom_name(self.digest, atoms, attr.name);
                    if let Some(value) = &attr.value {
                        self.observe_attr_value(value, resolver)?;
                    }
                }
            }
            Token::EndTag { name } => {
                self.digest = mix_atom_name(self.digest, atoms, *name);
            }
            Token::Comment { text } | Token::Text { text } => {
                self.observe_text_value(text, resolver)?;
            }
            Token::Eof => {
                if self.saw_eof {
                    return Err(ObserveError::DuplicateEof);
                }
                self.saw_eof = true;
            }
        }
        Ok(())
    }

    fn observe_attr_value(
        &mut self,
        value: &AttributeValue,
        resolver: &dyn TextResolver,
    ) -> Result<(), ObserveError> {
        match value {
            AttributeValue::Span(span) => {
                let text = resolver
                    .resolve_span(*span)
                    .map_err(ObserveError::InvalidSpan)?;
                self.span_resolve_count = self.span_resolve_count.saturating_add(1);
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
            AttributeValue::Owned(text) => {
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
        }
        Ok(())
    }

    fn observe_text_value(
        &mut self,
        value: &TextValue,
        resolver: &dyn TextResolver,
    ) -> Result<(), ObserveError> {
        match value {
            TextValue::Span(span) => {
                let text = resolver
                    .resolve_span(*span)
                    .map_err(ObserveError::InvalidSpan)?;
                self.span_resolve_count = self.span_resolve_count.saturating_add(1);
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
            TextValue::Owned(text) => {
                self.digest = mix_bytes(self.digest, text.as_bytes());
            }
        }
        Ok(())
    }
}

enum ObserveError {
    InvalidSpan(TextResolveError),
    DuplicateEof,
    TokenBudgetReached,
}

struct DrainResult {
    drained_tokens: usize,
    termination: Option<TokenizerFuzzTermination>,
}

fn token_discriminant(token: &Token) -> u64 {
    match token {
        Token::Doctype { .. } => 1,
        Token::StartTag { .. } => 2,
        Token::EndTag { .. } => 3,
        Token::Comment { .. } => 4,
        Token::Text { .. } => 5,
        Token::Eof => 6,
    }
}

fn mix_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    if hash == 0 {
        hash = 0xcbf29ce484222325;
    }
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn mix_u64(hash: u64, value: u64) -> u64 {
    mix_bytes(hash, &value.to_le_bytes())
}

fn mix_atom_name(hash: u64, atoms: &AtomTable, id: AtomId) -> u64 {
    if let Some(name) = atoms.resolve(id) {
        return mix_bytes(hash, name.as_bytes());
    }
    mix_u64(hash, u64::from(id.0))
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

struct HarnessRng {
    state: u64,
}

impl HarnessRng {
    fn new(seed: u64) -> Self {
        let state = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    fn gen_range(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        (self.next_u64() as usize) % upper
    }
}

trait TokenizerStateDebugName {
    fn as_str(&self) -> &'static str;
}

impl TokenizerStateDebugName for super::states::TokenizerState {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Data => "Data",
            Self::RawText => "RawText",
            Self::Rcdata => "Rcdata",
            Self::ScriptData => "ScriptData",
            Self::ScriptDataEscaped => "ScriptDataEscaped",
            Self::ScriptDataEscapedDash => "ScriptDataEscapedDash",
            Self::ScriptDataEscapedDashDash => "ScriptDataEscapedDashDash",
            Self::ScriptDataDoubleEscaped => "ScriptDataDoubleEscaped",
            Self::ScriptDataDoubleEscapedDash => "ScriptDataDoubleEscapedDash",
            Self::ScriptDataDoubleEscapedDashDash => "ScriptDataDoubleEscapedDashDash",
            Self::TagOpen => "TagOpen",
            Self::EndTagOpen => "EndTagOpen",
            Self::TagName => "TagName",
            Self::BeforeAttributeName => "BeforeAttributeName",
            Self::AttributeName => "AttributeName",
            Self::AfterAttributeName => "AfterAttributeName",
            Self::BeforeAttributeValue => "BeforeAttributeValue",
            Self::AttributeValueDoubleQuoted => "AttributeValueDoubleQuoted",
            Self::AttributeValueSingleQuoted => "AttributeValueSingleQuoted",
            Self::AttributeValueUnquoted => "AttributeValueUnquoted",
            Self::AfterAttributeValueQuoted => "AfterAttributeValueQuoted",
            Self::SelfClosingStartTag => "SelfClosingStartTag",
            Self::MarkupDeclarationOpen => "MarkupDeclarationOpen",
            Self::CommentStart => "CommentStart",
            Self::CommentStartDash => "CommentStartDash",
            Self::Comment => "Comment",
            Self::CommentEndDash => "CommentEndDash",
            Self::CommentEnd => "CommentEnd",
            Self::BogusComment => "BogusComment",
            Self::Doctype => "Doctype",
            Self::BeforeDoctypeName => "BeforeDoctypeName",
            Self::DoctypeName => "DoctypeName",
            Self::AfterDoctypeName => "AfterDoctypeName",
            Self::BogusDoctype => "BogusDoctype",
            Self::CharacterReference => "CharacterReference",
            Self::NamedCharacterReference => "NamedCharacterReference",
            Self::AmbiguousAmpersand => "AmbiguousAmpersand",
            Self::NumericCharacterReference => "NumericCharacterReference",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PumpSnapshot, TokenizerFuzzConfig, TokenizerFuzzError, TokenizerFuzzTermination,
        derive_fuzz_seed, ensure_pump_progress, next_chunk_len, run_seeded_byte_fuzz_case,
    };
    use crate::html5::tokenizer::TokenizeResult;

    #[test]
    fn fuzz_seed_is_stable_for_same_bytes() {
        let bytes = b"<div>\xF0\x9F\x98\x80</div>";
        assert_eq!(derive_fuzz_seed(bytes), derive_fuzz_seed(bytes));
    }

    #[test]
    fn chunk_planner_is_seeded_and_starts_with_one_byte_chunk() {
        let mut rng_a = super::HarnessRng::new(0x1234);
        let mut rng_b = super::HarnessRng::new(0x1234);
        let mut remaining_a = 17usize;
        let mut remaining_b = 17usize;
        let mut sizes_a = Vec::new();
        let mut sizes_b = Vec::new();

        for chunk_index in 0..6 {
            let len_a = next_chunk_len(remaining_a, chunk_index, 8, &mut rng_a);
            let len_b = next_chunk_len(remaining_b, chunk_index, 8, &mut rng_b);
            sizes_a.push(len_a);
            sizes_b.push(len_b);
            remaining_a = remaining_a.saturating_sub(len_a);
            remaining_b = remaining_b.saturating_sub(len_b);
        }

        assert_eq!(sizes_a, sizes_b);
        assert_eq!(sizes_a.first().copied(), Some(1));
    }

    #[test]
    fn progress_guard_rejects_progress_without_cursor_or_tokens() {
        let before = PumpSnapshot {
            cursor: 7,
            queued_tokens: 0,
            state: "Data",
            end_of_stream: false,
            eof_emitted: false,
            progress_epoch: 11,
        };
        let after = before;
        let decision =
            ensure_pump_progress("streaming", 3, TokenizeResult::Progress, before, after, 0);
        let super::PumpDecision::Fail(err) = decision else {
            panic!("expected no-progress failure");
        };
        assert!(matches!(err, TokenizerFuzzError::NoProgress { .. }));
    }

    #[test]
    fn progress_guard_accepts_state_only_progress() {
        let before = PumpSnapshot {
            cursor: 7,
            queued_tokens: 0,
            state: "Data",
            end_of_stream: false,
            eof_emitted: false,
            progress_epoch: 11,
        };
        let after = PumpSnapshot {
            state: "TagOpen",
            ..before
        };
        let decision =
            ensure_pump_progress("streaming", 1, TokenizeResult::Progress, before, after, 0);
        assert!(matches!(decision, super::PumpDecision::Ok));
    }

    #[test]
    fn progress_guard_accepts_epoch_only_progress() {
        let before = PumpSnapshot {
            cursor: 7,
            queued_tokens: 0,
            state: "Data",
            end_of_stream: false,
            eof_emitted: false,
            progress_epoch: 11,
        };
        let after = PumpSnapshot {
            progress_epoch: 12,
            ..before
        };
        let decision =
            ensure_pump_progress("streaming", 2, TokenizeResult::Progress, before, after, 0);
        assert!(matches!(decision, super::PumpDecision::Ok));
    }

    #[test]
    fn seeded_byte_fuzz_harness_is_reproducible() {
        let bytes = b"<!DOCTYPE html><title>caf\xC3\xA9</title><!--x-->";
        let config = TokenizerFuzzConfig {
            seed: 0x4242,
            max_chunk_len: 7,
            ..TokenizerFuzzConfig::default()
        };
        let first = run_seeded_byte_fuzz_case(bytes, config).expect("first run should pass");
        let second = run_seeded_byte_fuzz_case(bytes, config).expect("second run should pass");
        assert_eq!(first, second);
        assert_eq!(first.termination, TokenizerFuzzTermination::Completed);
        assert!(first.saw_one_byte_chunk);
        assert!(first.tokens_observed > 0);
    }

    #[test]
    fn seeded_byte_fuzz_harness_handles_invalid_utf8() {
        let bytes = [0xFFu8, b'<', b'a', b'>', 0xC3];
        let summary = run_seeded_byte_fuzz_case(
            &bytes,
            TokenizerFuzzConfig {
                seed: 0x99,
                max_chunk_len: 4,
                ..TokenizerFuzzConfig::default()
            },
        )
        .expect("invalid UTF-8 case should remain recoverable");
        assert_eq!(summary.termination, TokenizerFuzzTermination::Completed);
        assert!(summary.decoded_bytes >= 3);
        assert!(summary.tokens_observed >= 1);
    }

    #[test]
    fn seeded_byte_fuzz_harness_rejects_inputs_above_explicit_limit() {
        let summary = run_seeded_byte_fuzz_case(
            b"0123456789",
            TokenizerFuzzConfig {
                seed: 0x55,
                max_input_bytes: 4,
                ..TokenizerFuzzConfig::default()
            },
        )
        .expect("oversized input should be rejected, not crash");
        assert_eq!(
            summary.termination,
            TokenizerFuzzTermination::RejectedMaxInputBytes
        );
        assert_eq!(summary.tokens_observed, 0);
    }
}
