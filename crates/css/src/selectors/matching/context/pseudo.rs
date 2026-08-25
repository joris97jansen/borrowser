use super::SelectorMatchingContext;
use super::dom::SelectorMatchDom;
use crate::selectors::{TreeStructuralPseudoClass, TreeStructuralPseudoClassSelector};

impl<D: SelectorMatchDom> SelectorMatchingContext<'_, D> {
    pub fn matches_tree_structural_pseudo_class(
        &self,
        element: D::ElementId,
        selector: &TreeStructuralPseudoClassSelector,
    ) -> bool {
        match selector.pseudo_class() {
            TreeStructuralPseudoClass::Root => self
                .document_element()
                .is_some_and(|document_element| self.same_element(element, document_element)),
            TreeStructuralPseudoClass::Empty => {
                self.first_element_child(element).is_none()
                    && self
                        .direct_text_children(element)
                        .all(text_is_document_whitespace)
            }
            TreeStructuralPseudoClass::FirstChild => {
                self.previous_sibling_element(element).is_none()
            }
            TreeStructuralPseudoClass::LastChild => self.next_sibling_element(element).is_none(),
            TreeStructuralPseudoClass::OnlyChild => {
                self.previous_sibling_element(element).is_none()
                    && self.next_sibling_element(element).is_none()
            }
        }
    }
}

pub(crate) fn text_is_document_whitespace(text: &str) -> bool {
    text.bytes()
        .all(|byte| matches!(byte, b'\t' | b'\n' | 0x0c | b'\r' | b' '))
}

#[cfg(test)]
mod tests {
    use super::text_is_document_whitespace;

    #[test]
    fn document_whitespace_is_exactly_the_css_document_whitespace_set() {
        assert!(text_is_document_whitespace(""));
        assert!(text_is_document_whitespace("\t\n\u{000c}\r "));
        assert!(!text_is_document_whitespace("x"));
        assert!(!text_is_document_whitespace("\u{00a0}"));
        assert!(!text_is_document_whitespace("\u{2003}"));
    }
}
