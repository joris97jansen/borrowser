use super::super::{
    MatchedSelector, SelectorDomIndex, SelectorMatchability, SelectorMatchingContext,
};
use super::support::{
    attribute_exists_selector, attribute_match_selector, class_selector, doc, element, id_selector,
    ident_value, named_type_selector, parse_selector_result, parsed_single_selector, string_value,
    universal_type_selector,
};
use crate::selectors::{
    AttributeMatcher, InvalidSelectorReason, Specificity, SubclassSelector,
    UnsupportedSelectorFeature,
};

#[test]
fn matching_context_highest_specificity_comes_from_actual_matches_only() {
    let dom = doc(vec![element(
        "div",
        vec![("class", Some("card"))],
        Vec::new(),
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");
    let selectors = parse_selector_result("#hero, div.card, div");
    let outcome = context.match_selector_list(element, &selectors);

    assert_eq!(outcome.matchability(), SelectorMatchability::Parsed);
    assert_eq!(
        outcome.matched_selectors(),
        &[
            MatchedSelector::new(1, Specificity::new(0, 1, 1)),
            MatchedSelector::new(2, Specificity::new(0, 0, 1)),
        ]
    );
    assert_eq!(
        outcome.highest_specificity(),
        Some(Specificity::new(0, 1, 1))
    );
}

#[test]
fn matching_context_matches_supported_simple_selector_inputs() {
    let dom = doc(vec![element(
        "div",
        vec![
            ("id", Some("hero")),
            ("class", Some("card featured")),
            ("data-kind", Some("promo")),
        ],
        Vec::new(),
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");

    assert!(context.matches_type_selector(element, &universal_type_selector()));
    assert!(context.matches_type_selector(element, &named_type_selector("DIV")));
    assert!(!context.matches_type_selector(element, &named_type_selector("span")));
    assert!(context.matches_id_selector(element, &id_selector("hero")));
    assert!(!context.matches_id_selector(element, &id_selector("HERO")));
    assert!(context.matches_class_selector(element, &class_selector("card")));
    assert!(!context.matches_class_selector(element, &class_selector("missing")));

    assert!(
        context.matches_subclass_selector(element, &SubclassSelector::Id(id_selector("hero")),)
    );
    assert!(context.matches_subclass_selector(
        element,
        &SubclassSelector::Class(class_selector("featured")),
    ));
    assert!(context.matches_subclass_selector(
        element,
        &SubclassSelector::Attribute(attribute_exists_selector("data-kind")),
    ));
}

#[test]
fn matching_context_matches_compound_selectors_element_locally() {
    let dom = doc(vec![element(
        "div",
        vec![
            ("id", Some("hero")),
            ("class", Some("card featured")),
            ("data-kind", Some("promo")),
        ],
        Vec::new(),
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");

    let selector = parsed_single_selector("div#hero.card[data-kind=\"promo\"]");
    assert!(context.matches_compound_selector(element, selector.head()));

    let selector = parsed_single_selector("span.card");
    assert!(!context.matches_compound_selector(element, selector.head()));

    let selector = parsed_single_selector("div.missing");
    assert!(!context.matches_compound_selector(element, selector.head()));
}

#[test]
fn matching_context_attribute_match_queries_cover_supported_matchers_and_edges() {
    let dom = doc(vec![element(
        "div",
        vec![
            ("data-tags", Some("alpha beta")),
            ("lang", Some("en-US")),
            ("data-prefix", Some("foobar")),
            ("data-suffix", Some("foobar")),
            ("data-sub", Some("xxfooyy")),
            ("data-empty", Some("")),
        ],
        Vec::new(),
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");

    assert!(context.matches_attribute_selector(element, &attribute_exists_selector("data-tags")));
    assert!(context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-empty", AttributeMatcher::Exact, string_value("")),
    ));
    assert!(context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-tags", AttributeMatcher::Includes, ident_value("beta")),
    ));
    assert!(!context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-tags", AttributeMatcher::Includes, string_value("")),
    ));
    assert!(!context.matches_attribute_selector(
        element,
        &attribute_match_selector(
            "data-tags",
            AttributeMatcher::Includes,
            string_value("alpha beta"),
        ),
    ));
    assert!(context.matches_attribute_selector(
        element,
        &attribute_match_selector("lang", AttributeMatcher::DashMatch, ident_value("en")),
    ));
    assert!(context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-prefix", AttributeMatcher::Prefix, ident_value("foo")),
    ));
    assert!(!context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-prefix", AttributeMatcher::Prefix, string_value("")),
    ));
    assert!(context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-suffix", AttributeMatcher::Suffix, ident_value("bar")),
    ));
    assert!(context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-sub", AttributeMatcher::Substring, ident_value("foo")),
    ));
    assert!(!context.matches_attribute_selector(
        element,
        &attribute_match_selector("data-sub", AttributeMatcher::Substring, string_value("")),
    ));
}

#[test]
fn matching_context_match_selector_list_reports_not_matched_for_supported_inputs() {
    let dom = doc(vec![element(
        "div",
        vec![
            ("id", Some("hero")),
            ("class", Some("card featured")),
            ("data-kind", Some("promo")),
        ],
        Vec::new(),
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");
    let selectors = parse_selector_result("span, #missing, .other, body > p.note");
    let outcome = context.match_selector_list(element, &selectors);

    assert_eq!(outcome.matchability(), SelectorMatchability::Parsed);
    assert!(!outcome.matched_any());
    assert!(outcome.matched_selectors().is_empty());
    assert_eq!(outcome.highest_specificity(), None);
}

#[test]
fn matching_context_match_selector_list_preserves_non_matchable_parse_states() {
    let dom = doc(vec![element("div", Vec::new(), Vec::new())]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");
    let unsupported = crate::selectors::SelectorListParseResult::Unsupported(
        crate::selectors::UnsupportedSelectorList::from_features(
            None,
            [UnsupportedSelectorFeature::PseudoClass],
        ),
    );
    let invalid = crate::selectors::SelectorListParseResult::Invalid(
        crate::selectors::InvalidSelectorList::new(None, InvalidSelectorReason::EmptySelectorList),
    );

    let unsupported_outcome = context.match_selector_list(element, &unsupported);
    let invalid_outcome = context.match_selector_list(element, &invalid);

    assert_eq!(
        unsupported_outcome.matchability(),
        SelectorMatchability::Unsupported
    );
    assert!(!unsupported_outcome.matched_any());
    assert_eq!(
        invalid_outcome.matchability(),
        SelectorMatchability::Invalid
    );
    assert!(!invalid_outcome.matched_any());
}
