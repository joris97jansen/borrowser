const DEFAULT_MAX_INPUT_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_TOKENS_GENERATED: usize = 8 * 1024;
const DEFAULT_MAX_ATTRS_PER_TAG: usize = 32;
const DEFAULT_MAX_TOTAL_ATTRS: usize = 4 * 1024;
const DEFAULT_MAX_STRING_BYTES_GENERATED: usize = 64 * 1024;
const DEFAULT_MAX_PATCHES_OBSERVED: usize = 64 * 1024;

pub fn derive_tree_builder_fuzz_seed(bytes: &[u8]) -> u64 {
    crate::html5::tokenizer::derive_fuzz_seed(bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeBuilderFuzzConfig {
    pub seed: u64,
    pub max_input_bytes: usize,
    pub max_tokens_generated: usize,
    pub max_attrs_per_tag: usize,
    pub max_total_attrs: usize,
    pub max_string_bytes_generated: usize,
    pub max_patches_observed: usize,
    pub max_processing_steps: usize,
}

impl Default for TreeBuilderFuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0x54_42_5f_46_55_5a_5a_32,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_tokens_generated: DEFAULT_MAX_TOKENS_GENERATED,
            max_attrs_per_tag: DEFAULT_MAX_ATTRS_PER_TAG,
            max_total_attrs: DEFAULT_MAX_TOTAL_ATTRS,
            max_string_bytes_generated: DEFAULT_MAX_STRING_BYTES_GENERATED,
            max_patches_observed: DEFAULT_MAX_PATCHES_OBSERVED,
            max_processing_steps: DEFAULT_MAX_TOKENS_GENERATED.saturating_add(1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeBuilderFuzzTermination {
    Completed,
    RejectedMaxInputBytes,
    RejectedMaxTokensGenerated,
    RejectedMaxAttributesGenerated,
    RejectedMaxStringBytesGenerated,
    RejectedMaxPatchesObserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeBuilderFuzzSummary {
    pub seed: u64,
    pub termination: TreeBuilderFuzzTermination,
    pub input_bytes: usize,
    pub tokens_generated: usize,
    pub attrs_generated: usize,
    pub string_bytes_generated: usize,
    pub patches_emitted: usize,
    pub tokenizer_controls_emitted: usize,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeBuilderFuzzError {
    DecodeFailure {
        token_index: usize,
        detail: String,
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
    ProcessingStepBudgetExceeded {
        budget: usize,
        processed_steps: usize,
        scheduled_steps: usize,
    },
}

impl std::fmt::Display for TreeBuilderFuzzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecodeFailure {
                token_index,
                detail,
            } => write!(
                f,
                "tree-builder fuzz token decode failed at synthetic token #{token_index}: {detail}"
            ),
            Self::TreeBuilderFailure {
                token_index,
                detail,
            } => write!(
                f,
                "tree builder returned an internal error at token #{token_index}: {detail}"
            ),
            Self::UnexpectedSuspend {
                token_index,
                reason,
            } => write!(
                f,
                "tree builder suspended unexpectedly at token #{token_index}: {reason:?}"
            ),
            Self::PatchInvariantViolation {
                token_index,
                detail,
            } => write!(
                f,
                "patch invariant violation after token #{token_index}: {detail}"
            ),
            Self::DomInvariantViolation {
                token_index,
                detail,
            } => write!(
                f,
                "DOM invariant violation after token #{token_index}: {detail}"
            ),
            Self::LiveStateMismatch { token_index } => write!(
                f,
                "live tree diverged from patch-derived state after token #{token_index}"
            ),
            Self::ProcessingStepBudgetExceeded {
                budget,
                processed_steps,
                scheduled_steps,
            } => write!(
                f,
                "tree-builder fuzz harness exceeded processing budget: budget={budget} processed_steps={processed_steps} scheduled_steps={scheduled_steps}"
            ),
        }
    }
}

impl std::error::Error for TreeBuilderFuzzError {}
