use crate::selectors::AttributeValue;
use crate::selectors::matching::comparison::contains_css_whitespace_token;
use crate::selectors::matching::host_language::id_and_class_selector_value_case;

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
        let sensitivity = id_and_class_selector_value_case(self.environment().document_mode());
        self.attribute_value(element, "id")
            .is_some_and(|value| sensitivity.equals(value, want))
    }

    pub fn element_has_class(&self, element: D::ElementId, want: &str) -> bool {
        if want.is_empty() {
            return false;
        }

        let sensitivity = id_and_class_selector_value_case(self.environment().document_mode());
        self.attribute_value(element, "class")
            .is_some_and(|value| contains_css_whitespace_token(value, want, sensitivity))
    }
}

pub(super) fn attribute_value_text(value: &AttributeValue) -> &str {
    match value {
        AttributeValue::Ident(value) => value.text(),
        AttributeValue::String(value) => value.value(),
    }
}
