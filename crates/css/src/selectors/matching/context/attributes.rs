use crate::selectors::AttributeValue;

pub(super) fn class_list_contains(class_list: &str, want: &str) -> bool {
    split_selector_whitespace_separated_tokens(class_list).any(|token| token == want)
}

pub(super) fn attribute_value_text(value: &AttributeValue) -> &str {
    match value {
        AttributeValue::Ident(value) => value.text(),
        AttributeValue::String(value) => value.value(),
    }
}

pub(super) fn split_selector_whitespace_separated_tokens(
    value: &str,
) -> impl Iterator<Item = &str> {
    value
        .split(is_selector_whitespace)
        .filter(|token| !token.is_empty())
}

pub(super) fn contains_selector_whitespace(value: &str) -> bool {
    value.chars().any(is_selector_whitespace)
}

fn is_selector_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | '\u{0020}'
    )
}
