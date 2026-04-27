use super::SelectorMatchingContext;
use super::attributes::{
    attribute_value_text, contains_selector_whitespace, split_selector_whitespace_separated_tokens,
};
use super::dom::SelectorMatchDom;
use crate::selectors::{
    AttributeMatchSelector, AttributeMatcher, AttributeSelector, ClassSelector, CompoundSelector,
    IdSelector, SubclassSelector, TypeSelector,
};

impl<D: SelectorMatchDom> SelectorMatchingContext<'_, D> {
    /// Matches one compound selector against one element without any combinator
    /// traversal.
    pub fn matches_compound_selector(
        &self,
        element: D::ElementId,
        selector: &CompoundSelector,
    ) -> bool {
        selector
            .type_selector()
            .is_none_or(|selector| self.matches_type_selector(element, selector))
            && selector
                .subclasses()
                .iter()
                .all(|selector| self.matches_subclass_selector(element, selector))
    }

    pub fn matches_type_selector(&self, element: D::ElementId, selector: &TypeSelector) -> bool {
        match selector {
            TypeSelector::Universal(_) => true,
            TypeSelector::Named(selector) => self
                .element_name(element)
                .eq_ignore_ascii_case(selector.name().text()),
        }
    }

    pub fn matches_id_selector(&self, element: D::ElementId, selector: &IdSelector) -> bool {
        self.element_has_id(element, selector.name().text())
    }

    pub fn matches_class_selector(&self, element: D::ElementId, selector: &ClassSelector) -> bool {
        self.element_has_class(element, selector.name().text())
    }

    pub fn matches_attribute_selector(
        &self,
        element: D::ElementId,
        selector: &AttributeSelector,
    ) -> bool {
        match selector {
            AttributeSelector::Exists(selector) => {
                self.has_attribute(element, selector.name().text())
            }
            AttributeSelector::Match(selector) => {
                self.matches_attribute_match_selector(element, selector)
            }
        }
    }

    pub fn matches_subclass_selector(
        &self,
        element: D::ElementId,
        selector: &SubclassSelector,
    ) -> bool {
        match selector {
            SubclassSelector::Id(selector) => self.matches_id_selector(element, selector),
            SubclassSelector::Class(selector) => self.matches_class_selector(element, selector),
            SubclassSelector::Attribute(selector) => {
                self.matches_attribute_selector(element, selector)
            }
        }
    }

    pub fn matches_attribute_match_selector(
        &self,
        element: D::ElementId,
        selector: &AttributeMatchSelector,
    ) -> bool {
        let Some(actual) = self.attribute_value(element, selector.name().text()) else {
            return false;
        };

        let expected = attribute_value_text(selector.value());

        match selector.matcher() {
            AttributeMatcher::Exact => actual == expected,
            AttributeMatcher::Includes => {
                !expected.is_empty()
                    && !contains_selector_whitespace(expected)
                    && split_selector_whitespace_separated_tokens(actual)
                        .any(|token| token == expected)
            }
            AttributeMatcher::DashMatch => {
                actual == expected
                    || actual
                        .strip_prefix(expected)
                        .is_some_and(|rest| rest.starts_with('-'))
            }
            AttributeMatcher::Prefix => !expected.is_empty() && actual.starts_with(expected),
            AttributeMatcher::Suffix => !expected.is_empty() && actual.ends_with(expected),
            AttributeMatcher::Substring => !expected.is_empty() && actual.contains(expected),
        }
    }
}
