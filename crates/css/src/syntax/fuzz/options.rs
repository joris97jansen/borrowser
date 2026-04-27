use super::config::{CssParserFuzzConfig, CssTokenizerFuzzConfig};
use crate::syntax::{CssParseOrigin, ParseOptions, SyntaxLimits};

pub(super) fn tokenizer_fuzz_options(config: &CssTokenizerFuzzConfig) -> ParseOptions {
    ParseOptions {
        origin: CssParseOrigin::Stylesheet,
        limits: SyntaxLimits {
            max_stylesheet_input_bytes: config.max_decoded_bytes,
            max_lexical_tokens: config.max_tokens_observed.saturating_add(1).max(1),
            max_diagnostics: config.max_diagnostics_observed.max(1),
            ..SyntaxLimits::default()
        },
        ..ParseOptions::stylesheet()
    }
}

pub(super) fn parser_fuzz_options(config: &CssParserFuzzConfig) -> ParseOptions {
    let mut limits = config.syntax_limits.clone();
    limits.max_stylesheet_input_bytes = limits
        .max_stylesheet_input_bytes
        .min(config.max_decoded_bytes);
    limits.max_diagnostics = limits
        .max_diagnostics
        .min(config.max_diagnostics_observed.max(1));

    ParseOptions {
        origin: CssParseOrigin::Stylesheet,
        limits,
        ..ParseOptions::stylesheet()
    }
}
