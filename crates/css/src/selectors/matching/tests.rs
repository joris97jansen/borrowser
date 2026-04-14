use super::{
    MatchedSelector, SelectorDomIndex, SelectorListMatchBuilder, SelectorListMatchOutcome,
    SelectorMatchDom, SelectorMatchability, SelectorMatchingContext,
};
use crate::selectors::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue, ClassSelector, ComplexSelector, CompoundSelector, IdSelector,
    InvalidSelectorReason, SelectorIdent, SelectorList, SelectorListParseResult, SelectorString,
    Specificity, SubclassSelector, TypeSelector, UnsupportedSelectorFeature,
};
use crate::syntax::{CssInput, CssRule, ParseOptions, parse_stylesheet_with_options};
use html::Node;
use std::sync::Arc;

fn element(name: &str, attributes: Vec<(&str, Option<&str>)>, children: Vec<Node>) -> Node {
    Node::Element {
        id: html::internal::Id::INVALID,
        name: Arc::<str>::from(name),
        attributes: attributes
            .into_iter()
            .map(|(name, value)| (Arc::<str>::from(name), value.map(str::to_string)))
            .collect(),
        style: Vec::new(),
        children,
    }
}

fn text(value: &str) -> Node {
    Node::Text {
        id: html::internal::Id::INVALID,
        text: value.to_string(),
    }
}

fn comment(value: &str) -> Node {
    Node::Comment {
        id: html::internal::Id::INVALID,
        text: value.to_string(),
    }
}

fn parsed_div_selector_result() -> SelectorListParseResult {
    let input = CssInput::from("div");
    let span = input.span(0, 3).expect("span");
    let named = TypeSelector::named(
        span,
        SelectorIdent::new("div", Some(span)).expect("selector ident"),
    )
    .expect("named type selector");
    let compound = CompoundSelector::new(span, Some(named), Vec::new()).expect("compound selector");
    let complex = ComplexSelector::new(span, compound, Vec::new()).expect("complex selector");
    let list = SelectorList::new(Some(span), vec![complex]).expect("selector list");
    SelectorListParseResult::Parsed(list)
}

fn parse_selector_result(source: &str) -> SelectorListParseResult {
    let stylesheet = format!("{source} {{ color: red; }}");
    let parse = parse_stylesheet_with_options(&stylesheet, &ParseOptions::stylesheet());
    let rule = parse.stylesheet.rules.first().expect("style rule");
    let CssRule::Qualified(rule) = rule else {
        panic!("expected qualified rule");
    };
    crate::selectors::parse_selector_list(&parse.input, &rule.prelude)
}

fn parsed_single_selector(source: &str) -> ComplexSelector {
    parse_selector_result(source)
        .parsed()
        .expect("parsed selector list")
        .selectors()
        .first()
        .expect("selector entry")
        .clone()
}

fn dummy_span(marker: &str) -> crate::syntax::CssSpan {
    let input = CssInput::from(marker);
    input.span(0, marker.len()).expect("dummy span")
}

fn selector_ident(text: &str) -> SelectorIdent {
    SelectorIdent::new(text, None).expect("selector ident")
}

fn selector_string(value: &str) -> SelectorString {
    SelectorString::new(value, None)
}

fn universal_type_selector() -> TypeSelector {
    TypeSelector::universal(dummy_span("*"))
}

fn named_type_selector(name: &str) -> TypeSelector {
    TypeSelector::named(dummy_span("t"), selector_ident(name)).expect("named type selector")
}

fn id_selector(name: &str) -> IdSelector {
    IdSelector::new(dummy_span("#"), selector_ident(name)).expect("id selector")
}

fn class_selector(name: &str) -> ClassSelector {
    ClassSelector::new(dummy_span("."), selector_ident(name)).expect("class selector")
}

fn attribute_exists_selector(name: &str) -> AttributeSelector {
    AttributeSelector::Exists(
        AttributeExistsSelector::new(dummy_span("[]"), selector_ident(name))
            .expect("attribute exists selector"),
    )
}

fn attribute_match_selector(
    name: &str,
    matcher: AttributeMatcher,
    value: AttributeValue,
) -> AttributeSelector {
    AttributeSelector::Match(
        AttributeMatchSelector::new(dummy_span("[]"), selector_ident(name), matcher, value)
            .expect("attribute match selector"),
    )
}

fn ident_value(value: &str) -> AttributeValue {
    AttributeValue::ident(selector_ident(value))
}

fn string_value(value: &str) -> AttributeValue {
    AttributeValue::string(selector_string(value))
}

