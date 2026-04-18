use super::labels::{block_kind_label, numeric_kind_label, quoted_raw, quoted_text};
use crate::syntax::{CssComponentValue, CssInput, CssTokenKind};

pub(super) fn component_value_snapshot(input: &CssInput, value: &CssComponentValue) -> String {
    match value {
        CssComponentValue::PreservedToken(token) => {
            format!(
                "token({}) @{}..{}",
                token_kind_snapshot(input, &token.kind),
                token.span.start,
                token.span.end
            )
        }
        CssComponentValue::SimpleBlock(block) => format!(
            "simple-block(kind={}, text={}) @{}..{}",
            block_kind_label(block.kind),
            quoted_raw(input.slice(block.span).unwrap_or("")),
            block.span.start,
            block.span.end
        ),
        CssComponentValue::Function(function) => format!(
            "function(name={}, text={}) @{}..{}",
            quoted_text(input, &function.name),
            quoted_raw(input.slice(function.span).unwrap_or("")),
            function.span.start,
            function.span.end
        ),
    }
}

fn token_kind_snapshot(input: &CssInput, kind: &CssTokenKind) -> String {
    match kind {
        CssTokenKind::Whitespace => "whitespace".to_string(),
        CssTokenKind::Comment(text) => format!("comment({})", quoted_text(input, text)),
        CssTokenKind::Ident(text) => format!("ident({})", quoted_text(input, text)),
        CssTokenKind::Function(text) => format!("function({})", quoted_text(input, text)),
        CssTokenKind::AtKeyword(text) => format!("at-keyword({})", quoted_text(input, text)),
        CssTokenKind::Hash { value, kind } => format!(
            "hash(kind={}, value={})",
            match kind {
                crate::syntax::CssHashKind::Id => "id",
                crate::syntax::CssHashKind::Unrestricted => "unrestricted",
            },
            quoted_text(input, value)
        ),
        CssTokenKind::String(text) => format!("string({})", quoted_text(input, text)),
        CssTokenKind::BadString => "bad-string".to_string(),
        CssTokenKind::Url(text) => format!("url({})", quoted_text(input, text)),
        CssTokenKind::BadUrl => "bad-url".to_string(),
        CssTokenKind::Delim(ch) => format!("delim({})", quoted_raw(&ch.to_string())),
        CssTokenKind::Number(number) => format!(
            "number(kind={}, value={})",
            numeric_kind_label(number.kind),
            quoted_text(input, &number.repr)
        ),
        CssTokenKind::Percentage(number) => format!(
            "percentage(kind={}, value={})",
            numeric_kind_label(number.kind),
            quoted_text(input, &number.repr)
        ),
        CssTokenKind::Dimension(dimension) => format!(
            "dimension(kind={}, value={}, unit={})",
            numeric_kind_label(dimension.number.kind),
            quoted_text(input, &dimension.number.repr),
            quoted_text(input, &dimension.unit)
        ),
        CssTokenKind::UnicodeRange(range) => {
            format!("unicode-range(U+{:X}-{:X})", range.start(), range.end())
        }
        CssTokenKind::Colon => "colon".to_string(),
        CssTokenKind::Semicolon => "semicolon".to_string(),
        CssTokenKind::Comma => "comma".to_string(),
        CssTokenKind::LeftSquareBracket => "left-square-bracket".to_string(),
        CssTokenKind::RightSquareBracket => "right-square-bracket".to_string(),
        CssTokenKind::LeftParenthesis => "left-parenthesis".to_string(),
        CssTokenKind::RightParenthesis => "right-parenthesis".to_string(),
        CssTokenKind::LeftCurlyBracket => "left-curly-bracket".to_string(),
        CssTokenKind::RightCurlyBracket => "right-curly-bracket".to_string(),
        CssTokenKind::IncludeMatch => "include-match".to_string(),
        CssTokenKind::DashMatch => "dash-match".to_string(),
        CssTokenKind::PrefixMatch => "prefix-match".to_string(),
        CssTokenKind::SuffixMatch => "suffix-match".to_string(),
        CssTokenKind::SubstringMatch => "substring-match".to_string(),
        CssTokenKind::Column => "column".to_string(),
        CssTokenKind::Cdo => "cdo".to_string(),
        CssTokenKind::Cdc => "cdc".to_string(),
        CssTokenKind::Eof => "eof".to_string(),
    }
}
