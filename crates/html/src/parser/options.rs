use crate::html5::shared::ErrorPolicy;
use crate::html5::tokenizer::{TokenizerConfig, TokenizerLimits};
use crate::html5::tree_builder::{TreeBuilderConfig, TreeBuilderLimits};

/// Controls parse-error tracking on the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlErrorPolicy {
    pub track: bool,
    pub max_stored: usize,
    pub debug_only: bool,
    pub track_counters: bool,
}

impl Default for HtmlErrorPolicy {
    fn default() -> Self {
        let policy = ErrorPolicy::default();
        Self {
            track: policy.track,
            max_stored: policy.max_stored,
            debug_only: policy.debug_only,
            track_counters: policy.track_counters,
        }
    }
}

impl From<HtmlErrorPolicy> for ErrorPolicy {
    fn from(value: HtmlErrorPolicy) -> Self {
        Self {
            track: value.track,
            max_stored: value.max_stored,
            debug_only: value.debug_only,
            track_counters: value.track_counters,
        }
    }
}

/// Tokenizer resource limits for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTokenizerLimits {
    pub max_tokens_per_batch: usize,
    pub max_tag_name_bytes: usize,
    pub max_attribute_name_bytes: usize,
    pub max_attribute_value_bytes: usize,
    pub max_attributes_per_tag: usize,
    pub max_comment_bytes: usize,
    pub max_processing_instruction_target_bytes: usize,
    pub max_processing_instruction_data_bytes: usize,
    pub max_doctype_bytes: usize,
    pub max_end_tag_match_scan_bytes: usize,
}

impl Default for HtmlTokenizerLimits {
    fn default() -> Self {
        let limits = TokenizerLimits::default();
        Self {
            max_tokens_per_batch: limits.max_tokens_per_batch,
            max_tag_name_bytes: limits.max_tag_name_bytes,
            max_attribute_name_bytes: limits.max_attribute_name_bytes,
            max_attribute_value_bytes: limits.max_attribute_value_bytes,
            max_attributes_per_tag: limits.max_attributes_per_tag,
            max_comment_bytes: limits.max_comment_bytes,
            max_processing_instruction_target_bytes: limits.max_processing_instruction_target_bytes,
            max_processing_instruction_data_bytes: limits.max_processing_instruction_data_bytes,
            max_doctype_bytes: limits.max_doctype_bytes,
            max_end_tag_match_scan_bytes: limits.max_end_tag_match_scan_bytes,
        }
    }
}

impl From<HtmlTokenizerLimits> for TokenizerLimits {
    fn from(value: HtmlTokenizerLimits) -> Self {
        Self {
            max_tokens_per_batch: value.max_tokens_per_batch,
            max_tag_name_bytes: value.max_tag_name_bytes,
            max_attribute_name_bytes: value.max_attribute_name_bytes,
            max_attribute_value_bytes: value.max_attribute_value_bytes,
            max_attributes_per_tag: value.max_attributes_per_tag,
            max_comment_bytes: value.max_comment_bytes,
            max_processing_instruction_target_bytes: value.max_processing_instruction_target_bytes,
            max_processing_instruction_data_bytes: value.max_processing_instruction_data_bytes,
            max_doctype_bytes: value.max_doctype_bytes,
            max_end_tag_match_scan_bytes: value.max_end_tag_match_scan_bytes,
        }
    }
}

/// Tokenizer configuration for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTokenizerOptions {
    pub emit_eof: bool,
    pub limits: HtmlTokenizerLimits,
}

impl Default for HtmlTokenizerOptions {
    fn default() -> Self {
        let config = TokenizerConfig::default();
        Self {
            emit_eof: config.emit_eof,
            limits: HtmlTokenizerLimits::default(),
        }
    }
}

impl From<HtmlTokenizerOptions> for TokenizerConfig {
    fn from(value: HtmlTokenizerOptions) -> Self {
        Self {
            emit_eof: value.emit_eof,
            limits: value.limits.into(),
        }
    }
}

/// Tree-builder resource limits for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTreeBuilderLimits {
    pub max_open_elements_depth: usize,
    pub max_nodes_created: usize,
    pub max_children_per_node: usize,
}

impl Default for HtmlTreeBuilderLimits {
    fn default() -> Self {
        let limits = TreeBuilderLimits::default();
        Self {
            max_open_elements_depth: limits.max_open_elements_depth,
            max_nodes_created: limits.max_nodes_created,
            max_children_per_node: limits.max_children_per_node,
        }
    }
}

impl From<HtmlTreeBuilderLimits> for TreeBuilderLimits {
    fn from(value: HtmlTreeBuilderLimits) -> Self {
        Self {
            max_open_elements_depth: value.max_open_elements_depth,
            max_nodes_created: value.max_nodes_created,
            max_children_per_node: value.max_children_per_node,
        }
    }
}

/// Tree-builder configuration for the stable parser facade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlTreeBuilderOptions {
    pub coalesce_text: bool,
    pub limits: HtmlTreeBuilderLimits,
}

impl Default for HtmlTreeBuilderOptions {
    fn default() -> Self {
        let config = TreeBuilderConfig::default();
        Self {
            coalesce_text: config.coalesce_text,
            limits: HtmlTreeBuilderLimits::default(),
        }
    }
}

impl From<HtmlTreeBuilderOptions> for TreeBuilderConfig {
    fn from(value: HtmlTreeBuilderOptions) -> Self {
        Self {
            coalesce_text: value.coalesce_text,
            limits: value.limits.into(),
        }
    }
}

/// Stable options for one-shot and streaming HTML parsing.
#[derive(Clone, Debug, Default)]
pub struct HtmlParseOptions {
    pub tokenizer: HtmlTokenizerOptions,
    pub tree_builder: HtmlTreeBuilderOptions,
    pub error_policy: HtmlErrorPolicy,
}
