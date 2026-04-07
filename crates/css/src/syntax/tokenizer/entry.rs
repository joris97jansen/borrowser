use super::super::{
    CssParseOrigin, DiagnosticKind, DiagnosticSeverity, ParseOptions, append_diagnostics,
    push_diagnostic, truncate_to_limit,
};
use super::engine::CssTokenizer;
use super::model::{CssTokenization, CssTokenizationStats};

pub fn tokenize_str(input: &str) -> CssTokenization {
    tokenize_str_with_options(input, &ParseOptions::stylesheet())
}

pub fn tokenize_str_with_options(input: &str, options: &ParseOptions) -> CssTokenization {
    let max_input_bytes = match options.origin {
        CssParseOrigin::Stylesheet => options.limits.max_stylesheet_input_bytes,
        CssParseOrigin::StyleAttribute => options.limits.max_declaration_list_input_bytes,
    };
    let bounded_input = truncate_to_limit(input, max_input_bytes);
    let mut tokenization = CssTokenization {
        input: super::super::CssInput::from(bounded_input),
        diagnostics: Vec::new(),
        stats: CssTokenizationStats {
            input_bytes: bounded_input.len(),
            ..CssTokenizationStats::default()
        },
        ..CssTokenization::default()
    };

    if bounded_input.len() != input.len() {
        tokenization.stats.hit_limit = true;
        push_tokenizer_diagnostic(
            options,
            &mut tokenization,
            DiagnosticSeverity::Error,
            DiagnosticKind::LimitExceeded,
            bounded_input.len(),
            format!(
                "tokenizer input truncated at {} bytes (limit {})",
                bounded_input.len(),
                max_input_bytes
            ),
        );
    }

    let mut tokenizer = CssTokenizer::new(&tokenization.input, options);
    tokenizer.tokenize_all();
    tokenization.tokens = tokenizer.tokens;
    tokenization.stats.tokens_emitted = tokenization.tokens.len();
    tokenization.stats.diagnostics_emitted += tokenizer.stats.diagnostics_emitted;
    tokenization.stats.hit_limit |= tokenizer.stats.hit_limit;
    append_diagnostics(
        options,
        &mut tokenization.diagnostics,
        tokenizer.diagnostics,
    );
    tokenization
}

pub(super) fn push_tokenizer_diagnostic(
    options: &ParseOptions,
    tokenization: &mut CssTokenization,
    severity: DiagnosticSeverity,
    kind: DiagnosticKind,
    byte_offset: usize,
    message: impl Into<String>,
) {
    let mut parse_stats = super::super::ParseStats::default();
    push_diagnostic(
        options,
        &mut tokenization.diagnostics,
        &mut parse_stats,
        severity,
        kind,
        byte_offset,
        message,
    );
    tokenization.stats.diagnostics_emitted += parse_stats.diagnostics_emitted;
}
