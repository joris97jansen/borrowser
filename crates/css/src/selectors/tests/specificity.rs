use super::super::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue, ClassSelector, IdSelector, Specificity, SubclassSelector, TypeSelector,
};
use super::support::{ident, parsed_selector_list, sample_selector_list, span, string};
use crate::syntax::CssInput;

#[test]
fn specificity_counts_supported_selector_components() {
    let input = CssInput::from("article.card > h1#hero[data-kind=\"promo\"]");
    let list = sample_selector_list(&input);
    let selector = list.iter().next().expect("selector");

    assert_eq!(selector.head().specificity(), Specificity::new(0, 1, 1));
    assert_eq!(
        selector.tail()[0].selector().specificity(),
        Specificity::new(1, 1, 1)
    );
    assert_eq!(selector.specificity(), Specificity::new(1, 2, 2));
    assert_eq!(selector.specificity().ids(), 1);
    assert_eq!(selector.specificity().classes(), 2);
    assert_eq!(selector.specificity().types(), 2);
}

#[test]
fn specificity_is_exposed_for_supported_simple_selector_kinds() {
    let universal_input = CssInput::from("*");
    let universal = TypeSelector::universal(span(&universal_input, 0, 1));
    assert_eq!(universal.specificity(), Specificity::ZERO);

    let type_input = CssInput::from("article");
    let named = TypeSelector::named(span(&type_input, 0, 7), ident(&type_input, 0, 7, "article"))
        .expect("named type selector");
    assert_eq!(named.specificity(), Specificity::TYPE);

    let id_input = CssInput::from("#hero");
    let id = IdSelector::new(span(&id_input, 0, 5), ident(&id_input, 1, 5, "hero"))
        .expect("id selector");
    assert_eq!(id.specificity(), Specificity::ID);
    assert_eq!(
        SubclassSelector::Id(id.clone()).specificity(),
        Specificity::ID
    );

    let class_input = CssInput::from(".card");
    let class = ClassSelector::new(span(&class_input, 0, 5), ident(&class_input, 1, 5, "card"))
        .expect("class selector");
    assert_eq!(class.specificity(), Specificity::CLASS);
    assert_eq!(
        SubclassSelector::Class(class.clone()).specificity(),
        Specificity::CLASS
    );

    let exists_input = CssInput::from("[data-kind]");
    let exists = AttributeExistsSelector::new(
        span(&exists_input, 0, 11),
        ident(&exists_input, 1, 10, "data-kind"),
    )
    .expect("attribute exists selector");
    assert_eq!(exists.specificity(), Specificity::CLASS);
    assert_eq!(
        AttributeSelector::Exists(exists.clone()).specificity(),
        Specificity::CLASS
    );

    let match_input = CssInput::from("[data-kind=\"promo\"]");
    let matched = AttributeMatchSelector::new(
        span(&match_input, 0, 19),
        ident(&match_input, 1, 10, "data-kind"),
        AttributeMatcher::Exact,
        AttributeValue::string(string(&match_input, 11, 18, "promo")),
    )
    .expect("attribute match selector");
    assert_eq!(matched.specificity(), Specificity::CLASS);
    assert_eq!(
        AttributeSelector::Match(matched).specificity(),
        Specificity::CLASS
    );
}

#[test]
fn specificity_saturates_deterministically() {
    let saturated = Specificity::new(u16::MAX, u16::MAX - 1, u16::MAX) + Specificity::new(1, 2, 1);
    assert_eq!(saturated, Specificity::new(u16::MAX, u16::MAX, u16::MAX));

    let mut accum = Specificity::new(u16::MAX - 1, 0, u16::MAX - 1);
    accum += Specificity::new(5, u16::MAX, 5);
    assert_eq!(accum, Specificity::new(u16::MAX, u16::MAX, u16::MAX));
}

#[test]
fn specificity_comparison_is_lexicographic_a_then_b_then_c() {
    assert!(Specificity::new(1, 0, 0) > Specificity::new(0, u16::MAX, u16::MAX));
    assert!(Specificity::new(0, 1, 0) > Specificity::new(0, 0, u16::MAX));
    assert!(Specificity::new(0, 0, 2) > Specificity::new(0, 0, 1));
    assert_eq!(Specificity::new(4, 7, 9), Specificity::new(4, 7, 9));
}

#[test]
fn specificity_saturation_does_not_wrap_or_reverse_ordering() {
    let saturated = Specificity::new(u16::MAX, u16::MAX, u16::MAX) + Specificity::TYPE;
    assert_eq!(saturated, Specificity::new(u16::MAX, u16::MAX, u16::MAX));
    assert!(saturated > Specificity::new(u16::MAX - 1, u16::MAX, u16::MAX));

    let b_saturated = Specificity::new(0, u16::MAX, u16::MAX) + Specificity::CLASS;
    assert_eq!(b_saturated, Specificity::new(0, u16::MAX, u16::MAX));
    assert!(b_saturated > Specificity::new(0, u16::MAX - 1, u16::MAX));
}

#[test]
fn combinators_are_specificity_neutral() {
    let descendant = parsed_selector_list("div span.card");
    let child = parsed_selector_list("div > span.card");
    let next_sibling = parsed_selector_list("div + span.card");
    let subsequent_sibling = parsed_selector_list("div ~ span.card");

    let expected = Specificity::new(0, 1, 2);
    assert_eq!(
        descendant.iter().next().expect("selector").specificity(),
        expected
    );
    assert_eq!(
        child.iter().next().expect("selector").specificity(),
        expected
    );
    assert_eq!(
        next_sibling.iter().next().expect("selector").specificity(),
        expected
    );
    assert_eq!(
        subsequent_sibling
            .iter()
            .next()
            .expect("selector")
            .specificity(),
        expected
    );
}

#[test]
fn parser_derives_specificity_from_selector_ir() {
    let list = parsed_selector_list("*#hero.card[data-kind] > section.notice");
    let selector = list.iter().next().expect("parsed selector");

    assert_eq!(selector.head().specificity(), Specificity::new(1, 2, 0));
    assert_eq!(
        selector.tail()[0].selector().specificity(),
        Specificity::new(0, 1, 1)
    );
    assert_eq!(selector.specificity(), Specificity::new(1, 3, 1));
}
