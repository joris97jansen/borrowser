use std::num::NonZeroU16;

use super::{ParserGuardrail, ParserResourceLimit};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum HtmlParseSemanticDegradationReason {
    TagNameTruncated,
    AttributeNameTruncated,
    AttributeValueTruncated,
    AttributeDroppedByCountLimit,
    CommentTruncated,
    ProcessingInstructionTargetSuppressed,
    ProcessingInstructionDataTruncated,
    DoctypeBoundedRecovery,
    TextModeEndTagMatchAbandoned,
    NumericCharacterReferenceBoundedRecovery,
    TreeOpenElementsDepthSuppressed,
    TreeNodeCreationSuppressed,
    TreeChildInsertionSuppressed,
    TreeTemplateModeDepthSuppressed,
    TokenizerStallRecovery,
}

impl HtmlParseSemanticDegradationReason {
    const fn bit(self) -> u16 {
        match self {
            Self::TagNameTruncated => 1 << 0,
            Self::AttributeNameTruncated => 1 << 1,
            Self::AttributeValueTruncated => 1 << 2,
            Self::AttributeDroppedByCountLimit => 1 << 3,
            Self::CommentTruncated => 1 << 4,
            Self::ProcessingInstructionTargetSuppressed => 1 << 5,
            Self::ProcessingInstructionDataTruncated => 1 << 6,
            Self::DoctypeBoundedRecovery => 1 << 7,
            Self::TextModeEndTagMatchAbandoned => 1 << 8,
            Self::NumericCharacterReferenceBoundedRecovery => 1 << 9,
            Self::TreeOpenElementsDepthSuppressed => 1 << 10,
            Self::TreeNodeCreationSuppressed => 1 << 11,
            Self::TreeChildInsertionSuppressed => 1 << 12,
            Self::TreeTemplateModeDepthSuppressed => 1 << 13,
            Self::TokenizerStallRecovery => 1 << 14,
        }
    }

    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::TagNameTruncated => "tag-name-truncated",
            Self::AttributeNameTruncated => "attribute-name-truncated",
            Self::AttributeValueTruncated => "attribute-value-truncated",
            Self::AttributeDroppedByCountLimit => "attribute-dropped-by-count-limit",
            Self::CommentTruncated => "comment-truncated",
            Self::ProcessingInstructionTargetSuppressed => {
                "processing-instruction-target-suppressed"
            }
            Self::ProcessingInstructionDataTruncated => "processing-instruction-data-truncated",
            Self::DoctypeBoundedRecovery => "doctype-bounded-recovery",
            Self::TextModeEndTagMatchAbandoned => "text-mode-end-tag-match-abandoned",
            Self::NumericCharacterReferenceBoundedRecovery => {
                "numeric-character-reference-bounded-recovery"
            }
            Self::TreeOpenElementsDepthSuppressed => "tree-open-elements-depth-suppressed",
            Self::TreeNodeCreationSuppressed => "tree-node-creation-suppressed",
            Self::TreeChildInsertionSuppressed => "tree-child-insertion-suppressed",
            Self::TreeTemplateModeDepthSuppressed => "tree-template-mode-depth-suppressed",
            Self::TokenizerStallRecovery => "tokenizer-stall-recovery",
        }
    }
}

