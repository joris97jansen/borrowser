use super::SelectorMatchingContext;
use super::dom::SelectorMatchDom;
use super::traversal::{AncestorElements, PreviousSiblingElements};

impl<'a, D: SelectorMatchDom> SelectorMatchingContext<'a, D> {
    pub fn same_element(&self, left: D::ElementId, right: D::ElementId) -> bool {
        left == right
    }

    pub fn parent_element(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.parent_element(element)
    }

    pub fn previous_sibling_element(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.previous_sibling_element(element)
    }

    /// Returns nearest-first ancestor elements, excluding `element` itself.
    pub fn ancestor_elements(&self, element: D::ElementId) -> AncestorElements<'a, D> {
        AncestorElements {
            dom: self.dom,
            next: self.parent_element(element),
        }
    }

    /// Returns nearest-first previous element siblings, excluding `element`
    /// itself.
    pub fn previous_sibling_elements(
        &self,
        element: D::ElementId,
    ) -> PreviousSiblingElements<'a, D> {
        PreviousSiblingElements {
            dom: self.dom,
            next: self.previous_sibling_element(element),
        }
    }

    pub fn is_child_of(&self, element: D::ElementId, parent: D::ElementId) -> bool {
        self.parent_element(element) == Some(parent)
    }

    pub fn is_descendant_of(&self, element: D::ElementId, ancestor: D::ElementId) -> bool {
        self.ancestor_elements(element)
            .any(|candidate| self.same_element(candidate, ancestor))
    }

    pub fn is_next_sibling_of(&self, element: D::ElementId, sibling: D::ElementId) -> bool {
        self.previous_sibling_element(element) == Some(sibling)
    }

    pub fn is_subsequent_sibling_of(&self, element: D::ElementId, sibling: D::ElementId) -> bool {
        self.previous_sibling_elements(element)
            .any(|candidate| self.same_element(candidate, sibling))
    }

    pub fn element_name(&self, element: D::ElementId) -> &str {
        self.dom.element_name(element)
    }

    pub fn has_attribute(&self, element: D::ElementId, name: &str) -> bool {
        self.dom.has_attribute(element, name)
    }

    pub fn attribute_value(&self, element: D::ElementId, name: &str) -> Option<&str> {
        self.dom.attribute_value(element, name)
    }

    pub fn element_has_id(&self, element: D::ElementId, want: &str) -> bool {
        self.dom.element_has_id(element, want)
    }

    pub fn element_has_class(&self, element: D::ElementId, want: &str) -> bool {
        self.dom.element_has_class(element, want)
    }
}
