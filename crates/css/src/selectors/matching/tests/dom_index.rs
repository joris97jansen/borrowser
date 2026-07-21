use super::super::{SelectorDomIndex, SelectorMatchDom};
use super::support::{comment, doc, element, namespaced_element, text};

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
            "version: 2\n",
            "selector-dom\n",
            "elements: 5\n",
            "element[0]: id=1 namespace=html local=\"html\" parent=none prev-sibling=none\n",
            "element[1]: id=2 namespace=html local=\"body\" parent=1 prev-sibling=none\n",
            "element[2]: id=3 namespace=html local=\"div\" parent=2 prev-sibling=none\n",
            "element[3]: id=4 namespace=html local=\"span\" parent=3 prev-sibling=none\n",
            "element[4]: id=5 namespace=html local=\"p\" parent=2 prev-sibling=3\n",
        )
    );
}

#[test]
fn selector_dom_index_skips_processing_instructions_without_breaking_sibling_axes() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><div></div><?Exact-Target data?><span></span></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("PI document parses");
    let index = SelectorDomIndex::from_root(&parsed.document);
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(index.len(), 5, "only html/head/body/div/span are indexed");
    assert_eq!(index.previous_sibling_element(ids[4]), Some(ids[3]));
    assert!(!index.to_debug_snapshot().contains("Exact-Target"));
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
            "version: 2\n",
            "selector-dom\n",
            "elements: 5\n",
            "element[0]: id=1 namespace=html local=\"body\" parent=none prev-sibling=none\n",
            "element[1]: id=2 namespace=html local=\"div\" parent=1 prev-sibling=none\n",
            "element[2]: id=3 namespace=html local=\"span\" parent=1 prev-sibling=2\n",
            "element[3]: id=4 namespace=html local=\"p\" parent=1 prev-sibling=3\n",
            "element[4]: id=5 namespace=html local=\"section\" parent=1 prev-sibling=4\n",
        )
    );
}

#[test]
fn selector_dom_snapshot_exposes_expanded_names_and_exact_foreign_case() {
    let dom = doc(vec![
        element("title", Vec::new(), Vec::new()),
        namespaced_element(
            html::ElementNamespace::Svg,
            "title",
            Vec::new(),
            vec![namespaced_element(
                html::ElementNamespace::Svg,
                "foreignObject",
                Vec::new(),
                Vec::new(),
            )],
        ),
        namespaced_element(
            html::ElementNamespace::MathMl,
            "title",
            Vec::new(),
            Vec::new(),
        ),
    ]);

    let index = SelectorDomIndex::from_root(&dom);
    assert_eq!(
        index.to_debug_snapshot(),
        concat!(
            "version: 2\n",
            "selector-dom\n",
            "elements: 4\n",
            "element[0]: id=1 namespace=html local=\"title\" parent=none prev-sibling=none\n",
            "element[1]: id=2 namespace=svg local=\"title\" parent=none prev-sibling=1\n",
            "element[2]: id=3 namespace=svg local=\"foreignObject\" parent=2 prev-sibling=none\n",
            "element[3]: id=4 namespace=mathml local=\"title\" parent=none prev-sibling=2\n",
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

#[test]
fn synthetic_html_name_construction_canonicalizes_before_selector_indexing() {
    let dom = doc(vec![element("DIV", Vec::new(), Vec::new())]);
    let index = SelectorDomIndex::from_root(&dom);
    let element = index.elements().next().expect("indexed element");
    assert_eq!(index.element_name(element), "div");
}

#[test]
fn unprefixed_attribute_queries_ignore_namespaced_xlink_attributes() {
    let parsed = html::parse_document(
        "<svg><a xlink:href='qualified'></a><a href='ordinary'></a></svg>",
        html::HtmlParseOptions::default(),
    )
    .expect("foreign attribute parse");
    let index = SelectorDomIndex::from_root(&parsed.document);
    let anchors = index
        .elements()
        .filter(|element| index.element_name(*element) == "a")
        .collect::<Vec<_>>();
    assert_eq!(anchors.len(), 2);
    assert!(!index.has_attribute(anchors[0], "href"));
    assert_eq!(index.attribute_value(anchors[0], "href"), None);
    assert!(index.has_attribute(anchors[1], "href"));
    assert_eq!(index.attribute_value(anchors[1], "href"), Some("ordinary"));
}
