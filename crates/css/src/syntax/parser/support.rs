use super::super::input::CssInput;
use super::super::token::{CssToken, CssTokenKind};
use super::super::{
    DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats, SyntaxDiagnostic, push_diagnostic,
};
use super::model::{CssBlockKind, CssComponentValue};

/// Canonical tokenizer-to-parser boundary validator.
///
/// All structured parser entry points must use this function before consuming a
/// token stream. Future tokenizer modes or stitched token streams must reuse
/// this validator rather than introducing separate, slightly different
/// boundary checks.
pub(crate) fn validate_token_stream_invariants(
    options: &ParseOptions,
    input: &CssInput,
    tokens: &[CssToken],
    base_offset: usize,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    stats: &mut ParseStats,
) -> bool {
    let emit = |byte_offset: usize,
                message: &str,
                diagnostics: &mut Vec<SyntaxDiagnostic>,
                stats: &mut ParseStats| {
        push_diagnostic(
            options,
            diagnostics,
            stats,
            DiagnosticSeverity::Error,
            DiagnosticKind::InvariantViolation,
            base_offset.saturating_add(byte_offset),
            message,
        );
    };

    let Some(last) = tokens.last() else {
        emit(
            0,
            "token stream invariant violated: empty token stream",
            diagnostics,
            stats,
        );
        return false;
    };

    if !matches!(last.kind, CssTokenKind::Eof) {
        emit(
            input.len_bytes(),
            "token stream invariant violated: missing trailing EOF token",
            diagnostics,
            stats,
        );
        return false;
    }

    let mut previous_end = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.span.input_id != input.id() {
            emit(
                token.span.start,
                "token stream invariant violated: token span belongs to a different input",
                diagnostics,
                stats,
            );
            return false;
        }
        if token.span.end > input.len_bytes() || token.span.start < previous_end {
            emit(
                token.span.start,
                "token stream invariant violated: token spans are not monotonic within the owning input",
                diagnostics,
                stats,
            );
            return false;
        }
        if index + 1 != tokens.len() && matches!(token.kind, CssTokenKind::Eof) {
            emit(
                token.span.start,
                "token stream invariant violated: EOF token must be the last token",
                diagnostics,
                stats,
            );
            return false;
        }
        previous_end = token.span.end;
    }

    true
}

pub(super) fn skip_trivia(tokens: &[CssToken], mut cursor: usize) -> Option<usize> {
    while let Some(token) = tokens.get(cursor) {
        if !is_trivia(&token.kind) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

pub(super) fn skip_trivia_until(
    tokens: &[CssToken],
    mut cursor: usize,
    end_index: Option<usize>,
) -> Option<usize> {
    while let Some(token) = tokens.get(cursor) {
        if Some(cursor) == end_index {
            return Some(cursor);
        }
        if !is_trivia(&token.kind) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

pub(super) fn skip_trivia_and_semicolons_until(
    tokens: &[CssToken],
    mut cursor: usize,
    end_index: Option<usize>,
) -> Option<usize> {
    while let Some(token) = tokens.get(cursor) {
        if Some(cursor) == end_index {
            return Some(cursor);
        }
        if !is_trivia(&token.kind) && !matches!(token.kind, CssTokenKind::Semicolon) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

pub(super) fn is_trivia(kind: &CssTokenKind) -> bool {
    matches!(kind, CssTokenKind::Whitespace | CssTokenKind::Comment(_))
}

pub(super) fn prelude_start_offset(prelude: &[CssComponentValue]) -> Option<usize> {
    prelude.first().map(component_value_start)
}

pub(super) fn component_value_start(value: &CssComponentValue) -> usize {
    match value {
        CssComponentValue::PreservedToken(token) => token.span.start,
        CssComponentValue::SimpleBlock(block) => block.span.start,
        CssComponentValue::Function(function) => function.span.start,
    }
}

pub(super) fn next_component_value_index(tokens: &[CssToken], start: usize) -> usize {
    match tokens.get(start).map(|token| &token.kind) {
        Some(CssTokenKind::LeftCurlyBracket) => {
            find_component_closer(tokens, start, CssBlockKind::Curly)
                .map_or(tokens.len().saturating_sub(1), |index| index + 1)
        }
        Some(CssTokenKind::LeftSquareBracket) => {
            find_component_closer(tokens, start, CssBlockKind::Square)
                .map_or(tokens.len().saturating_sub(1), |index| index + 1)
        }
        Some(CssTokenKind::LeftParenthesis) => {
            find_component_closer(tokens, start, CssBlockKind::Parenthesis)
                .map_or(tokens.len().saturating_sub(1), |index| index + 1)
        }
        Some(CssTokenKind::Function(_)) => find_function_closer(tokens, start)
            .map_or(tokens.len().saturating_sub(1), |index| index + 1),
        Some(_) => start + 1,
        None => start,
    }
}

pub(super) fn is_declaration_start(
    tokens: &[CssToken],
    start: usize,
    end_index: Option<usize>,
) -> bool {
    if !matches!(
        tokens.get(start).map(|token| &token.kind),
        Some(CssTokenKind::Ident(_))
    ) {
        return false;
    }

    let mut cursor = start + 1;
    while let Some(token) = tokens.get(cursor) {
        if Some(cursor) == end_index {
            return false;
        }
        match token.kind {
            CssTokenKind::Whitespace | CssTokenKind::Comment(_) => cursor += 1,
            CssTokenKind::Colon => return true,
            _ => return false,
        }
    }

    false
}

pub(super) fn find_component_closer(
    tokens: &[CssToken],
    start: usize,
    kind: CssBlockKind,
) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start + 1) {
        match &token.kind {
            kind_token if block_kind_matches_opener(kind, kind_token) => depth += 1,
            kind_token if block_kind_matches_closer(kind, kind_token) => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
            }
            CssTokenKind::Eof => return Some(index),
            _ => {}
        }
    }
    None
}

pub(super) fn find_function_closer(tokens: &[CssToken], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start + 1) {
        match token.kind {
            CssTokenKind::Function(_) | CssTokenKind::LeftParenthesis => depth += 1,
            CssTokenKind::RightParenthesis => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
            }
            CssTokenKind::Eof => return Some(index),
            _ => {}
        }
    }
    None
}

pub(super) fn block_kind_for_opener(kind: &CssTokenKind) -> Option<CssBlockKind> {
    match kind {
        CssTokenKind::LeftCurlyBracket => Some(CssBlockKind::Curly),
        CssTokenKind::LeftSquareBracket => Some(CssBlockKind::Square),
        CssTokenKind::LeftParenthesis => Some(CssBlockKind::Parenthesis),
        _ => None,
    }
}

pub(super) fn block_kind_matches_opener(kind: CssBlockKind, token_kind: &CssTokenKind) -> bool {
    matches!(
        (kind, token_kind),
        (CssBlockKind::Curly, CssTokenKind::LeftCurlyBracket)
            | (CssBlockKind::Square, CssTokenKind::LeftSquareBracket)
            | (CssBlockKind::Parenthesis, CssTokenKind::LeftParenthesis)
    )
}

pub(super) fn block_kind_matches_closer(kind: CssBlockKind, token_kind: &CssTokenKind) -> bool {
    matches!(
        (kind, token_kind),
        (CssBlockKind::Curly, CssTokenKind::RightCurlyBracket)
            | (CssBlockKind::Square, CssTokenKind::RightSquareBracket)
            | (CssBlockKind::Parenthesis, CssTokenKind::RightParenthesis)
    )
}
