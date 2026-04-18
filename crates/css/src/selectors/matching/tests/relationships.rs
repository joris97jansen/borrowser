use super::super::{SelectorDomIndex, SelectorMatchingContext};
use super::support::{comment, doc, element, text};

#[test]
fn matching_context_exposes_nearest_first_traversal_sequences() {
    let dom = doc(vec![element(
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
    )]);

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
    let dom = doc(vec![element(
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
    )]);

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
