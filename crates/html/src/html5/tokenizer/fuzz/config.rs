const DEFAULT_MAX_CHUNK_LEN: usize = 32;
const DEFAULT_MAX_INPUT_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_DECODED_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_TOKENS_OBSERVED: usize = 128 * 1024;
pub(crate) const MIN_PUMP_BUDGET: usize = 32;
pub(crate) const PUMP_BUDGET_FACTOR: usize = 8;
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
        source: super::super::TextResolveError,
    },
    InvariantViolation {
        phase: &'static str,
        pump_index: usize,
        detail: String,
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
            Self::InvariantViolation {
                phase,
                pump_index,
                detail,
            } => write!(
                f,
                "tokenizer invariant violation during {phase} pump={pump_index}: {detail}"
            ),
            Self::DuplicateEof => f.write_str("tokenizer emitted duplicate EOF tokens"),
            Self::MissingEof => f.write_str("tokenizer never emitted EOF"),
        }
    }
}

impl std::error::Error for TokenizerFuzzError {}
