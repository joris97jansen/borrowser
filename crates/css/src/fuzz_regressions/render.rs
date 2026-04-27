use super::labels::{
    bool_label, cascade_termination_label, parser_termination_label,
    selector_matching_termination_label, selector_parser_termination_label,
    tokenizer_termination_label, value_termination_label,
};
use super::tool::{CssFuzzRegressionProfile, CssFuzzRegressionTool};
use crate::cascade::fuzz::CssCascadeFuzzSummary;
use crate::computed::fuzz::CssValueFuzzSummary;
use crate::selectors::fuzz::{SelectorMatchingFuzzSummary, SelectorParserFuzzSummary};
use crate::syntax::{CssParserFuzzSummary, CssTokenizerFuzzSummary};
use std::fmt::Write;

pub(super) fn render_tokenizer_summary(
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

pub(super) fn render_parser_summary(
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

pub(super) fn render_selector_parser_summary(
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

pub(super) fn render_selector_matching_summary(
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

pub(super) fn render_cascade_summary(
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

pub(super) fn render_values_summary(
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
