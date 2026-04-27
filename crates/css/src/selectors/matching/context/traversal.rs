use super::dom::SelectorMatchDom;

/// Nearest-first ancestor iterator for selector matching.
pub struct AncestorElements<'a, D: SelectorMatchDom> {
    pub(super) dom: &'a D,
    pub(super) next: Option<D::ElementId>,
}

impl<D: SelectorMatchDom> Iterator for AncestorElements<'_, D> {
    type Item = D::ElementId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = self.dom.parent_element(current);
        Some(current)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

/// Nearest-first previous-sibling iterator for selector matching.
pub struct PreviousSiblingElements<'a, D: SelectorMatchDom> {
    pub(super) dom: &'a D,
    pub(super) next: Option<D::ElementId>,
}

impl<D: SelectorMatchDom> Iterator for PreviousSiblingElements<'_, D> {
    type Item = D::ElementId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = self.dom.previous_sibling_element(current);
        Some(current)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
