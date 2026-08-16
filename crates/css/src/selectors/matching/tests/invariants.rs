use super::super::{SelectorDomBuildError, SelectorDomIndex, SelectorMatchingContext};
use super::support::{doc, element, parse_selector_result};

#[test]
fn matching_context_rejects_nested_documents_instead_of_normalizing_them() {
    let nested_dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![element(
            "main",
            Vec::new(),
            vec![
                element("div", Vec::new(), Vec::new()),
                doc(vec![element("span", Vec::new(), Vec::new())]),
                element("p", vec![("class", Some("note"))], Vec::new()),
            ],
        )],
    )]);

    let error = SelectorDomIndex::try_from_document(&nested_dom)
        .expect_err("nested document must be a selector projection invariant failure");

    assert!(matches!(
        error,
        SelectorDomBuildError::NestedDocument { .. }
    ));
}

#[test]
fn matching_context_complex_selector_matching_is_independent_of_raw_parse_formatting() {
    let dom = doc(vec![element(
        "main",
        Vec::new(),
        vec![
            element("span", Vec::new(), Vec::new()),
            element("p", vec![("class", Some("note"))], Vec::new()),
        ],
    )]);

    let index = SelectorDomIndex::try_from_document(&dom).expect("valid selector test document");
    let context = SelectorMatchingContext::new(&index, super::support::matching_environment());
    let target = index.elements().last().expect("target element");
    let compact = parse_selector_result("main>span+p.note");
    let formatted = parse_selector_result("main /**/ > /**/ span /**/ + /**/ p.note");

    let compact_outcome = context
        .match_selector_list(target, &compact)
        .expect("compact selector match outcome");
    let formatted_outcome = context
        .match_selector_list(target, &formatted)
        .expect("formatted selector match outcome");

    assert_eq!(compact_outcome, formatted_outcome);
}
