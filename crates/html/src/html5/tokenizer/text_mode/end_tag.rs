use super::super::Html5Tokenizer;
use super::super::control::TextModeKind;
use super::super::limits::LIMIT_DETAIL_END_TAG_MATCHER;
use super::super::machine::Step;
use super::super::scan::{IncrementalEndTagMatch, IncrementalEndTagMatcher};
use crate::html5::shared::{
    DocumentParseContext, Input, ParserResourceLimit, WhatwgParseErrorCode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextModeEndTagMatch {
    Matched {
        cursor_after: usize,
        name: crate::html5::shared::AtomId,
        attribute_error_position: Option<usize>,
        trailing_solidus_position: Option<usize>,
    },
    InvariantFailure,
    LimitExceeded,
    NeedMoreInputWithoutCandidate,
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
                if let Err(invariant) =
                    matcher.validate_live_candidate_range(input.as_str().as_bytes())
                {
                    self.latch_invariant(invariant);
                    return TextModeEndTagMatch::InvariantFailure;
                }
                if let Err(invariant) =
                    matcher.validate_live_diagnostic_evidence(input.as_str().as_bytes())
                {
                    self.latch_invariant(invariant);
                    return TextModeEndTagMatch::InvariantFailure;
                }
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
                attribute_error_position,
                trailing_solidus_position,
            } => TextModeEndTagMatch::Matched {
                cursor_after,
                name: active_text_mode.end_tag_name,
                attribute_error_position,
                trailing_solidus_position,
            },
            IncrementalEndTagMatch::InvariantFailure(invariant) => {
                self.latch_invariant(invariant);
                TextModeEndTagMatch::InvariantFailure
            }
            IncrementalEndTagMatch::LimitExceeded => TextModeEndTagMatch::LimitExceeded,
            IncrementalEndTagMatch::NeedMoreInput(matcher) => {
                if !matcher.has_complete_end_tag_opener(input.as_str().as_bytes()) {
                    TextModeEndTagMatch::NeedMoreInputWithoutCandidate
                } else if let Err(invariant) = matcher
                    .validate_live_candidate_range(input.as_str().as_bytes())
                    .and_then(|()| {
                        matcher.validate_live_diagnostic_evidence(input.as_str().as_bytes())
                    })
                {
                    self.latch_invariant(invariant);
                    TextModeEndTagMatch::InvariantFailure
                } else {
                    TextModeEndTagMatch::NeedMoreInput(matcher)
                }
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
            input,
            ctx,
            less_than_pos,
            ParserResourceLimit::EndTagMatchScanBytes,
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
        input: &Input,
        ctx: &mut DocumentParseContext,
        attribute_error_position: Option<usize>,
        trailing_solidus_position: Option<usize>,
    ) {
        if let Some(position) = attribute_error_position {
            self.record_tokenizer_parse_error_with_recovery(
                input,
                ctx,
                WhatwgParseErrorCode::EndTagWithAttributes,
                position,
                crate::html5::shared::ParserRecoveryAction::DropEndTagAttributes,
                super::super::normalization::legacy_diagnostic(
                    "text-mode-end-tag-attributes-ignored",
                    None,
                ),
            );
        }
        if let Some(position) = trailing_solidus_position {
            self.record_tokenizer_parse_error_with_recovery(
                input,
                ctx,
                WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                position,
                crate::html5::shared::ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                super::super::normalization::legacy_diagnostic(
                    "text-mode-end-tag-self-closing-ignored",
                    None,
                ),
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn force_text_mode_end_tag_evidence_for_test(
        &mut self,
        input: &Input,
        candidate_start: usize,
        cursor_after: usize,
        attribute_error_position: Option<usize>,
        trailing_solidus_position: Option<usize>,
    ) -> bool {
        match super::super::scan::validate_completed_text_mode_end_tag_evidence(
            input.as_str().as_bytes(),
            candidate_start,
            cursor_after,
            attribute_error_position,
            trailing_solidus_position,
        ) {
            Ok(()) => true,
            Err(invariant) => {
                self.latch_invariant(invariant);
                false
            }
        }
    }
}
