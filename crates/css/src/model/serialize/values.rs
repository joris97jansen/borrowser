use super::super::{ValueBlock, ValueComponent, ValueFunction, ValueText, ValueToken};
use super::labels::{block_kind_label, numeric_kind_label, quoted_raw, value_symbol_label};
use crate::syntax::CssInput;

pub(super) fn value_component_snapshot(input: &CssInput, value: &ValueComponent) -> String {
    match value {
        ValueComponent::Token(token) => value_token_snapshot(input, token),
        ValueComponent::SimpleBlock(block) => value_block_snapshot(input, block),
        ValueComponent::Function(function) => value_function_snapshot(input, function),
    }
}

fn value_token_snapshot(_input: &CssInput, token: &ValueToken) -> String {
    match token {
        ValueToken::Whitespace { span } => format!("whitespace @{}..{}", span.start, span.end),
        ValueToken::Comment { span, text } => format!(
            "comment({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Ident { span, text } => format!(
            "ident({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::AtKeyword { span, text } => format!(
            "at-keyword({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Hash { span, kind, text } => format!(
            "hash(kind={}, text={}) @{}..{}",
            match kind {
                crate::syntax::CssHashKind::Id => "id",
                crate::syntax::CssHashKind::Unrestricted => "unrestricted",
            },
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::String { span, text } => format!(
            "string({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::BadString { span } => format!("bad-string @{}..{}", span.start, span.end),
        ValueToken::Url { span, text } => format!(
            "url({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::BadUrl { span } => format!("bad-url @{}..{}", span.start, span.end),
        ValueToken::Delim { span, value } => format!(
            "delim({}) @{}..{}",
            quoted_raw(&value.to_string()),
            span.start,
            span.end
        ),
        ValueToken::Number { span, kind, text } => format!(
            "number(kind={}, text={}) @{}..{}",
            numeric_kind_label(*kind),
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Percentage { span, kind, text } => format!(
            "percentage(kind={}, text={}) @{}..{}",
            numeric_kind_label(*kind),
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Dimension {
            span,
            kind,
            number,
            unit,
        } => format!(
            "dimension(kind={}, number={}, unit={}) @{}..{}",
            numeric_kind_label(*kind),
            value_text_snapshot(number),
            value_text_snapshot(unit),
            span.start,
            span.end
        ),
        ValueToken::UnicodeRange { span, range } => format!(
            "unicode-range(U+{:X}-{:X}) @{}..{}",
            range.start(),
            range.end(),
            span.start,
            span.end
        ),
        ValueToken::Symbol { span, kind } => format!(
            "symbol({}) @{}..{}",
            value_symbol_label(*kind),
            span.start,
            span.end
        ),
    }
}

fn value_block_snapshot(input: &CssInput, block: &ValueBlock) -> String {
    let nested = block
        .components
        .iter()
        .map(|value| value_component_snapshot(input, value))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "simple-block(kind={}, components=[{}]) @{}..{}",
        block_kind_label(block.kind),
        nested,
        block.span.start,
        block.span.end
    )
}

fn value_function_snapshot(input: &CssInput, function: &ValueFunction) -> String {
    let nested = function
        .components
        .iter()
        .map(|value| value_component_snapshot(input, value))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "function(name={}, components=[{}]) @{}..{}",
        value_text_snapshot(&function.name),
        nested,
        function.span.start,
        function.span.end
    )
}

fn value_text_snapshot(text: &ValueText) -> String {
    match &text.text {
        Some(text) => quoted_raw(text),
        None => "<invalid-text>".to_string(),
    }
}
