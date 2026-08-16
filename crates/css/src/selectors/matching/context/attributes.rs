use crate::selectors::AttributeValue;

use super::{SelectorMatchingContext, dom::SelectorMatchDom};

impl<D: SelectorMatchDom> SelectorMatchingContext<'_, D> {
    pub fn has_attribute(&self, element: D::ElementId, name: &str) -> bool {
        self.effective_unqualified_attribute(element, name)
            .is_some()
    }

    pub fn attribute_value(&self, element: D::ElementId, name: &str) -> Option<&str> {
        self.effective_unqualified_attribute(element, name)
            .map(|attribute| attribute.value())
    }

    pub fn element_has_id(&self, element: D::ElementId, want: &str) -> bool {
        self.attribute_value(element, "id")
            .is_some_and(|value| value == want)
    }

    pub fn element_has_class(&self, element: D::ElementId, want: &str) -> bool {
        if want.is_empty() {
            return false;
        }

        self.attribute_value(element, "class")
            .is_some_and(|value| class_list_contains(value, want))
    }
}

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
