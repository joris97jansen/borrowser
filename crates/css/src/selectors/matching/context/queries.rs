use super::SelectorMatchingContext;
use super::dom::{SelectorDomAttribute, SelectorMatchDom};
use super::traversal::{
    AncestorElements, ElementChildren, NextSiblingElements, PreviousSiblingElements,
};

impl<'a, D: SelectorMatchDom> SelectorMatchingContext<'a, D> {
    pub fn same_element(&self, left: D::ElementId, right: D::ElementId) -> bool {
        left == right
    }

    pub fn parent_element(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.parent_element(element)
    }

    pub fn document_element(&self) -> Option<D::ElementId> {
        self.dom.document_element()
    }

    pub fn previous_sibling_element(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.previous_sibling_element(element)
    }

    pub fn next_sibling_element(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.next_sibling_element(element)
    }

    pub fn first_element_child(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.first_element_child(element)
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

    /// Returns nearest-first following element siblings, excluding `element`
    /// itself.
    pub fn next_sibling_elements(&self, element: D::ElementId) -> NextSiblingElements<'a, D> {
        NextSiblingElements {
            dom: self.dom,
            next: self.next_sibling_element(element),
        }
    }

    /// Returns ordinary direct element children in forward sibling order.
    pub fn element_children(&self, element: D::ElementId) -> ElementChildren<'a, D> {
        ElementChildren {
            dom: self.dom,
            next: self.first_element_child(element),
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

    pub fn element_local_name(&self, element: D::ElementId) -> &str {
        self.dom.element_local_name(element)
    }

    pub fn element_namespace(&self, element: D::ElementId) -> html::ElementNamespace {
        self.dom.element_namespace(element)
    }

    pub fn attributes(&self, element: D::ElementId) -> D::AttributeIter<'_> {
        self.dom.attributes(element)
    }

    pub fn direct_text_children(&self, element: D::ElementId) -> D::DirectTextChildIter<'_> {
        self.dom.direct_text_children(element)
    }

    pub(crate) fn effective_unqualified_attribute(
        &self,
        element: D::ElementId,
        requested_local_name: &str,
    ) -> Option<SelectorDomAttribute<'_>> {
        crate::dom_attributes::first_effective_unqualified_attribute(
            self.element_namespace(element),
            self.attributes(element),
            requested_local_name,
        )
    }
}