const ALL_REASONS: [HtmlParseSemanticDegradationReason; 15] = [
    HtmlParseSemanticDegradationReason::TagNameTruncated,
    HtmlParseSemanticDegradationReason::AttributeNameTruncated,
    HtmlParseSemanticDegradationReason::AttributeValueTruncated,
    HtmlParseSemanticDegradationReason::AttributeDroppedByCountLimit,
    HtmlParseSemanticDegradationReason::CommentTruncated,
    HtmlParseSemanticDegradationReason::ProcessingInstructionTargetSuppressed,
    HtmlParseSemanticDegradationReason::ProcessingInstructionDataTruncated,
    HtmlParseSemanticDegradationReason::DoctypeBoundedRecovery,
    HtmlParseSemanticDegradationReason::TextModeEndTagMatchAbandoned,
    HtmlParseSemanticDegradationReason::NumericCharacterReferenceBoundedRecovery,
    HtmlParseSemanticDegradationReason::TreeOpenElementsDepthSuppressed,
    HtmlParseSemanticDegradationReason::TreeNodeCreationSuppressed,
    HtmlParseSemanticDegradationReason::TreeChildInsertionSuppressed,
    HtmlParseSemanticDegradationReason::TreeTemplateModeDepthSuppressed,
    HtmlParseSemanticDegradationReason::TokenizerStallRecovery,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HtmlParseSemanticDegradations(NonZeroU16);

impl HtmlParseSemanticDegradations {
    pub const fn contains(self, reason: HtmlParseSemanticDegradationReason) -> bool {
        self.0.get() & reason.bit() != 0
    }

    pub fn len(self) -> usize {
        self.0.get().count_ones() as usize
    }

    pub const fn is_empty(self) -> bool {
        false
    }

    pub fn reasons(self) -> impl Iterator<Item = HtmlParseSemanticDegradationReason> {
        ALL_REASONS
            .into_iter()
            .filter(move |reason| self.0.get() & reason.bit() != 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HtmlParseSemanticCompleteness {
    Complete,
    Degraded(HtmlParseSemanticDegradations),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HtmlParseSemanticCompletenessTracker(u16);

impl HtmlParseSemanticCompletenessTracker {
    pub(crate) fn record(&mut self, reason: HtmlParseSemanticDegradationReason) {
        self.0 |= reason.bit();
    }

    pub(crate) fn status(self) -> HtmlParseSemanticCompleteness {
        match NonZeroU16::new(self.0) {
            Some(bits) => {
                HtmlParseSemanticCompleteness::Degraded(HtmlParseSemanticDegradations(bits))
            }
            None => HtmlParseSemanticCompleteness::Complete,
        }
    }
}

pub(crate) const fn resource_limit_degradation(
    limit: ParserResourceLimit,
) -> Option<HtmlParseSemanticDegradationReason> {
    match limit {
        ParserResourceLimit::TokenBatchCapacity => None,
        ParserResourceLimit::TagNameBytes => {
            Some(HtmlParseSemanticDegradationReason::TagNameTruncated)
        }
        ParserResourceLimit::AttributeNameBytes => {
            Some(HtmlParseSemanticDegradationReason::AttributeNameTruncated)
        }
        ParserResourceLimit::AttributeValueBytes => {
            Some(HtmlParseSemanticDegradationReason::AttributeValueTruncated)
        }
        ParserResourceLimit::AttributesPerTag => {
            Some(HtmlParseSemanticDegradationReason::AttributeDroppedByCountLimit)
        }
        ParserResourceLimit::CommentBytes => {
            Some(HtmlParseSemanticDegradationReason::CommentTruncated)
        }
        ParserResourceLimit::ProcessingInstructionTargetBytes => {
            Some(HtmlParseSemanticDegradationReason::ProcessingInstructionTargetSuppressed)
        }
        ParserResourceLimit::ProcessingInstructionDataBytes => {
            Some(HtmlParseSemanticDegradationReason::ProcessingInstructionDataTruncated)
        }
        ParserResourceLimit::DoctypeBytes => {
            Some(HtmlParseSemanticDegradationReason::DoctypeBoundedRecovery)
        }
        ParserResourceLimit::EndTagMatchScanBytes => {
            Some(HtmlParseSemanticDegradationReason::TextModeEndTagMatchAbandoned)
        }
        ParserResourceLimit::NumericCharacterReferenceDigits => {
            Some(HtmlParseSemanticDegradationReason::NumericCharacterReferenceBoundedRecovery)
        }
        ParserResourceLimit::TreeOpenElementsDepth => {
            Some(HtmlParseSemanticDegradationReason::TreeOpenElementsDepthSuppressed)
        }
        ParserResourceLimit::TreeNodeCount => {
            Some(HtmlParseSemanticDegradationReason::TreeNodeCreationSuppressed)
        }
        ParserResourceLimit::TreeChildrenPerNode => {
            Some(HtmlParseSemanticDegradationReason::TreeChildInsertionSuppressed)
        }
        ParserResourceLimit::TreeTemplateModeDepth => {
            Some(HtmlParseSemanticDegradationReason::TreeTemplateModeDepthSuppressed)
        }
    }
}

pub(crate) const fn guardrail_degradation(
    guardrail: ParserGuardrail,
) -> HtmlParseSemanticDegradationReason {
    match guardrail {
        ParserGuardrail::TokenizerStallRecovery => {
            HtmlParseSemanticDegradationReason::TokenizerStallRecovery
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_resource_limit_has_an_explicit_semantic_classification() {
        assert_eq!(
            resource_limit_degradation(ParserResourceLimit::TokenBatchCapacity),
            None
        );
        let degrading = [
            ParserResourceLimit::TagNameBytes,
            ParserResourceLimit::AttributeNameBytes,
            ParserResourceLimit::AttributeValueBytes,
            ParserResourceLimit::AttributesPerTag,
            ParserResourceLimit::CommentBytes,
            ParserResourceLimit::ProcessingInstructionTargetBytes,
            ParserResourceLimit::ProcessingInstructionDataBytes,
            ParserResourceLimit::DoctypeBytes,
            ParserResourceLimit::EndTagMatchScanBytes,
            ParserResourceLimit::NumericCharacterReferenceDigits,
            ParserResourceLimit::TreeOpenElementsDepth,
            ParserResourceLimit::TreeNodeCount,
            ParserResourceLimit::TreeChildrenPerNode,
            ParserResourceLimit::TreeTemplateModeDepth,
        ];
        for limit in degrading {
            let reason = resource_limit_degradation(limit)
                .unwrap_or_else(|| panic!("{limit:?} must have an explicit degradation reason"));
            let mut tracker = HtmlParseSemanticCompletenessTracker::default();
            tracker.record(reason);
            let HtmlParseSemanticCompleteness::Degraded(reasons) = tracker.status() else {
                panic!("{limit:?} must publish typed semantic degradation")
            };
            assert_eq!(reasons.reasons().collect::<Vec<_>>(), vec![reason]);
        }

        let guardrail_reason = guardrail_degradation(ParserGuardrail::TokenizerStallRecovery);
        let mut tracker = HtmlParseSemanticCompletenessTracker::default();
        tracker.record(guardrail_reason);
        assert!(matches!(
            tracker.status(),
            HtmlParseSemanticCompleteness::Degraded(reasons)
                if reasons.reasons().eq([guardrail_reason])
        ));
    }

    #[test]
    fn degradation_set_is_fixed_bounded_idempotent_and_canonical() {
        let mut first = HtmlParseSemanticCompletenessTracker::default();
        first.record(HtmlParseSemanticDegradationReason::TreeNodeCreationSuppressed);
        first.record(HtmlParseSemanticDegradationReason::TagNameTruncated);
        first.record(HtmlParseSemanticDegradationReason::TreeNodeCreationSuppressed);
        let mut second = HtmlParseSemanticCompletenessTracker::default();
        second.record(HtmlParseSemanticDegradationReason::TagNameTruncated);
        second.record(HtmlParseSemanticDegradationReason::TreeNodeCreationSuppressed);
        assert_eq!(first.status(), second.status());
        let HtmlParseSemanticCompleteness::Degraded(reasons) = first.status() else {
            panic!("expected degradation")
        };
        assert_eq!(reasons.len(), 2);
        assert!(!reasons.is_empty());
        assert!(reasons.contains(HtmlParseSemanticDegradationReason::TagNameTruncated));
        assert!(reasons.contains(HtmlParseSemanticDegradationReason::TreeNodeCreationSuppressed));
        assert_eq!(
            reasons.reasons().collect::<Vec<_>>(),
            vec![
                HtmlParseSemanticDegradationReason::TagNameTruncated,
                HtmlParseSemanticDegradationReason::TreeNodeCreationSuppressed,
            ]
        );
        assert_eq!(
            std::mem::size_of::<HtmlParseSemanticCompletenessTracker>(),
            2
        );
    }
}
