use crate::cascade::StyleResolutionLimits;
use crate::cascade::fuzz::{
    CssCascadeFuzzConfig, CssCascadeFuzzSummary, CssCascadeFuzzTermination,
    run_seeded_cascade_fuzz_case,
};
use crate::computed::fuzz::{
    CssValueFuzzConfig, CssValueFuzzSummary, CssValueFuzzTermination, run_seeded_value_fuzz_case,
};
use crate::selectors::SelectorMatchingLimits;
use crate::selectors::fuzz::{
    SelectorMatchingFuzzConfig, SelectorMatchingFuzzSummary, SelectorMatchingFuzzTermination,
    SelectorParserFuzzConfig, SelectorParserFuzzSummary, SelectorParserFuzzTermination,
    run_seeded_selector_matching_fuzz_case, run_seeded_selector_parser_fuzz_case,
};
use crate::syntax::{
    CssParserFuzzConfig, CssParserFuzzSummary, CssParserFuzzTermination, CssTokenizerFuzzConfig,
    CssTokenizerFuzzSummary, CssTokenizerFuzzTermination, run_seeded_parser_fuzz_case,
    run_seeded_tokenizer_fuzz_case,
};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssFuzzRegressionTool {
    Tokenizer,
    Parser,
    SelectorParser,
    SelectorMatching,
    Cascade,
    Values,
}

impl CssFuzzRegressionTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tokenizer => "css_tokenizer",
            Self::Parser => "css_parser",
            Self::SelectorParser => "css_selector_parser",
            Self::SelectorMatching => "css_selector_matching",
            Self::Cascade => "css_cascade",
            Self::Values => "css_values",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "css_tokenizer" => Some(Self::Tokenizer),
            "css_parser" => Some(Self::Parser),
            "css_selector_parser" => Some(Self::SelectorParser),
            "css_selector_matching" => Some(Self::SelectorMatching),
            "css_cascade" => Some(Self::Cascade),
            "css_values" => Some(Self::Values),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssFuzzRegressionProfile {
    Default,
    SelectorLimitZero,
}

impl CssFuzzRegressionProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::SelectorLimitZero => "selector-limit-zero",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "selector-limit-zero" => Some(Self::SelectorLimitZero),
            _ => None,
        }
    }
}

pub fn render_css_fuzz_regression_summary(
    tool: CssFuzzRegressionTool,
    bytes: &[u8],
    seed: u64,
) -> Result<String, String> {
    render_css_fuzz_regression_summary_with_profile(
        tool,
        CssFuzzRegressionProfile::Default,
        bytes,
        seed,
    )
}

pub fn render_css_fuzz_regression_summary_with_profile(
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    bytes: &[u8],
    seed: u64,
) -> Result<String, String> {
    match tool {
        CssFuzzRegressionTool::Tokenizer => {
            let summary = match run_seeded_tokenizer_fuzz_case(bytes, tokenizer_config(seed)) {
                Ok(summary) => summary,
                Err(err) => return Err(err.to_string()),
            };
            Ok(render_tokenizer_summary(tool, profile, &summary))
        }
        CssFuzzRegressionTool::Parser => {
            let summary = match run_seeded_parser_fuzz_case(bytes, parser_config(seed)) {
                Ok(summary) => summary,
                Err(err) => return Err(err.to_string()),
            };
            Ok(render_parser_summary(tool, profile, &summary))
        }
        CssFuzzRegressionTool::SelectorParser => {
            let summary = run_seeded_selector_parser_fuzz_case(bytes, selector_parser_config(seed))
                .map_err(|err| err.to_string())?;
            Ok(render_selector_parser_summary(tool, profile, &summary))
        }
        CssFuzzRegressionTool::SelectorMatching => {
            let summary = run_seeded_selector_matching_fuzz_case(
                bytes,
                selector_matching_config(seed, profile),
            )
            .map_err(|err| err.to_string())?;
            Ok(render_selector_matching_summary(tool, profile, &summary))
        }
        CssFuzzRegressionTool::Cascade => {
            let summary = run_seeded_cascade_fuzz_case(bytes, cascade_config(seed, profile))
                .map_err(|err| err.to_string())?;
            Ok(render_cascade_summary(tool, profile, &summary))
        }
        CssFuzzRegressionTool::Values => {
            let summary = run_seeded_value_fuzz_case(bytes, values_config(seed))
                .map_err(|err| err.to_string())?;
            Ok(render_values_summary(tool, profile, &summary))
        }
    }
}

fn tokenizer_config(seed: u64) -> CssTokenizerFuzzConfig {
    CssTokenizerFuzzConfig {
        seed,
        ..CssTokenizerFuzzConfig::default()
    }
}