#[test]
fn parse_results_expose_matchability_without_collapsing_invalidity() {
    let parsed = parsed_div_selector_result();
    let unsupported = crate::selectors::SelectorListParseResult::Unsupported(
        crate::selectors::UnsupportedSelectorList::from_features(
            None,
            [UnsupportedSelectorFeature::PseudoClass],
        ),
    );
    let invalid = crate::selectors::SelectorListParseResult::Invalid(
        crate::selectors::InvalidSelectorList::new(None, InvalidSelectorReason::EmptySelectorList),
    );

    assert_eq!(parsed.matchability(), SelectorMatchability::Parsed);
    assert_eq!(
        unsupported.matchability(),
        SelectorMatchability::Unsupported
    );
    assert_eq!(invalid.matchability(), SelectorMatchability::Invalid);
}

#[test]
fn match_builder_coalesces_duplicates_and_builds_stable_outcome() {
    let mut builder = SelectorListMatchBuilder::new();
    assert!(builder.record_match(3, Specificity::new(0, 1, 2)));
    assert!(builder.record_match(1, Specificity::new(1, 0, 0)));
    assert!(!builder.record_match(3, Specificity::new(0, 1, 2)));
    let outcome = builder.build();

    assert_eq!(
        outcome.matched_selectors(),
        &[
            MatchedSelector::new(1, Specificity::new(1, 0, 0)),
            MatchedSelector::new(3, Specificity::new(0, 1, 2)),
        ]
    );
    assert_eq!(
        outcome.highest_specificity(),
        Some(Specificity::new(1, 0, 0))
    );
    assert_eq!(
        outcome.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-match\n",
            "matchability: parsed\n",
            "matched: yes\n",
            "highest-specificity: (1,0,0)\n",
            "match[0]: selector=1 specificity=(1,0,0)\n",
            "match[1]: selector=3 specificity=(0,1,2)\n",
        )
    );
}

#[test]
fn match_builder_orders_results_by_selector_index_not_insertion_order() {
    let mut builder = SelectorListMatchBuilder::new();
    assert!(builder.record_match(5, Specificity::new(0, 0, 1)));
    assert!(builder.record_match(2, Specificity::new(1, 0, 0)));
    assert!(builder.record_match(4, Specificity::new(0, 2, 0)));
    let outcome = builder.build();

    assert_eq!(
        outcome.matched_selectors(),
        &[
            MatchedSelector::new(2, Specificity::new(1, 0, 0)),
            MatchedSelector::new(4, Specificity::new(0, 2, 0)),
            MatchedSelector::new(5, Specificity::new(0, 0, 1)),
        ]
    );
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "duplicate selector index must not disagree on specificity")]
fn match_builder_rejects_duplicate_selector_indexes_with_different_specificity() {
    let mut builder = SelectorListMatchBuilder::new();
    assert!(builder.record_match(2, Specificity::new(0, 1, 0)));
    let _ = builder.record_match(2, Specificity::new(1, 0, 0));
}

#[test]
fn non_matchable_outcomes_never_report_matches() {
    let unsupported = SelectorListMatchOutcome::unsupported();
    let invalid = SelectorListMatchOutcome::invalid();

    assert!(!unsupported.is_matchable());
    assert!(!unsupported.matched_any());
    assert_eq!(unsupported.highest_specificity(), None);
    assert!(!invalid.is_matchable());
    assert!(!invalid.matched_any());
    assert_eq!(invalid.highest_specificity(), None);
}

#[test]
fn match_outcome_exposes_builder_for_matcher_construction() {
    let mut builder = SelectorListMatchOutcome::builder();
    assert!(builder.record_match(4, Specificity::new(0, 2, 0)));
    let outcome = builder.build();

    assert_eq!(
        outcome.matched_selectors(),
        &[MatchedSelector::new(4, Specificity::new(0, 2, 0))]
    );
}

#[test]
fn matching_context_exposes_nearest_first_traversal_sequences() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![
                element(
                    "main",
                    Vec::new(),
                    vec![
                        element("div", Vec::new(), Vec::new()),
                        text("gap"),
                        element("span", Vec::new(), Vec::new()),
                        comment("ignored"),
                        element("p", Vec::new(), Vec::new()),
                    ],
                ),
                element("footer", Vec::new(), Vec::new()),
            ],
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(
        context.ancestor_elements(ids[4]).collect::<Vec<_>>(),
        vec![ids[1], ids[0]]
    );
    assert_eq!(
        context
            .previous_sibling_elements(ids[4])
            .collect::<Vec<_>>(),
        vec![ids[3], ids[2]]
    );
    assert!(context.ancestor_elements(ids[0]).next().is_none());
    assert!(context.previous_sibling_elements(ids[0]).next().is_none());
}

