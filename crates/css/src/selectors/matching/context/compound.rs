use super::super::comparison::matches_attribute_value;
use super::super::host_language::{attribute_selector_value_case, matches_type_selector_name};
use super::SelectorMatchingContext;
use super::attributes::attribute_value_text;
use super::dom::SelectorMatchDom;
use crate::selectors::{
    AttributeMatchSelector, AttributeSelector, ClassSelector, CompoundSelector, IdSelector,
    SubclassSelector, TypeSelector,
};

impl<D: SelectorMatchDom> SelectorMatchingContext<'_, D> {
    /// Matches one compound selector against one element without any combinator
    /// traversal.
    pub fn matches_compound_selector(
        &self,
        element: D::ElementId,
        selector: &CompoundSelector,
    ) -> bool {
        let namespace_matches = match self.namespace_constraint() {
            super::SelectorNamespaceConstraint::Unconstrained => true,
            super::SelectorNamespaceConstraint::Exact(namespace) => {
                self.element_namespace(element) == namespace
            }
        };
        namespace_matches
            && selector
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
            TypeSelector::Named(selector) => {
                let actual = self.element_local_name(element);
                matches_type_selector_name(
                    self.element_namespace(element),
                    actual,
                    selector.name().text(),
                )
            }
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
            SubclassSelector::TreeStructuralPseudoClass(selector) => {
                self.matches_tree_structural_pseudo_class(element, selector)
            }
        }
    }

    pub fn matches_attribute_match_selector(
        &self,
        element: D::ElementId,
        selector: &AttributeMatchSelector,
    ) -> bool {
        let Some(attribute) = self.effective_unqualified_attribute(element, selector.name().text())
        else {
            return false;
        };

        let expected = attribute_value_text(selector.value());
        let sensitivity = attribute_selector_value_case(self.element_namespace(element), attribute);
        matches_attribute_value(selector.matcher(), attribute.value(), expected, sensitivity)
    }
}
