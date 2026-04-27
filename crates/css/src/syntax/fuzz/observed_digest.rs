use super::config::CssSyntaxFuzzError;
use super::digest::{mix_str, mix_u64, mix_usize};
use super::invariants::ensure_span_in_input;
use crate::syntax::{
    CssHashKind, CssInput, CssNumericKind, CssSpan, CssToken, CssTokenKind, CssTokenText,
    DiagnosticKind, DiagnosticSeverity, SyntaxDiagnostic,
};

fn mix_diagnostic(mut digest: u64, diagnostic: &SyntaxDiagnostic) -> u64 {
    digest = mix_u64(
        digest,
        match diagnostic.severity {
            DiagnosticSeverity::Warning => 1,
            DiagnosticSeverity::Error => 2,
        },
    );

    digest = mix_u64(
        digest,
        match diagnostic.kind {
            DiagnosticKind::UnexpectedEof => 1,
            DiagnosticKind::UnexpectedToken => 2,
            DiagnosticKind::InvariantViolation => 3,
            DiagnosticKind::EmptySelectorList => 4,
            DiagnosticKind::InvalidSelector => 5,
            DiagnosticKind::UnsupportedSelector => 6,
            DiagnosticKind::InvalidDeclaration => 7,
            DiagnosticKind::UnterminatedComment => 8,
            DiagnosticKind::UnterminatedString => 9,
            DiagnosticKind::BadUrl => 10,
            DiagnosticKind::LimitExceeded => 11,
        },
    );

    mix_usize(digest, diagnostic.byte_offset)
}

pub(super) fn mix_token(
    mut digest: u64,
    input: &CssInput,
    token: &CssToken,
    phase: &'static str,
) -> Result<u64, CssSyntaxFuzzError> {
    ensure_span_in_input(input, token.span, phase)?;

    digest = mix_span(digest, token.span);
    digest = mix_u64(
        digest,
        match &token.kind {
            CssTokenKind::Whitespace => 1,
            CssTokenKind::Comment(_) => 2,
            CssTokenKind::Ident(_) => 3,
            CssTokenKind::Function(_) => 4,
            CssTokenKind::AtKeyword(_) => 5,
            CssTokenKind::Hash { .. } => 6,
            CssTokenKind::String(_) => 7,
            CssTokenKind::BadString => 8,
            CssTokenKind::Url(_) => 9,
            CssTokenKind::BadUrl => 10,
            CssTokenKind::Delim(_) => 11,
            CssTokenKind::Number(_) => 12,
            CssTokenKind::Percentage(_) => 13,
            CssTokenKind::Dimension(_) => 14,
            CssTokenKind::UnicodeRange(_) => 15,
            CssTokenKind::Colon => 16,
            CssTokenKind::Semicolon => 17,
            CssTokenKind::Comma => 18,
            CssTokenKind::LeftSquareBracket => 19,
            CssTokenKind::RightSquareBracket => 20,
            CssTokenKind::LeftParenthesis => 21,
            CssTokenKind::RightParenthesis => 22,
            CssTokenKind::LeftCurlyBracket => 23,
            CssTokenKind::RightCurlyBracket => 24,
            CssTokenKind::IncludeMatch => 25,
            CssTokenKind::DashMatch => 26,
            CssTokenKind::PrefixMatch => 27,
            CssTokenKind::SuffixMatch => 28,
            CssTokenKind::SubstringMatch => 29,
            CssTokenKind::Column => 30,
            CssTokenKind::Cdo => 31,
            CssTokenKind::Cdc => 32,
            CssTokenKind::Eof => 33,
        },
    );

    match &token.kind {
        CssTokenKind::Comment(text)
        | CssTokenKind::Ident(text)
        | CssTokenKind::Function(text)
        | CssTokenKind::AtKeyword(text)
        | CssTokenKind::String(text)
        | CssTokenKind::Url(text) => {
            digest = mix_token_text(digest, input, text, phase)?;
        }
        CssTokenKind::Hash { value, kind } => {
            digest = mix_u64(
                digest,
                match kind {
                    CssHashKind::Id => 1,
                    CssHashKind::Unrestricted => 2,
                },
            );
            digest = mix_token_text(digest, input, value, phase)?;
        }
        CssTokenKind::Delim(value) => {
            digest = mix_u64(digest, u64::from(*value));
        }
        CssTokenKind::Number(number) | CssTokenKind::Percentage(number) => {
            digest = mix_u64(
                digest,
                match number.kind {
                    CssNumericKind::Integer => 1,
                    CssNumericKind::Number => 2,
                },
            );
            digest = mix_token_text(digest, input, &number.repr, phase)?;
        }
        CssTokenKind::Dimension(dimension) => {
            digest = mix_u64(
                digest,
                match dimension.number.kind {
                    CssNumericKind::Integer => 1,
                    CssNumericKind::Number => 2,
                },
            );
            digest = mix_token_text(digest, input, &dimension.number.repr, phase)?;
            digest = mix_token_text(digest, input, &dimension.unit, phase)?;
        }
        CssTokenKind::UnicodeRange(range) => {
            digest = mix_u64(digest, u64::from(range.start()));
            digest = mix_u64(digest, u64::from(range.end()));
        }
        CssTokenKind::Whitespace
        | CssTokenKind::BadString
        | CssTokenKind::BadUrl
        | CssTokenKind::Colon
        | CssTokenKind::Semicolon
        | CssTokenKind::Comma
        | CssTokenKind::LeftSquareBracket
        | CssTokenKind::RightSquareBracket
        | CssTokenKind::LeftParenthesis
        | CssTokenKind::RightParenthesis
        | CssTokenKind::LeftCurlyBracket
        | CssTokenKind::RightCurlyBracket
        | CssTokenKind::IncludeMatch
        | CssTokenKind::DashMatch
        | CssTokenKind::PrefixMatch
        | CssTokenKind::SuffixMatch
        | CssTokenKind::SubstringMatch
        | CssTokenKind::Column
        | CssTokenKind::Cdo
        | CssTokenKind::Cdc
        | CssTokenKind::Eof => {}
    }

    Ok(digest)
}

pub(super) fn mix_token_text(
    digest: u64,
    input: &CssInput,
    text: &CssTokenText,
    phase: &'static str,
) -> Result<u64, CssSyntaxFuzzError> {
    match text.resolve(input) {
        Some(text) => Ok(mix_str(digest, text.as_ref())),
        None => Err(CssSyntaxFuzzError::StructuralInvariantViolation {
            phase,
            detail: "span-backed token text failed to resolve against its owning input".to_string(),
        }),
    }
}

pub(super) fn mix_span(mut digest: u64, span: CssSpan) -> u64 {
    digest = mix_usize(digest, span.start);
    mix_usize(digest, span.end)
}

pub(super) fn observe_diagnostics<T: Copy>(
    input: &CssInput,
    diagnostics: &[SyntaxDiagnostic],
    max_observed: usize,
    mut digest: u64,
    completed: T,
    rejected: T,
    phase: &'static str,
) -> Result<(usize, u64, T), CssSyntaxFuzzError> {
    let mut observed = 0usize;

    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if observed >= max_observed {
            return Ok((observed, digest, rejected));
        }

        if diagnostic.byte_offset > input.len_bytes() {
            return Err(CssSyntaxFuzzError::InvalidDiagnosticOffset {
                phase,
                diagnostic_index: index,
                byte_offset: diagnostic.byte_offset,
                input_bytes: input.len_bytes(),
            });
        }

        digest = mix_diagnostic(digest, diagnostic);
        observed += 1;
    }

    Ok((observed, digest, completed))
}