#[test]
fn matching_context_relationship_queries_are_centralized_and_testable() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![element(
                "main",
                Vec::new(),
                vec![
                    element("div", Vec::new(), Vec::new()),
                    element("span", Vec::new(), Vec::new()),
                    element("p", Vec::new(), Vec::new()),
                ],
            )],
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let ids = index.elements().collect::<Vec<_>>();
    let body = ids[0];
    let main = ids[1];
    let div = ids[2];
    let span = ids[3];
    let paragraph = ids[4];

    assert!(context.same_element(main, main));
    assert!(context.is_child_of(main, body));
    assert!(context.is_child_of(div, main));
    assert!(context.is_descendant_of(paragraph, body));
    assert!(!context.is_descendant_of(body, paragraph));
    assert!(context.is_next_sibling_of(span, div));
    assert!(!context.is_next_sibling_of(paragraph, div));
    assert!(context.is_subsequent_sibling_of(paragraph, div));
    assert!(!context.is_subsequent_sibling_of(div, paragraph));
}

#[test]
fn matching_context_matches_supported_simple_selector_inputs() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "div",
            vec![
                ("id", Some("hero")),
                ("class", Some("card featured")),
                ("data-kind", Some("promo")),
            ],
            Vec::new(),
        )],
    };

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
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "div",
            vec![
                ("id", Some("hero")),
                ("class", Some("card featured")),
                ("data-kind", Some("promo")),
            ],
            Vec::new(),
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");

    let parsed = parse_selector_result("div#hero.card[data-kind=\"promo\"]");
    let selector = parsed
        .parsed()
        .expect("parsed selector list")
        .selectors()
        .first()
        .expect("selector entry");
    assert!(context.matches_compound_selector(element, selector.head()));

    let parsed = parse_selector_result("span.card");
    let selector = parsed
        .parsed()
        .expect("parsed selector list")
        .selectors()
        .first()
        .expect("selector entry");
    assert!(!context.matches_compound_selector(element, selector.head()));

    let parsed = parse_selector_result("div.missing");
    let selector = parsed
        .parsed()
        .expect("parsed selector list")
        .selectors()
        .first()
        .expect("selector entry");
    assert!(!context.matches_compound_selector(element, selector.head()));
}

#[test]
fn matching_context_attribute_match_queries_cover_supported_matchers_and_edges() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
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
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let element = index.elements().next().expect("indexed element");

    assert!(context.matches_attribute_selector(element, &attribute_exists_selector("data-tags"),));
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
fn matching_context_matches_complex_selectors_with_supported_combinators() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![
                element(
                    "main",
                    Vec::new(),
                    vec![
                        element(
                            "div",
                            vec![("id", Some("lead")), ("class", Some("card"))],
                            Vec::new(),
                        ),
                        text("gap"),
                        element("span", vec![("class", Some("hero"))], Vec::new()),
                        comment("ignored"),
                        element("p", vec![("class", Some("note featured"))], Vec::new()),
                        element(
                            "section",
                            Vec::new(),
                            vec![element("a", vec![("class", Some("link"))], Vec::new())],
                        ),
                    ],
                ),
                element("footer", Vec::new(), Vec::new()),
            ],
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let ids = index.elements().collect::<Vec<_>>();
    let paragraph = ids[4];
    let link = ids[6];

    assert!(context.matches_complex_selector(paragraph, &parsed_single_selector("main > p.note"),));
    assert!(context.matches_complex_selector(paragraph, &parsed_single_selector("body p.note"),));
    assert!(context.matches_complex_selector(paragraph, &parsed_single_selector("span + p.note"),));
    assert!(context.matches_complex_selector(paragraph, &parsed_single_selector("div ~ p.note"),));
    assert!(!context.matches_complex_selector(paragraph, &parsed_single_selector("div + p.note"),));
    assert!(
        !context.matches_complex_selector(paragraph, &parsed_single_selector("body > p.note"),)
    );
    assert!(context.matches_complex_selector(
        link,
        &parsed_single_selector("body > main > section > a.link"),
    ));
    assert!(context.matches_complex_selector(link, &parsed_single_selector("main a.link")));
    assert!(!context.matches_complex_selector(link, &parsed_single_selector("footer a.link"),));
}

