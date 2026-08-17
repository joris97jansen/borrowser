use super::super::{
    Specificity, SubclassSelector, TreeStructuralPseudoClass, TreeStructuralPseudoClassSelector,
};
use super::support::{parsed_selector_list, span};
use crate::syntax::CssInput;

#[test]
fn tree_structural_pseudo_ir_covers_all_supported_semantic_variants() {
    let input = CssInput::from(":root");
    for pseudo_class in [
        TreeStructuralPseudoClass::Root,
        TreeStructuralPseudoClass::Empty,
        TreeStructuralPseudoClass::FirstChild,
        TreeStructuralPseudoClass::LastChild,
        TreeStructuralPseudoClass::OnlyChild,
    ] {
        let selector = TreeStructuralPseudoClassSelector::new(span(&input, 0, 5), pseudo_class);
        assert_eq!(selector.span(), span(&input, 0, 5));
        assert_eq!(selector.pseudo_class(), pseudo_class);
        assert_eq!(selector.specificity(), Specificity::B);
        assert_eq!(
            SubclassSelector::TreeStructuralPseudoClass(selector).specificity(),
            Specificity::B
        );
    }
}

#[test]
fn parsed_pseudo_spans_include_the_colon_and_remain_monotonic_in_compounds() {
    let list = parsed_selector_list("section:first-child:empty");
    let selector = list.selectors().first().expect("selector");
    let subclasses = selector.head().subclasses();

    assert_eq!(subclasses.len(), 2);
    assert_eq!(subclasses[0].span().start, 7);
    assert_eq!(subclasses[0].span().end, 19);
    assert_eq!(subclasses[1].span().start, 19);
    assert_eq!(subclasses[1].span().end, 25);
}
