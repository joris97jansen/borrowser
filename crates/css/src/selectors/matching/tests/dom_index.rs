use super::super::{SelectorDomIndex, SelectorMatchDom};
use super::support::{comment, doc, element, text};

#[test]
fn selector_dom_index_is_document_ordered_and_element_only() {
    let dom = doc(vec![
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
    ]);

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
    let dom = doc(vec![element(
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
    )]);

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
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![
            element("div", Vec::new(), Vec::new()),
            doc(vec![
                text("gap"),
                element("span", Vec::new(), Vec::new()),
                element("p", Vec::new(), Vec::new()),
            ]),
            element("section", Vec::new(), Vec::new()),
        ],
    )]);

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
    let dom = doc(vec![element(
        "div",
        vec![
            ("ID", Some("hero")),
            ("id", Some("shadowed")),
            ("CLASS", Some("Foo bar")),
            ("data-kind", Some("promo")),
        ],
        Vec::new(),
    )]);

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
    let dom = doc(vec![element("DIV", Vec::new(), Vec::new())]);

    let _ = SelectorDomIndex::from_root(&dom);
}
