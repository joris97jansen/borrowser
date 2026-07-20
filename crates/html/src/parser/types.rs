use crate::html5::Html5SessionError;
use crate::html5::shared::{
    Counters as Html5Counters, ErrorOrigin, ParseError as Html5ParseError, ParseErrorCode,
};

/// Stable origin classification for surfaced parse events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtmlParseEventOrigin {
    Tokenizer,
    TreeBuilder,
}

impl From<ErrorOrigin> for HtmlParseEventOrigin {
    fn from(value: ErrorOrigin) -> Self {
        match value {
            ErrorOrigin::Tokenizer => Self::Tokenizer,
            ErrorOrigin::TreeBuilder => Self::TreeBuilder,
        }
    }
}

/// Stable event code classification for surfaced parse events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtmlParseEventCode {
    UnexpectedNullCharacter,
    UnexpectedEof,
    InvalidCharacterReference,
    ResourceLimit,
    ImplementationGuardrail,
    Other,
}

impl From<ParseErrorCode> for HtmlParseEventCode {
    fn from(value: ParseErrorCode) -> Self {
        match value {
            ParseErrorCode::UnexpectedNullCharacter => Self::UnexpectedNullCharacter,
            ParseErrorCode::UnexpectedEof => Self::UnexpectedEof,
            ParseErrorCode::InvalidCharacterReference => Self::InvalidCharacterReference,
            ParseErrorCode::ResourceLimit => Self::ResourceLimit,
            ParseErrorCode::ImplementationGuardrail => Self::ImplementationGuardrail,
            ParseErrorCode::Other => Self::Other,
        }
    }
}

/// Stable engine-facing parse event record.
///
/// `detail` is diagnostic metadata and is not intended to be a hard stability
/// boundary for downstream decision logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HtmlParseEvent {
    pub origin: HtmlParseEventOrigin,
    pub code: HtmlParseEventCode,
    pub position: usize,
    pub detail: Option<&'static str>,
    pub aux: Option<u32>,
}

impl From<Html5ParseError> for HtmlParseEvent {
    fn from(value: Html5ParseError) -> Self {
        Self {
            origin: value.origin.into(),
            code: value.code.into(),
            position: value.position,
            detail: value.detail,
            aux: value.aux,
        }
    }
}

/// Stable parser counters surfaced by the HTML5-backed facade.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HtmlParseCounters {
    pub tokens_processed: u64,
    pub patches_emitted: u64,
    pub decode_errors: u64,
    pub adapter_invariant_violations: u64,
    pub tree_builder_invariant_errors: u64,
    pub parse_errors: u64,
    pub errors_dropped: u64,
    pub max_open_elements_depth: u32,
    pub max_active_formatting_depth: u32,
    pub soe_push_ops: u64,
    pub soe_pop_ops: u64,
    pub soe_scope_scan_calls: u64,
    pub soe_scope_scan_steps: u64,
    pub soe_name_count_lookup_calls: u64,
    pub soe_name_count_lookup_steps: u64,
    pub soe_name_count_update_calls: u64,
    pub soe_name_count_update_steps: u64,
    pub soe_distinct_name_high_water: u32,
    pub tree_builder_patches_emitted: u64,
    pub tree_builder_text_nodes_created: u64,
    pub tree_builder_text_appends: u64,
    pub tree_builder_text_coalescing_invalidations: u64,
}

impl From<Html5Counters> for HtmlParseCounters {
    fn from(value: Html5Counters) -> Self {
        Self {
            tokens_processed: value.tokens_processed,
            patches_emitted: value.patches_emitted,
            decode_errors: value.decode_errors,
            adapter_invariant_violations: value.adapter_invariant_violations,
            tree_builder_invariant_errors: value.tree_builder_invariant_errors,
            parse_errors: value.parse_errors,
            errors_dropped: value.errors_dropped,
            max_open_elements_depth: value.max_open_elements_depth,
            max_active_formatting_depth: value.max_active_formatting_depth,
            soe_push_ops: value.soe_push_ops,
            soe_pop_ops: value.soe_pop_ops,
            soe_scope_scan_calls: value.soe_scope_scan_calls,
            soe_scope_scan_steps: value.soe_scope_scan_steps,
            soe_name_count_lookup_calls: value.soe_name_count_lookup_calls,
            soe_name_count_lookup_steps: value.soe_name_count_lookup_steps,
            soe_name_count_update_calls: value.soe_name_count_update_calls,
            soe_name_count_update_steps: value.soe_name_count_update_steps,
            soe_distinct_name_high_water: value.soe_distinct_name_high_water,
            tree_builder_patches_emitted: value.tree_builder_patches_emitted,
            tree_builder_text_nodes_created: value.tree_builder_text_nodes_created,
            tree_builder_text_appends: value.tree_builder_text_appends,
            tree_builder_text_coalescing_invalidations: value
                .tree_builder_text_coalescing_invalidations,
        }
    }
}

/// Stable error surface for the engine-facing parser facade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HtmlParseError {
    Decode,
    /// Terminal parser-state violation, including use after a poisoned
    /// patch-mirror failure.
    Invariant,
    PatchValidation(String),
}

impl core::fmt::Display for HtmlParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HtmlParseError::Decode => write!(f, "decode error"),
            HtmlParseError::Invariant => write!(f, "engine invariant violation"),
            HtmlParseError::PatchValidation(detail) => {
                write!(f, "patch validation error: {detail}")
            }
        }
    }
}

impl std::error::Error for HtmlParseError {}

impl From<Html5SessionError> for HtmlParseError {
    fn from(value: Html5SessionError) -> Self {
        match value {
            Html5SessionError::Decode => Self::Decode,
            Html5SessionError::Invariant => Self::Invariant,
        }
    }
}