#[test]
fn matching_context_complex_selector_matching_backtracks_across_structural_candidates() {
    let descendant_dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![element(
                "section",
                vec![("class", Some("hit"))],
                vec![element(
                    "section",
                    vec![("class", Some("hit"))],
                    vec![element("span", vec![("class", Some("target"))], Vec::new())],
                )],
            )],
        )],
    };

    let descendant_index = SelectorDomIndex::from_root(&descendant_dom);
    let descendant_context = SelectorMatchingContext::new(&descendant_index);
    let descendant_target = descendant_index
        .elements()
        .last()
        .expect("descendant target element");

    assert!(descendant_context.matches_complex_selector(
        descendant_target,
        &parsed_single_selector("body > section.hit span.target"),
    ));

    let sibling_dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "main",
            Vec::new(),
            vec![
                element("a", Vec::new(), Vec::new()),
                element("span", Vec::new(), Vec::new()),
                element("div", Vec::new(), Vec::new()),
                element("span", Vec::new(), Vec::new()),
                element("p", vec![("class", Some("target"))], Vec::new()),
            ],
        )],
    };

    let sibling_index = SelectorDomIndex::from_root(&sibling_dom);
    let sibling_context = SelectorMatchingContext::new(&sibling_index);
    let sibling_target = sibling_index
        .elements()
        .last()
        .expect("sibling target element");

    assert!(sibling_context.matches_complex_selector(
        sibling_target,
        &parsed_single_selector("a + span ~ p.target"),
    ));
}

#[test]
fn matching_context_match_selector_list_matches_complex_selectors_in_source_order() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![element(
                "main",
                Vec::new(),
                vec![
                    element("div", Vec::new(), Vec::new()),
                    element("span", Vec::new(), Vec::new()),
                    element("p", vec![("class", Some("note"))], Vec::new()),
                ],
            )],
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let target = index.elements().last().expect("target element");
    let selectors = parse_selector_result(
        "footer, body > main > p.note, span + p.note, div ~ p.note, div + p.note",
    );
    let outcome = context.match_selector_list(target, &selectors);

    assert_eq!(outcome.matchability(), SelectorMatchability::Parsed);
    assert_eq!(
        outcome.matched_selectors(),
        &[
            MatchedSelector::new(1, Specificity::new(0, 1, 3)),
            MatchedSelector::new(2, Specificity::new(0, 1, 2)),
            MatchedSelector::new(3, Specificity::new(0, 1, 2)),
        ]
    );
    assert_eq!(
        outcome.highest_specificity(),
        Some(Specificity::new(0, 1, 3))
    );
}

#[test]
fn matching_context_match_selector_list_reports_not_matched_for_supported_inputs() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "div",
            vec![
                ("id", Some("hero")),
                ("class", Some("card featured")),
                ("data-kind", Some("promo")),
            ],
            Vec::new(),
        )],
    };

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
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element("div", Vec::new(), Vec::new())],
    };

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

#[test]
fn matching_context_complex_selector_matching_is_independent_of_equivalent_dom_construction_paths()
{
    let flat_dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![element(
                "main",
                Vec::new(),
                vec![
                    element("div", Vec::new(), Vec::new()),
                    element("span", Vec::new(), Vec::new()),
                    element("p", vec![("class", Some("note"))], Vec::new()),
                ],
            )],
        )],
    };
    let nested_dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![element(
                "main",
                Vec::new(),
                vec![
                    element("div", Vec::new(), Vec::new()),
                    Node::Document {
                        id: html::internal::Id::INVALID,
                        doctype: None,
                        children: vec![element("span", Vec::new(), Vec::new())],
                    },
                    element("p", vec![("class", Some("note"))], Vec::new()),
                ],
            )],
        )],
    };

    let flat_index = SelectorDomIndex::from_root(&flat_dom);
    let nested_index = SelectorDomIndex::from_root(&nested_dom);
    let flat_context = SelectorMatchingContext::new(&flat_index);
    let nested_context = SelectorMatchingContext::new(&nested_index);
    let flat_target = flat_index.elements().last().expect("flat target");
    let nested_target = nested_index.elements().last().expect("nested target");
    let selectors = parse_selector_result("main > p.note, span + p.note, div ~ p.note");

    let flat_outcome = flat_context.match_selector_list(flat_target, &selectors);
    let nested_outcome = nested_context.match_selector_list(nested_target, &selectors);

    assert_eq!(flat_outcome, nested_outcome);
    assert_eq!(
        flat_outcome.to_debug_snapshot(),
        nested_outcome.to_debug_snapshot()
    );
}