fn parser_config(seed: u64) -> CssParserFuzzConfig {
    CssParserFuzzConfig {
        seed,
        ..CssParserFuzzConfig::default()
    }
}

fn selector_parser_config(seed: u64) -> SelectorParserFuzzConfig {
    SelectorParserFuzzConfig {
        seed,
        ..SelectorParserFuzzConfig::default()
    }
}

fn selector_matching_config(
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

fn cascade_config(seed: u64, profile: CssFuzzRegressionProfile) -> CssCascadeFuzzConfig {
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

fn values_config(seed: u64) -> CssValueFuzzConfig {
    CssValueFuzzConfig {
        seed,
        ..CssValueFuzzConfig::default()
    }
}

fn render_tokenizer_summary(
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    summary: &CssTokenizerFuzzSummary,
) -> String {
    let mut out = header(tool, profile, summary.seed);
    line(
        &mut out,
        "termination",
        tokenizer_termination_label(summary.termination),
    );
    line(&mut out, "input-bytes", &summary.input_bytes.to_string());
    line(
        &mut out,
        "decoded-bytes",
        &summary.decoded_bytes.to_string(),
    );
    line(
        &mut out,
        "tokens-observed",
        &summary.tokens_observed.to_string(),
    );
    line(
        &mut out,
        "diagnostics-observed",
        &summary.diagnostics_observed.to_string(),
    );
    line(&mut out, "hit-limit", bool_label(summary.hit_limit));
    line(&mut out, "digest", &summary.digest.to_string());
    out
}

fn render_parser_summary(
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    summary: &CssParserFuzzSummary,
) -> String {
    let mut out = header(tool, profile, summary.seed);
    line(
        &mut out,
        "termination",
        parser_termination_label(summary.termination),
    );
    line(&mut out, "input-bytes", &summary.input_bytes.to_string());
    line(
        &mut out,
        "decoded-bytes",
        &summary.decoded_bytes.to_string(),
    );
    line(
        &mut out,
        "rules-observed",
        &summary.rules_observed.to_string(),
    );
    line(
        &mut out,
        "declarations-observed",
        &summary.declarations_observed.to_string(),
    );
    line(
        &mut out,
        "component-values-observed",
        &summary.component_values_observed.to_string(),
    );
    line(
        &mut out,
        "diagnostics-observed",
        &summary.diagnostics_observed.to_string(),
    );
    line(&mut out, "hit-limit", bool_label(summary.hit_limit));
    line(&mut out, "digest", &summary.digest.to_string());
    out
}

fn render_selector_parser_summary(
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    summary: &SelectorParserFuzzSummary,
) -> String {
    let mut out = header(tool, profile, summary.seed);
    line(
        &mut out,
        "termination",
        selector_parser_termination_label(summary.termination),
    );
    line(&mut out, "input-bytes", &summary.input_bytes.to_string());
    line(
        &mut out,
        "decoded-bytes",
        &summary.decoded_bytes.to_string(),
    );
    line(
        &mut out,
        "selector-cases-observed",
        &summary.selector_cases_observed.to_string(),
    );
    line(&mut out, "parsed-cases", &summary.parsed_cases.to_string());
    line(
        &mut out,
        "unsupported-cases",
        &summary.unsupported_cases.to_string(),
    );
    line(
        &mut out,
        "invalid-cases",
        &summary.invalid_cases.to_string(),
    );
    line(
        &mut out,
        "resource-limit-invalid-cases",
        &summary.resource_limit_invalid_cases.to_string(),
    );
    line(&mut out, "digest", &summary.digest.to_string());
    out
}

fn render_selector_matching_summary(
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    summary: &SelectorMatchingFuzzSummary,
) -> String {
    let mut out = header(tool, profile, summary.seed);
    line(
        &mut out,
        "termination",
        selector_matching_termination_label(summary.termination),
    );
    line(&mut out, "input-bytes", &summary.input_bytes.to_string());
    line(
        &mut out,
        "decoded-bytes",
        &summary.decoded_bytes.to_string(),
    );
    line(
        &mut out,
        "selector-cases-observed",
        &summary.selector_cases_observed.to_string(),
    );
    line(
        &mut out,
        "elements-observed",
        &summary.elements_observed.to_string(),
    );
    line(&mut out, "parsed-cases", &summary.parsed_cases.to_string());
    line(
        &mut out,
        "unsupported-cases",
        &summary.unsupported_cases.to_string(),
    );
    line(
        &mut out,
        "invalid-cases",
        &summary.invalid_cases.to_string(),
    );
    line(
        &mut out,
        "matched-targets-observed",
        &summary.matched_targets_observed.to_string(),
    );
    line(
        &mut out,
        "unmatched-targets-observed",
        &summary.unmatched_targets_observed.to_string(),
    );
    line(
        &mut out,
        "unsupported-targets-observed",
        &summary.unsupported_targets_observed.to_string(),
    );
    line(
        &mut out,
        "invalid-targets-observed",
        &summary.invalid_targets_observed.to_string(),
    );
    line(
        &mut out,
        "limit-errors-observed",
        &summary.limit_errors_observed.to_string(),
    );
    line(&mut out, "digest", &summary.digest.to_string());
    out
}

fn render_cascade_summary(
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    summary: &CssCascadeFuzzSummary,
) -> String {
    let mut out = header(tool, profile, summary.seed);
    line(
        &mut out,
        "termination",
        cascade_termination_label(summary.termination),
    );
    line(&mut out, "input-bytes", &summary.input_bytes.to_string());
    line(
        &mut out,
        "decoded-bytes",
        &summary.decoded_bytes.to_string(),
    );
    line(
        &mut out,
        "stylesheet-cases-observed",
        &summary.stylesheet_cases_observed.to_string(),
    );
    line(
        &mut out,
        "resolved-elements-observed",
        &summary.resolved_elements_observed.to_string(),
    );
    line(
        &mut out,
        "computed-elements-observed",
        &summary.computed_elements_observed.to_string(),
    );
    line(&mut out, "digest", &summary.digest.to_string());
    out
}

fn render_values_summary(
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    summary: &CssValueFuzzSummary,
) -> String {
    let mut out = header(tool, profile, summary.seed);
    line(
        &mut out,
        "termination",
        value_termination_label(summary.termination),
    );
    line(&mut out, "input-bytes", &summary.input_bytes.to_string());
    line(
        &mut out,
        "decoded-bytes",
        &summary.decoded_bytes.to_string(),
    );
    line(
        &mut out,
        "properties-observed",
        &summary.properties_observed.to_string(),
    );
    line(
        &mut out,
        "value-cases-observed",
        &summary.value_cases_observed.to_string(),
    );
    line(
        &mut out,
        "missing-declaration-value-cases",
        &summary.missing_declaration_value_cases.to_string(),
    );
    line(
        &mut out,
        "specified-ok-cases",
        &summary.specified_ok_cases.to_string(),
    );
    line(
        &mut out,
        "specified-error-cases",
        &summary.specified_error_cases.to_string(),
    );
    line(
        &mut out,
        "computed-ok-cases",
        &summary.computed_ok_cases.to_string(),
    );
    line(
        &mut out,
        "computed-error-cases",
        &summary.computed_error_cases.to_string(),
    );
    line(&mut out, "digest", &summary.digest.to_string());
    out
}

fn header(tool: CssFuzzRegressionTool, profile: CssFuzzRegressionProfile, seed: u64) -> String {
    let mut out = String::new();
    writeln!(&mut out, "version: 1").expect("write regression summary");
    writeln!(&mut out, "css-fuzz-regression").expect("write regression summary");
    writeln!(&mut out, "tool: {}", tool.as_str()).expect("write regression summary");
    writeln!(&mut out, "profile: {}", profile.as_str()).expect("write regression summary");
    writeln!(&mut out, "seed: {seed}").expect("write regression summary");
    out
}

fn line(out: &mut String, key: &str, value: &str) {
    writeln!(out, "{key}: {value}").expect("write regression summary");
}

fn bool_label(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn tokenizer_termination_label(value: CssTokenizerFuzzTermination) -> &'static str {
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

fn parser_termination_label(value: CssParserFuzzTermination) -> &'static str {
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

fn selector_parser_termination_label(value: SelectorParserFuzzTermination) -> &'static str {
    match value {
        SelectorParserFuzzTermination::Completed => "completed",
        SelectorParserFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        SelectorParserFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
        SelectorParserFuzzTermination::RejectedMaxSelectorCases => "rejected-max-selector-cases",
    }
}

fn selector_matching_termination_label(value: SelectorMatchingFuzzTermination) -> &'static str {
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

fn cascade_termination_label(value: CssCascadeFuzzTermination) -> &'static str {
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

fn value_termination_label(value: CssValueFuzzTermination) -> &'static str {
    match value {
        CssValueFuzzTermination::Completed => "completed",
        CssValueFuzzTermination::RejectedMaxInputBytes => "rejected-max-input-bytes",
        CssValueFuzzTermination::RejectedMaxDecodedBytes => "rejected-max-decoded-bytes",
    }
}
