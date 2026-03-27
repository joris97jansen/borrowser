const DEFAULT_MAX_CHUNK_LEN: usize = 32;
const DEFAULT_MAX_INPUT_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_DECODED_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_TOKENS_STREAMED: usize = 128 * 1024;
const DEFAULT_MAX_PATCHES_OBSERVED: usize = 64 * 1024;
const DEFAULT_MAX_PIPELINE_STEPS: usize = 1_048_576;
const DEFAULT_MAX_TOKENS_WITHOUT_BUILDER_PROGRESS: usize = 32 * 1024;
const DEFAULT_FINISH_DRAIN_BUDGET: usize = 32;

pub fn derive_html5_pipeline_fuzz_seed(bytes: &[u8]) -> u64 {
    crate::html5::tokenizer::derive_fuzz_seed(bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Html5PipelineFuzzConfig {
    pub seed: u64,
    pub max_chunk_len: usize,
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_tokens_streamed: usize,
    pub max_patches_observed: usize,
    pub max_pipeline_steps: usize,
    pub max_tokens_without_builder_progress: usize,
    pub finish_drain_budget: usize,
}

impl Default for Html5PipelineFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x50_69_70_65_5f_46_55_5a,
            max_chunk_len: DEFAULT_MAX_CHUNK_LEN,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_decoded_bytes: DEFAULT_MAX_DECODED_BYTES,
            max_tokens_streamed: DEFAULT_MAX_TOKENS_STREAMED,
            max_patches_observed: DEFAULT_MAX_PATCHES_OBSERVED,
            max_pipeline_steps: DEFAULT_MAX_PIPELINE_STEPS,
            max_tokens_without_builder_progress: DEFAULT_MAX_TOKENS_WITHOUT_BUILDER_PROGRESS,
            finish_drain_budget: DEFAULT_FINISH_DRAIN_BUDGET,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Html5PipelineFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxDecodedBytes,
    RejectedMaxTokensStreamed,
    RejectedMaxPatchesObserved,
    RejectedMaxPipelineSteps,
    RejectedMaxBuilderNoProgressTokens,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Html5PipelineFuzzSummary {
    pub seed: u64,
    pub termination: Html5PipelineFuzzTermination,
    pub input_bytes: usize,
    pub decoded_bytes: usize,
    pub chunk_count: usize,
    pub saw_one_byte_chunk: bool,
    pub tokens_streamed: usize,
    pub span_resolve_count: usize,
    pub patches_emitted: usize,
    pub tokenizer_controls_applied: usize,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Html5PipelineFuzzError {
    Tokenizer(crate::html5::tokenizer::TokenizerFuzzError),
    NonTokenGranularBatch {
        phase: &'static str,
        pump_index: usize,
        batch_len: usize,
    },
    TreeBuilderFailure {
        token_index: usize,
        detail: String,
    },
    UnexpectedSuspend {
        token_index: usize,
        reason: crate::html5::tree_builder::SuspendReason,
    },
    PatchInvariantViolation {
        token_index: usize,
        detail: String,
    },
    DomInvariantViolation {
        token_index: usize,
        detail: String,
    },
    LiveStateMismatch {
        token_index: usize,
    },
}

impl From<crate::html5::tokenizer::TokenizerFuzzError> for Html5PipelineFuzzError {
    fn from(value: crate::html5::tokenizer::TokenizerFuzzError) -> Self {
        Self::Tokenizer(value)
    }
}

impl std::fmt::Display for Html5PipelineFuzzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tokenizer(source) => write!(f, "tokenizer-side pipeline fuzz failure: {source}"),
            Self::NonTokenGranularBatch {
                phase,
                pump_index,
                batch_len,
            } => write!(
                f,
                "tokenizer produced a non-token-granular batch during {phase} pump={pump_index}: batch_len={batch_len}"
            ),
            Self::TreeBuilderFailure {
                token_index,
                detail,
            } => write!(
                f,
                "tree builder returned an internal error at streamed token #{token_index}: {detail}"
            ),
            Self::UnexpectedSuspend {
                token_index,
                reason,
            } => write!(
                f,
                "tree builder suspended unexpectedly at streamed token #{token_index}: {reason:?}"
            ),
            Self::PatchInvariantViolation {
                token_index,
                detail,
            } => write!(
                f,
                "patch invariant violation after streamed token #{token_index}: {detail}"
            ),
            Self::DomInvariantViolation {
                token_index,
                detail,
            } => write!(
                f,
                "DOM invariant violation after streamed token #{token_index}: {detail}"
            ),
            Self::LiveStateMismatch { token_index } => write!(
                f,
                "live tree diverged from patch-derived state after streamed token #{token_index}"
            ),
        }
    }
}

impl std::error::Error for Html5PipelineFuzzError {}