#[test]
fn matching_context_complex_selector_matching_is_independent_of_raw_parse_formatting() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "main",
            Vec::new(),
            vec![
                element("span", Vec::new(), Vec::new()),
                element("p", vec![("class", Some("note"))], Vec::new()),
            ],
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let target = index.elements().last().expect("target element");
    let compact = parse_selector_result("main>span+p.note");
    let formatted = parse_selector_result("main /**/ > /**/ span /**/ + /**/ p.note");

    let compact_outcome = context.match_selector_list(target, &compact);
    let formatted_outcome = context.match_selector_list(target, &formatted);

    assert_eq!(compact_outcome, formatted_outcome);
}

#[test]
fn selector_dom_index_is_document_ordered_and_element_only() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![
            text("before"),
            element(
                "html",
                Vec::new(),
                vec![element(
                    "body",
                    Vec::new(),
                    vec![
                        text("gap"),
                        element(
                            "div",
                            vec![("id", Some("hero"))],
                            vec![element("span", Vec::new(), Vec::new())],
                        ),
                        comment("ignored"),
                        element("p", Vec::new(), Vec::new()),
                    ],
                )],
            ),
        ],
    };

    let index = SelectorDomIndex::from_root(&dom);

    assert_eq!(index.len(), 5);
    assert_eq!(
        index.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-dom\n",
            "elements: 5\n",
            "element[0]: id=1 name=\"html\" parent=none prev-sibling=none\n",
            "element[1]: id=2 name=\"body\" parent=1 prev-sibling=none\n",
            "element[2]: id=3 name=\"div\" parent=2 prev-sibling=none\n",
            "element[3]: id=4 name=\"span\" parent=3 prev-sibling=none\n",
            "element[4]: id=5 name=\"p\" parent=2 prev-sibling=3\n",
        )
    );
}

#[test]
fn selector_dom_index_previous_sibling_skips_non_elements() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![
                text("a"),
                element("div", Vec::new(), Vec::new()),
                comment("b"),
                element("span", Vec::new(), Vec::new()),
                text("c"),
                element("p", Vec::new(), Vec::new()),
            ],
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(ids.len(), 4);
    assert_eq!(index.previous_sibling_element(ids[0]), None);
    assert_eq!(index.previous_sibling_element(ids[1]), None);
    assert_eq!(index.previous_sibling_element(ids[2]), Some(ids[1]));
    assert_eq!(index.previous_sibling_element(ids[3]), Some(ids[2]));
}

#[test]
fn selector_dom_index_normalizes_nested_document_nodes_by_splicing_children() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "body",
            Vec::new(),
            vec![
                element("div", Vec::new(), Vec::new()),
                Node::Document {
                    id: html::internal::Id::INVALID,
                    doctype: None,
                    children: vec![
                        text("gap"),
                        element("span", Vec::new(), Vec::new()),
                        element("p", Vec::new(), Vec::new()),
                    ],
                },
                element("section", Vec::new(), Vec::new()),
            ],
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);

    assert_eq!(
        index.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-dom\n",
            "elements: 5\n",
            "element[0]: id=1 name=\"body\" parent=none prev-sibling=none\n",
            "element[1]: id=2 name=\"div\" parent=1 prev-sibling=none\n",
            "element[2]: id=3 name=\"span\" parent=1 prev-sibling=2\n",
            "element[3]: id=4 name=\"p\" parent=1 prev-sibling=3\n",
            "element[4]: id=5 name=\"section\" parent=1 prev-sibling=4\n",
        )
    );
}

#[test]
fn selector_dom_index_attribute_lookup_is_case_insensitive_on_names_and_exact_on_values() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element(
            "div",
            vec![
                ("ID", Some("hero")),
                ("id", Some("shadowed")),
                ("CLASS", Some("Foo bar")),
                ("data-kind", Some("promo")),
            ],
            Vec::new(),
        )],
    };

    let index = SelectorDomIndex::from_root(&dom);
    let element = index.elements().next().expect("indexed element");

    assert!(index.has_attribute(element, "id"));
    assert_eq!(index.attribute_value(element, "Id"), Some("hero"));
    assert!(index.element_has_id(element, "hero"));
    assert!(!index.element_has_id(element, "HERO"));
    assert!(index.element_has_class(element, "Foo"));
    assert!(index.element_has_class(element, "bar"));
    assert!(!index.element_has_class(element, "foo"));
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "selector DOM element name must be canonical lowercase")]
fn selector_dom_index_rejects_non_canonical_html_element_names() {
    let dom = Node::Document {
        id: html::internal::Id::INVALID,
        doctype: None,
        children: vec![element("DIV", Vec::new(), Vec::new())],
    };

    let _ = SelectorDomIndex::from_root(&dom);
}
