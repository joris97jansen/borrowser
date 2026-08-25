use crate::selectors::matching::comparison::contains_css_whitespace_token;
use crate::selectors::matching::comparison::matches_attribute_value;
use crate::selectors::matching::host_language::{
    attribute_selector_value_case, id_and_class_selector_value_case,
};
use crate::selectors::{AttributeMatcher, AttributeValue, SelectorDomAttribute};
use html::{DocumentMode, ElementNamespace};
use std::cmp::Ordering;

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
        matches_id_in_attributes(
            self.element_namespace(element),
            self.attributes(element),
            self.environment().document_mode(),
            want,
        )
    }

    pub fn element_has_class(&self, element: D::ElementId, want: &str) -> bool {
        matches_class_in_attributes(
            self.element_namespace(element),
            self.attributes(element),
            self.environment().document_mode(),
            want,
        )
    }
}

pub(crate) fn matches_id_in_attributes<'a>(
    element_namespace: ElementNamespace,
    attributes: impl IntoIterator<Item = SelectorDomAttribute<'a>>,
    document_mode: DocumentMode,
    want: &str,
) -> bool {
    let sensitivity = id_and_class_selector_value_case(document_mode);
    crate::dom_attributes::first_effective_unqualified_attribute(
        element_namespace,
        attributes,
        "id",
    )
    .is_some_and(|attribute| sensitivity.equals(attribute.value(), want))
}

pub(crate) fn matches_class_in_attributes<'a>(
    element_namespace: ElementNamespace,
    attributes: impl IntoIterator<Item = SelectorDomAttribute<'a>>,
    document_mode: DocumentMode,
    want: &str,
) -> bool {
    if want.is_empty() {
        return false;
    }
    let sensitivity = id_and_class_selector_value_case(document_mode);
    crate::dom_attributes::first_effective_unqualified_attribute(
        element_namespace,
        attributes,
        "class",
    )
    .is_some_and(|attribute| contains_css_whitespace_token(attribute.value(), want, sensitivity))
}

pub(crate) fn compare_id_and_class_selector_values(
    document_mode: DocumentMode,
    left: &str,
    right: &str,
) -> Ordering {
    id_and_class_selector_value_case(document_mode).compare(left, right)
}

pub(crate) fn id_and_class_selector_values_equal(
    document_mode: DocumentMode,
    left: Option<&str>,
    right: Option<&str>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            compare_id_and_class_selector_values(document_mode, left, right).is_eq()
        }
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

pub(crate) fn matches_attribute_in_attributes<'a>(
    element_namespace: ElementNamespace,
    attributes: impl IntoIterator<Item = SelectorDomAttribute<'a>>,
    name: &str,
    predicate: Option<(AttributeMatcher, &str)>,
) -> bool {
    let Some(attribute) = crate::dom_attributes::first_effective_unqualified_attribute(
        element_namespace,
        attributes,
        name,
    ) else {
        return false;
    };
    let Some((matcher, expected)) = predicate else {
        return true;
    };
    matches_attribute_value(
        matcher,
        attribute.value(),
        expected,
        attribute_selector_value_case(element_namespace, attribute),
    )
}

pub(super) fn attribute_value_text(value: &AttributeValue) -> &str {
    match value {
        AttributeValue::Ident(value) => value.text(),
        AttributeValue::String(value) => value.value(),
    }
}
