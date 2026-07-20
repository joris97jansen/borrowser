use super::super::{
    MatchedSelector, SelectorDomIndex, SelectorMatchability, SelectorMatchingContext,
    SelectorMatchingLimitError, SelectorMatchingLimits,
};
use super::support::namespaced_element;
use super::support::{comment, doc, element, parse_selector_result, parsed_single_selector, text};
use crate::selectors::Specificity;

#[test]
fn matching_context_matches_complex_selectors_with_supported_combinators() {
    let dom = doc(vec![element(
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
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let ids = index.elements().collect::<Vec<_>>();
    let paragraph = ids[4];
    let link = ids[6];

    assert!(
        context
            .matches_complex_selector(paragraph, &parsed_single_selector("main > p.note"))
            .expect("complex selector match")
    );
    assert!(
        context
            .matches_complex_selector(paragraph, &parsed_single_selector("body p.note"))
            .expect("complex selector match")
    );
    assert!(
        context
            .matches_complex_selector(paragraph, &parsed_single_selector("span + p.note"))
            .expect("complex selector match")
    );
    assert!(
        context
            .matches_complex_selector(paragraph, &parsed_single_selector("div ~ p.note"))
            .expect("complex selector match")
    );
    assert!(
        !context
            .matches_complex_selector(paragraph, &parsed_single_selector("div + p.note"))
            .expect("complex selector match")
    );
    assert!(
        !context
            .matches_complex_selector(paragraph, &parsed_single_selector("body > p.note"))
            .expect("complex selector match")
    );
    assert!(
        context
            .matches_complex_selector(
                link,
                &parsed_single_selector("body > main > section > a.link"),
            )
            .expect("complex selector match")
    );
    assert!(
        context
            .matches_complex_selector(link, &parsed_single_selector("main a.link"))
            .expect("complex selector match")
    );
    assert!(
        !context
            .matches_complex_selector(link, &parsed_single_selector("footer a.link"))
            .expect("complex selector match")
    );
}

#[test]
fn ua_namespace_constraint_propagates_through_every_complex_selector_compound() {
    use crate::selectors::SelectorNamespaceConstraint;
    use html::ElementNamespace;

    let dom = doc(vec![element(
        "html",
        Vec::new(),
        vec![element(
            "body",
            Vec::new(),
            vec![
                element("div", vec![("class", Some("notice"))], Vec::new()),
                namespaced_element(
                    ElementNamespace::Svg,
                    "svg",
                    Vec::new(),
                    vec![namespaced_element(
                        ElementNamespace::Svg,
                        "html",
                        Vec::new(),
                        vec![element("div", vec![("class", Some("notice"))], Vec::new())],
                    )],
                ),
                namespaced_element(
                    ElementNamespace::Svg,
                    "div",
                    vec![("class", Some("notice"))],
                    Vec::new(),
                ),
            ],
        )],
    )]);
    let index = SelectorDomIndex::from_root(&dom);
    let elements = index.elements().collect::<Vec<_>>();
    let html_notice = elements[2];
    let html_below_foreign_lookalike = elements[5];
    let foreign_notice = elements[6];
    let author = SelectorMatchingContext::new(&index);
    let ua = author
        .with_namespace_constraint(SelectorNamespaceConstraint::Exact(ElementNamespace::Html));

    for selector in ["html body", "html .notice", "body > *", ".notice"] {
        let parsed = parsed_single_selector(selector);
        let target = if selector == "html body" {
            elements[1]
        } else {
            html_notice
        };
        assert!(
            ua.matches_complex_selector(target, &parsed).unwrap(),
            "{selector}"
        );
    }
    assert!(
        !ua.matches_complex_selector(
            html_below_foreign_lookalike,
            &parsed_single_selector("html > .notice"),
        )
        .unwrap()
    );
    assert!(
        author
            .matches_complex_selector(
                html_below_foreign_lookalike,
                &parsed_single_selector("html > .notice"),
            )
            .unwrap(),
        "author matching can select the foreign lookalike ancestor"
    );
    assert!(
        ua.matches_complex_selector(
            html_below_foreign_lookalike,
            &parsed_single_selector(".notice"),
        )
        .unwrap()
    );
    for selector in ["body > *", ".notice", "*"] {
        assert!(
            !ua.matches_complex_selector(foreign_notice, &parsed_single_selector(selector))
                .unwrap(),
            "{selector} must constrain the foreign candidate"
        );
    }
    assert!(
        author
            .matches_complex_selector(foreign_notice, &parsed_single_selector(".notice"))
            .unwrap(),
        "author typeless selectors retain their current unconstrained namespace semantics"
    );
}

#[test]
fn matching_context_complex_selector_matching_backtracks_across_structural_candidates() {
    let descendant_dom = doc(vec![element(
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
    )]);

    let descendant_index = SelectorDomIndex::from_root(&descendant_dom);
    let descendant_context = SelectorMatchingContext::new(&descendant_index);
    let descendant_target = descendant_index
        .elements()
        .last()
        .expect("descendant target element");

    assert!(
        descendant_context
            .matches_complex_selector(
                descendant_target,
                &parsed_single_selector("body > section.hit span.target"),
            )
            .expect("descendant complex selector match")
    );

    let sibling_dom = doc(vec![element(
        "main",
        Vec::new(),
        vec![
            element("a", Vec::new(), Vec::new()),
            element("span", Vec::new(), Vec::new()),
            element("div", Vec::new(), Vec::new()),
            element("span", Vec::new(), Vec::new()),
            element("p", vec![("class", Some("target"))], Vec::new()),
        ],
    )]);

    let sibling_index = SelectorDomIndex::from_root(&sibling_dom);
    let sibling_context = SelectorMatchingContext::new(&sibling_index);
    let sibling_target = sibling_index
        .elements()
        .last()
        .expect("sibling target element");

    assert!(
        sibling_context
            .matches_complex_selector(
                sibling_target,
                &parsed_single_selector("a + span ~ p.target"),
            )
            .expect("sibling complex selector match")
    );
}

#[test]
fn matching_context_match_selector_list_matches_complex_selectors_in_source_order() {
    let dom = doc(vec![element(
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
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let target = index.elements().last().expect("target element");
    let selectors = parse_selector_result(
        "footer, body > main > p.note, span + p.note, div ~ p.note, div + p.note",
    );
    let outcome = context
        .match_selector_list(target, &selectors)
        .expect("selector list match outcome");

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
fn matching_context_reports_axis_step_limit_deterministically() {
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![element("span", vec![("class", Some("target"))], Vec::new())],
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::with_limits(
        &index,
        SelectorMatchingLimits {
            max_axis_steps_per_match: 0,
        },
    );
    let target = index.elements().last().expect("target element");
    let selector = parsed_single_selector("body span");

    let error = context
        .matches_complex_selector_checked(target, &selector)
        .expect_err("descendant traversal must hit the configured axis budget");

    assert_eq!(
        error,
        SelectorMatchingLimitError::AxisStepLimitExceeded { limit: 0 }
    );
    assert!(context.matches_complex_selector(target, &selector).is_err());
    assert!(!context.matches_complex_selector_conservative(target, &selector));
}
