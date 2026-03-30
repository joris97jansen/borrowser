use super::super::Html5Tokenizer;
use super::super::control::TextModeKind;
use super::super::limits::LIMIT_DETAIL_END_TAG_MATCHER;
use super::super::machine::Step;
use super::super::scan::{IncrementalEndTagMatch, IncrementalEndTagMatcher};
use crate::html5::shared::{DocumentParseContext, ErrorOrigin, Input, ParseError, ParseErrorCode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextModeEndTagMatch {
    Matched {
        cursor_after: usize,
        name: crate::html5::shared::AtomId,
        had_attributes: bool,
        self_closing: bool,
    },
    LimitExceeded,
    NeedMoreInput(IncrementalEndTagMatcher),
    NoMatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingTextModeEndTag {
    pub(crate) cursor_after: usize,
    pub(crate) name: crate::html5::shared::AtomId,
}

impl Html5Tokenizer {
    pub(super) fn match_text_mode_end_tag(
        &mut self,
        input: &Input,
        expected_kind: TextModeKind,
        matcher: Option<IncrementalEndTagMatcher>,
    ) -> TextModeEndTagMatch {
        let Some(active_text_mode) = self.active_text_mode else {
            return TextModeEndTagMatch::NoMatch;
        };
        if active_text_mode.kind != expected_kind {
            return TextModeEndTagMatch::NoMatch;
        }
        let tag_name = active_text_mode.text_mode_end_tag_literal();
        let matcher = match matcher {
            Some(matcher) => {
                self.stats_inc_text_mode_end_tag_matcher_resumes();
                matcher
            }
            None => {
                self.stats_inc_text_mode_end_tag_matcher_starts();
                IncrementalEndTagMatcher::new(self.cursor)
            }
        };
        let mut progress_bytes = 0u64;
        let result = matcher.advance_counted_limited(
            input.as_str().as_bytes(),
            tag_name,
            &mut progress_bytes,
            self.max_end_tag_match_scan_bytes(),
        );
        self.stats_add_text_mode_end_tag_match_progress_bytes(progress_bytes);
        match result {
            IncrementalEndTagMatch::Matched {
                cursor_after,
                had_attributes,
                self_closing,
            } => TextModeEndTagMatch::Matched {
                cursor_after,
                name: active_text_mode.end_tag_name,
                had_attributes,
                self_closing,
            },
            IncrementalEndTagMatch::LimitExceeded => TextModeEndTagMatch::LimitExceeded,
            IncrementalEndTagMatch::NeedMoreInput(matcher) => {
                TextModeEndTagMatch::NeedMoreInput(matcher)
            }
            IncrementalEndTagMatch::NoMatch => TextModeEndTagMatch::NoMatch,
        }
    }

    pub(super) fn recover_from_text_mode_end_tag_limit(
        &mut self,
        ctx: &mut DocumentParseContext,
        input: &Input,
        less_than_pos: usize,
    ) -> Step {
        self.record_limit_error(
            ctx,
            less_than_pos,
            LIMIT_DETAIL_END_TAG_MATCHER,
            self.max_end_tag_match_scan_bytes(),
        );
        if self.pending_text_start.is_none() {
            self.pending_text_start = Some(self.cursor);
        }
        let _ = self.consume_if(input, '<');
        Step::Progress
    }

    pub(super) fn record_text_mode_end_tag_parse_errors(
        &mut self,
        ctx: &mut DocumentParseContext,
        position: usize,
        had_attributes: bool,
        self_closing: bool,
    ) {
        if had_attributes {
            ctx.record_error(ParseError {
                origin: ErrorOrigin::Tokenizer,
                code: ParseErrorCode::Other,
                position,
                detail: Some("text-mode-end-tag-attributes-ignored"),
                aux: None,
            });
        }
        if self_closing {
            ctx.record_error(ParseError {
                origin: ErrorOrigin::Tokenizer,
                code: ParseErrorCode::Other,
                position,
                detail: Some("text-mode-end-tag-self-closing-ignored"),
                aux: None,
            });
        }
    }
}
