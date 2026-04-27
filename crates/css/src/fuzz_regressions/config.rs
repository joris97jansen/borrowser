use super::tool::CssFuzzRegressionProfile;
use crate::cascade::StyleResolutionLimits;
use crate::cascade::fuzz::CssCascadeFuzzConfig;
use crate::computed::fuzz::CssValueFuzzConfig;
use crate::selectors::SelectorMatchingLimits;
use crate::selectors::fuzz::{SelectorMatchingFuzzConfig, SelectorParserFuzzConfig};
use crate::syntax::{CssParserFuzzConfig, CssTokenizerFuzzConfig};

pub(super) fn tokenizer_config(seed: u64) -> CssTokenizerFuzzConfig {
    CssTokenizerFuzzConfig {
        seed,
        ..CssTokenizerFuzzConfig::default()
    }
}

pub(super) fn parser_config(seed: u64) -> CssParserFuzzConfig {
    CssParserFuzzConfig {
        seed,
        ..CssParserFuzzConfig::default()
    }
}

pub(super) fn selector_parser_config(seed: u64) -> SelectorParserFuzzConfig {
    SelectorParserFuzzConfig {
        seed,
        ..SelectorParserFuzzConfig::default()
    }
}

pub(super) fn selector_matching_config(
    seed: u64,
    profile: CssFuzzRegressionProfile,
) -> SelectorMatchingFuzzConfig {
    let mut config = SelectorMatchingFuzzConfig {
        seed,
        ..SelectorMatchingFuzzConfig::default()
    };

    if matches!(profile, CssFuzzRegressionProfile::SelectorLimitZero) {
        config.matching_limits = SelectorMatchingLimits {
            max_axis_steps_per_match: 0,
        };
    }

    config
}

pub(super) fn cascade_config(seed: u64, profile: CssFuzzRegressionProfile) -> CssCascadeFuzzConfig {
    let mut config = CssCascadeFuzzConfig {
        seed,
        ..CssCascadeFuzzConfig::default()
    };

    if matches!(profile, CssFuzzRegressionProfile::SelectorLimitZero) {
        config.style_resolution_limits = StyleResolutionLimits {
            selector_matching: SelectorMatchingLimits {
                max_axis_steps_per_match: 0,
            },
            ..StyleResolutionLimits::default()
        };
    }

    config
}

pub(super) fn values_config(seed: u64) -> CssValueFuzzConfig {
    CssValueFuzzConfig {
        seed,
        ..CssValueFuzzConfig::default()
    }
}
