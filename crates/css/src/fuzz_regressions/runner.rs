use super::config::{
    cascade_config, parser_config, selector_matching_config, selector_parser_config,
    tokenizer_config, values_config,
};
use super::render::{
    render_cascade_summary, render_parser_summary, render_selector_matching_summary,
    render_selector_parser_summary, render_tokenizer_summary, render_values_summary,
};
use super::tool::{CssFuzzRegressionProfile, CssFuzzRegressionTool};
use crate::cascade::fuzz::run_seeded_cascade_fuzz_case;
use crate::computed::fuzz::run_seeded_value_fuzz_case;
use crate::selectors::fuzz::{
    run_seeded_selector_matching_fuzz_case, run_seeded_selector_parser_fuzz_case,
};
use crate::syntax::{run_seeded_parser_fuzz_case, run_seeded_tokenizer_fuzz_case};

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
            let summary = run_seeded_tokenizer_fuzz_case(bytes, tokenizer_config(seed))
                .map_err(|err| err.to_string())?;
            Ok(render_tokenizer_summary(tool, profile, &summary))
        }
        CssFuzzRegressionTool::Parser => {
            let summary = run_seeded_parser_fuzz_case(bytes, parser_config(seed))
                .map_err(|err| err.to_string())?;
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
