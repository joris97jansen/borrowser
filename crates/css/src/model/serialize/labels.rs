use super::super::{PropertyNameKind, ValueSymbol};
use crate::syntax::{
    CssBlockKind, CssInput, CssNumericKind, CssParseOrigin, CssSpan, CssTokenText,
};

pub(super) fn indexed_label(label: &str, index: Option<usize>) -> String {
    match index {
        Some(index) => format!("{label}[{index}] "),
        None => format!("{label} "),
    }
}

pub(super) fn origin_label(origin: CssParseOrigin) -> &'static str {
    match origin {
        CssParseOrigin::Stylesheet => "stylesheet",
        CssParseOrigin::StyleAttribute => "style-attribute",
    }
}

pub(super) fn property_name_kind_label(kind: PropertyNameKind) -> &'static str {
    match kind {
        PropertyNameKind::Standard => "standard",
        PropertyNameKind::Custom => "custom",
        PropertyNameKind::Invalid => "invalid",
    }
}

pub(super) fn value_symbol_label(kind: ValueSymbol) -> &'static str {
    match kind {
        ValueSymbol::Colon => "colon",
        ValueSymbol::Semicolon => "semicolon",
        ValueSymbol::Comma => "comma",
        ValueSymbol::LeftSquareBracket => "left-square-bracket",
        ValueSymbol::RightSquareBracket => "right-square-bracket",
        ValueSymbol::LeftParenthesis => "left-parenthesis",
        ValueSymbol::RightParenthesis => "right-parenthesis",
        ValueSymbol::LeftCurlyBracket => "left-curly-bracket",
        ValueSymbol::RightCurlyBracket => "right-curly-bracket",
        ValueSymbol::IncludeMatch => "include-match",
        ValueSymbol::DashMatch => "dash-match",
        ValueSymbol::PrefixMatch => "prefix-match",
        ValueSymbol::SuffixMatch => "suffix-match",
        ValueSymbol::SubstringMatch => "substring-match",
        ValueSymbol::Column => "column",
        ValueSymbol::Cdo => "cdo",
        ValueSymbol::Cdc => "cdc",
    }
}

pub(super) fn span_label(span: Option<CssSpan>) -> String {
    match span {
        Some(span) => format!("@{}..{}", span.start, span.end),
        None => "@<none>".to_string(),
    }
}

pub(super) fn numeric_kind_label(kind: CssNumericKind) -> &'static str {
    match kind {
        CssNumericKind::Integer => "integer",
        CssNumericKind::Number => "number",
    }
}

pub(super) fn block_kind_label(kind: CssBlockKind) -> &'static str {
    match kind {
        CssBlockKind::Curly => "curly",
        CssBlockKind::Square => "square",
        CssBlockKind::Parenthesis => "parenthesis",
    }
}

pub(super) fn quoted_text(input: &CssInput, text: &CssTokenText) -> String {
    match text.resolve(input) {
        Some(text) => quoted_raw(&text),
        None => "<invalid-span>".to_string(),
    }
}

pub(super) fn quoted_raw(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
