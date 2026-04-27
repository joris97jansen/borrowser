use super::config::CssSyntaxFuzzError;
use crate::syntax::parser::validate_token_stream_invariants;
use crate::syntax::{
    CssInput, CssRule, CssSpan, CssStylesheet, CssToken, ParseOptions, ParseStats, StylesheetParse,
};

pub(super) fn validate_css_token_stream(
    input: &CssInput,
    tokens: &[CssToken],
    options: &ParseOptions,
) -> Result<(), CssSyntaxFuzzError> {
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats::default();

    if validate_token_stream_invariants(options, input, tokens, 0, &mut diagnostics, &mut stats) {
        return Ok(());
    }

    let detail = diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| "token stream validation failed without a diagnostic".to_string());

    Err(CssSyntaxFuzzError::TokenStreamInvariantViolation { detail })
}

pub(super) fn ensure_parse_stats_consistent(
    parse: &StylesheetParse,
    phase: &'static str,
) -> Result<(), CssSyntaxFuzzError> {
    if parse.stats.input_bytes != parse.input.len_bytes() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.input_bytes={} but input.len_bytes()={}",
                parse.stats.input_bytes,
                parse.input.len_bytes()
            ),
        });
    }

    if parse.stats.rules_emitted != parse.stylesheet.rules.len() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.rules_emitted={} but stylesheet.rules.len()={}",
                parse.stats.rules_emitted,
                parse.stylesheet.rules.len()
            ),
        });
    }

    let declaration_count = count_stylesheet_declarations(&parse.stylesheet);
    if parse.stats.declarations_emitted != declaration_count {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.declarations_emitted={} but counted declarations={declaration_count}",
                parse.stats.declarations_emitted
            ),
        });
    }

    if parse.stats.diagnostics_emitted < parse.diagnostics.len() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "stats.diagnostics_emitted={} but diagnostics.len()={}",
                parse.stats.diagnostics_emitted,
                parse.diagnostics.len()
            ),
        });
    }

    Ok(())
}

fn count_stylesheet_declarations(stylesheet: &CssStylesheet) -> usize {
    stylesheet
        .rules
        .iter()
        .map(|rule| match rule {
            CssRule::Qualified(rule) => rule.block.declarations.len(),
            CssRule::At(_) => 0,
        })
        .sum()
}

pub(super) fn ensure_span_in_input(
    input: &CssInput,
    span: CssSpan,
    phase: &'static str,
) -> Result<(), CssSyntaxFuzzError> {
    if span.input_id != input.id() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: "span belongs to a different input".to_string(),
        });
    }

    if input.slice(span).is_none() {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "span @{}..{} is out of bounds or not on UTF-8 boundaries",
                span.start, span.end
            ),
        });
    }

    Ok(())
}

pub(super) fn ensure_span_within(
    input: &CssInput,
    outer: CssSpan,
    inner: CssSpan,
    phase: &'static str,
) -> Result<(), CssSyntaxFuzzError> {
    ensure_span_in_input(input, inner, phase)?;

    if outer.input_id != inner.input_id || inner.start < outer.start || inner.end > outer.end {
        return Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: format!(
                "child span @{}..{} escapes parent span @{}..{}",
                inner.start, inner.end, outer.start, outer.end
            ),
        });
    }

    Ok(())
}
