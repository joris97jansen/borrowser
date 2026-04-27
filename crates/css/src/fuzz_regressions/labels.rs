use crate::cascade::fuzz::CssCascadeFuzzTermination;
use crate::computed::fuzz::CssValueFuzzTermination;
use crate::selectors::fuzz::{SelectorMatchingFuzzTermination, SelectorParserFuzzTermination};
use crate::syntax::{CssParserFuzzTermination, CssTokenizerFuzzTermination};

pub(super) fn bool_label(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub(super) fn tokenizer_termination_label(value: CssTokenizerFuzzTermination) -> &'static str {
    match value {
        CssTokenizerFuzzTermination::Completed => "completed",
        CssTokenizerFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        CssTokenizerFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
        CssTokenizerFuzzTermination::RejectedMaxTokensObserved => "rejected-max-tokens-observed",
        CssTokenizerFuzzTermination::RejectedMaxDiagnosticsObserved => {
            "rejected-max-diagnostics-observed"
        }
    }
}

pub(super) fn parser_termination_label(value: CssParserFuzzTermination) -> &'static str {
    match value {
        CssParserFuzzTermination::Completed => "completed",
        CssParserFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        CssParserFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
        CssParserFuzzTermination::RejectedMaxRulesObserved => "rejected-max-rules-observed",
        CssParserFuzzTermination::RejectedMaxDeclarationsObserved => {
            "rejected-max-declarations-observed"
        }
        CssParserFuzzTermination::RejectedMaxComponentValuesObserved => {
            "rejected-max-component-values-observed"
        }
        CssParserFuzzTermination::RejectedMaxDiagnosticsObserved => {
            "rejected-max-diagnostics-observed"
        }
    }
}

pub(super) fn selector_parser_termination_label(
    value: SelectorParserFuzzTermination,
) -> &'static str {
    match value {
        SelectorParserFuzzTermination::Completed => "completed",
        SelectorParserFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        SelectorParserFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
        SelectorParserFuzzTermination::RejectedMaxSelectorCases => "rejected-max-selector-cases",
    }
}

pub(super) fn selector_matching_termination_label(
    value: SelectorMatchingFuzzTermination,
) -> &'static str {
    match value {
        SelectorMatchingFuzzTermination::Completed => "completed",
        SelectorMatchingFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        SelectorMatchingFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
        SelectorMatchingFuzzTermination::RejectedMaxSelectorCases => "rejected-max-selector-cases",
        SelectorMatchingFuzzTermination::RejectedMaxElementsObserved => {
            "rejected-max-elements-observed"
        }
        SelectorMatchingFuzzTermination::SelectorMatchingLimitExceeded => {
            "selector-matching-limit-exceeded"
        }
    }
}

pub(super) fn cascade_termination_label(value: CssCascadeFuzzTermination) -> &'static str {
    match value {
        CssCascadeFuzzTermination::Completed => "completed",
        CssCascadeFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        CssCascadeFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
        CssCascadeFuzzTermination::RejectedMaxStylesheetCases => "rejected-max-stylesheet-cases",
        CssCascadeFuzzTermination::RejectedMaxResolvedElementsObserved => {
            "rejected-max-resolved-elements-observed"
        }
        CssCascadeFuzzTermination::RejectedMaxComputedElementsObserved => {
            "rejected-max-computed-elements-observed"
        }
        CssCascadeFuzzTermination::StyleResolutionLimitExceeded => {
            "style-resolution-limit-exceeded"
        }
        CssCascadeFuzzTermination::SelectorMatchingLimitExceeded => {
            "selector-matching-limit-exceeded"
        }
        CssCascadeFuzzTermination::ComputedNormalizationError => "computed-normalization-error",
    }
}

pub(super) fn value_termination_label(value: CssValueFuzzTermination) -> &'static str {
    match value {
        CssValueFuzzTermination::Completed => "completed",
        CssValueFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        CssValueFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
    }
}
