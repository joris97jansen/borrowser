use super::Html5Tokenizer;
use super::api::TokenizerLimits;
use crate::html5::shared::{DocumentParseContext, ErrorOrigin, Input, ParseError, ParseErrorCode};

pub(super) const LIMIT_DETAIL_TOKEN_BATCH: &str = "token-batch-limit";
pub(super) const LIMIT_DETAIL_TAG_NAME: &str = "tag-name-truncated";
pub(super) const LIMIT_DETAIL_ATTRIBUTE_NAME: &str = "attribute-name-truncated";
pub(super) const LIMIT_DETAIL_ATTRIBUTE_VALUE: &str = "attribute-value-truncated";
pub(super) const LIMIT_DETAIL_ATTRIBUTES_PER_TAG: &str = "attributes-per-tag-limit";
pub(super) const LIMIT_DETAIL_COMMENT: &str = "comment-truncated";
pub(super) const LIMIT_DETAIL_DOCTYPE: &str = "doctype-limit";
pub(super) const LIMIT_DETAIL_END_TAG_MATCHER: &str = "end-tag-matcher-limit";

impl Html5Tokenizer {
    pub(in crate::html5::tokenizer) fn limits(&self) -> TokenizerLimits {
        self.config.limits
    }

    pub(in crate::html5::tokenizer) fn max_tokens_per_batch(&self) -> usize {
        self.limits().max_tokens_per_batch.max(1)
    }

    pub(in crate::html5::tokenizer) fn max_tag_name_bytes(&self) -> usize {
        self.limits().max_tag_name_bytes.max(1)
    }

    pub(in crate::html5::tokenizer) fn max_attribute_name_bytes(&self) -> usize {
        self.limits().max_attribute_name_bytes.max(1)
    }

    pub(in crate::html5::tokenizer) fn max_attribute_value_bytes(&self) -> usize {
        self.limits().max_attribute_value_bytes.max(1)
    }

    pub(in crate::html5::tokenizer) fn max_attributes_per_tag(&self) -> usize {
        // Unlike byte-oriented limits, zero retained attributes is an explicit
        // supported policy rather than an invalid configuration.
        self.limits().max_attributes_per_tag
    }

    pub(in crate::html5::tokenizer) fn max_comment_bytes(&self) -> usize {
        self.limits().max_comment_bytes.max(1)
    }

    pub(in crate::html5::tokenizer) fn max_doctype_bytes(&self) -> usize {
        self.limits().max_doctype_bytes.max(1)
    }

    pub(in crate::html5::tokenizer) fn max_end_tag_match_scan_bytes(&self) -> usize {
        self.limits().max_end_tag_match_scan_bytes.max(1)
    }

    pub(in crate::html5::tokenizer) fn record_limit_error(
        &self,
        ctx: &mut DocumentParseContext,
        position: usize,
        detail: &'static str,
        limit: usize,
    ) {
        ctx.record_error(ParseError {
            origin: ErrorOrigin::Tokenizer,
            code: ParseErrorCode::ResourceLimit,
            position,
            detail: Some(detail),
            aux: Some(limit.min(u32::MAX as usize) as u32),
        });
    }

    pub(in crate::html5::tokenizer) fn truncate_str_to_bytes<'a>(
        &self,
        raw: &'a str,
        max_bytes: usize,
    ) -> (&'a str, bool) {
        if raw.len() <= max_bytes {
            return (raw, false);
        }
        if max_bytes == 0 {
            return ("", true);
        }
        let mut end = 0usize;
        for (idx, ch) in raw.char_indices() {
            let next = idx + ch.len_utf8();
            if next > max_bytes {
                break;
            }
            end = next;
        }
        (&raw[..end], true)
    }

    pub(in crate::html5::tokenizer) fn truncate_input_range(
        &self,
        input: &Input,
        start: usize,
        end: usize,
        max_bytes: usize,
    ) -> Option<(usize, bool)> {
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            return None;
        }
        let raw = &input.as_str()[start..end];
        let (prefix, truncated) = self.truncate_str_to_bytes(raw, max_bytes);
        Some((start + prefix.len(), truncated))
    }
}
