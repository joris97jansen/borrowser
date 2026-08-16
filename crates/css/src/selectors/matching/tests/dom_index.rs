use super::super::{
    BoundedSelectorDomConstructionError, SelectorDomBuildError, SelectorDomElementIter,
    SelectorDomIndex, SelectorDomNodeKind, SelectorMatchDom,
};
use super::support::{comment, doc, element, text};
use html::internal::Id;

#[test]
fn document_projection_records_actual_document_element_and_neutral_facts() {
    let dom = doc(vec![
        text("outside"),
        element(
            "html",
            vec![("id", Some("root"))],
            vec![text("inside"), element("body", Vec::new(), Vec::new())],
        ),
    ]);

    let index = SelectorDomIndex::try_from_document(&dom).expect("valid document projection");
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(ids.len(), 2);
    assert_eq!(index.document_element(), Some(ids[0]));
    assert_eq!(index.parent_element(ids[0]), None);
    assert_eq!(index.first_element_child(ids[0]), Some(ids[1]));
    assert_eq!(
        index.direct_text_children(ids[0]).collect::<Vec<_>>(),
        vec!["inside"]
    );
    assert_eq!(
        index.to_debug_snapshot(),
        concat!(
            "version: 3\n",
            "selector-dom\n",
            "projection: document\n",
            "document-element: 1\n",
            "elements: 2\n",
            "element[0]: id=1 namespace=html local=\"html\" parent=none prev-sibling=none next-sibling=none first-child=2\n",
            "  attribute[0]: namespace=none local=\"id\" value=\"root\"\n",
            "  direct-text[0]: \"inside\"\n",
            "element[1]: id=2 namespace=html local=\"body\" parent=1 prev-sibling=none next-sibling=none first-child=none\n",
        )
    );
}

#[test]
fn document_projection_accepts_no_document_element_without_inference() {
    let dom = doc(vec![text("outside"), comment("comment")]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("elementless document is valid");

    assert!(index.is_empty());
    assert_eq!(index.document_element(), None);
    assert!(index.to_debug_snapshot().contains("projection: document\n"));
    assert!(
        index
            .to_debug_snapshot()
            .contains("document-element: none\n")
    );
}

#[test]
fn element_subtree_is_explicit_and_never_becomes_the_document_element() {
    let root = element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );
    let html::Node::Element { element: root } = &root else {
        panic!("fixture must be an element");
    };
    let index =
        SelectorDomIndex::try_from_element_subtree(root).expect("valid element subtree projection");
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(ids.len(), 2);
    assert_eq!(index.document_element(), None);
    assert_eq!(index.parent_element(ids[0]), None);
    assert_eq!(index.previous_sibling_element(ids[0]), None);
    assert_eq!(index.next_sibling_element(ids[0]), None);
    assert_eq!(index.first_element_child(ids[0]), Some(ids[1]));
    assert!(
        index
            .to_debug_snapshot()
            .contains("projection: element-subtree\ndocument-element: none\nsubtree-root: 1\n")
    );
}

#[test]
fn document_projection_rejects_invalid_root_kind() {
    let invalid = element("html", Vec::new(), Vec::new());
    assert_eq!(
        expect_build_error(SelectorDomIndex::try_from_document(&invalid)),
        SelectorDomBuildError::InvalidDocumentRoot {
            actual: SelectorDomNodeKind::Element,
        }
    );

    let invalid = text("leaf");
    assert_eq!(
        expect_build_error(SelectorDomIndex::try_from_document(&invalid)),
        SelectorDomBuildError::InvalidDocumentRoot {
            actual: SelectorDomNodeKind::Text,
        }
    );
}

#[test]
fn document_projection_rejects_multiple_direct_document_elements() {
    let dom = doc(vec![
        element("html", Vec::new(), Vec::new()),
        comment("boundary"),
        element("svg", Vec::new(), Vec::new()),
    ]);

    assert_eq!(
        expect_build_error(SelectorDomIndex::try_from_document(&dom)),
        SelectorDomBuildError::MultipleDocumentElements {
            first_child_index: 0,
            second_child_index: 2,
        }
    );
}

#[test]
fn nested_document_is_a_typed_build_failure_instead_of_spliced_content() {
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![
            element("div", Vec::new(), Vec::new()),
            doc(vec![element("span", Vec::new(), Vec::new())]),
        ],
    )]);

    assert_eq!(
        expect_build_error(SelectorDomIndex::try_from_document(&dom)),
        SelectorDomBuildError::NestedDocument { depth: 2 }
    );
}

#[test]
fn element_id_representation_exhaustion_is_typed_through_narrow_test_seam() {
    let dom = doc(vec![element(
        "html",
        Vec::new(),
        vec![
            element("body", Vec::new(), Vec::new()),
            element("footer", Vec::new(), Vec::new()),
        ],
    )]);

    assert_eq!(
        expect_build_error(
            SelectorDomIndex::try_from_document_with_max_element_id_for_test(&dom, 2)
        ),
        SelectorDomBuildError::ElementIdRepresentationExhausted { maximum: 2 }
    );
}

#[test]
fn caller_element_budget_is_not_a_selector_dom_build_error() {
    let dom = doc(vec![element("html", Vec::new(), Vec::new())]);

    assert_eq!(
        expect_bounded_error(SelectorDomIndex::try_from_document_with_element_limit(
            &dom, 0,
        )),
        BoundedSelectorDomConstructionError::ElementLimitExceeded {
            limit: 0,
            observed: 1,
        }
    );
    assert!(SelectorDomIndex::try_from_document(&dom).is_ok());
}

#[test]
fn noncanonical_html_name_is_a_typed_build_failure() {
    let mut names = html::AtomTable::new();
    let name = names.intern_exact("DIV").expect("name allocation");
    let expanded = names
        .expanded_name(html::ElementNamespace::Html, name)
        .expect("expanded name");
    let invalid = html::Node::new_element(expanded, Vec::new(), Vec::new(), Vec::new());
    let dom = doc(vec![invalid]);

    assert_eq!(
        expect_build_error(SelectorDomIndex::try_from_document(&dom)),
        SelectorDomBuildError::NonCanonicalHtmlElementLocalName { element_index: 0 }
    );
}

#[test]
fn parser_created_non_ascii_html_name_is_preserved_as_canonical() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><dív></dív></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("non-ASCII HTML tag name parses");
    let index =
        SelectorDomIndex::try_from_document(&parsed.document).expect("valid parser document");

    let element = index
        .elements()
        .find(|element| index.element_local_name(*element) == "dív")
        .expect("non-ASCII HTML element is projected");
    assert_eq!(index.element_local_name(element), "dív");
    assert_eq!(
        index.element_namespace(element),
        html::ElementNamespace::Html
    );
}

#[test]
fn previous_and_next_element_siblings_skip_text_comments_and_processing_instructions() {
    let processing_instruction = html::internal::processing_instruction_from_parts(
        Id(90),
        "Exact-Target".to_string(),
        "data".to_string(),
    )
    .expect("valid processing instruction");
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![
            text("a"),
            element("div", Vec::new(), Vec::new()),
            comment("b"),
            processing_instruction,
            element("span", Vec::new(), Vec::new()),
            text("c"),
            element("p", Vec::new(), Vec::new()),
        ],
    )]);

    let index = SelectorDomIndex::try_from_document(&dom).expect("valid document projection");
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(ids.len(), 4);
    assert_eq!(index.first_element_child(ids[0]), Some(ids[1]));
    assert_eq!(index.previous_sibling_element(ids[1]), None);
    assert_eq!(index.next_sibling_element(ids[1]), Some(ids[2]));
    assert_eq!(index.previous_sibling_element(ids[2]), Some(ids[1]));
    assert_eq!(index.next_sibling_element(ids[2]), Some(ids[3]));
    assert_eq!(index.previous_sibling_element(ids[3]), Some(ids[2]));
    assert_eq!(index.next_sibling_element(ids[3]), None);
    assert!(!index.to_debug_snapshot().contains("Exact-Target"));
}

#[test]
fn parser_created_text_comment_and_processing_instruction_boundaries_preserve_sibling_axes() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><div></div>text<!--comment--><?Exact-Target data?><span></span></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("mixed non-element sibling document parses");
    let index =
        SelectorDomIndex::try_from_document(&parsed.document).expect("valid parser document");
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(index.len(), 5, "only html/head/body/div/span are indexed");
    assert_eq!(index.document_element(), Some(ids[0]));
    assert_eq!(index.previous_sibling_element(ids[4]), Some(ids[3]));
    assert_eq!(index.next_sibling_element(ids[3]), Some(ids[4]));
}

#[test]
fn exact_direct_text_blocks_are_owner_contiguous_before_descendant_text() {
    let dom = doc(vec![element(
        "div",
        Vec::new(),
        vec![
            text("before"),
            element("span", Vec::new(), vec![text("descendant")]),
            text("after"),
            text(""),
            text(" \t\n\r\u{000c}"),
            text("\u{00a0}"),
        ],
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid document projection");
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(
        index.direct_text_children(ids[0]).collect::<Vec<_>>(),
        vec!["before", "after", "", " \t\n\r\u{000c}", "\u{00a0}"]
    );
    assert_eq!(
        index.direct_text_children(ids[1]).collect::<Vec<_>>(),
        vec!["descendant"]
    );
}

#[test]
fn expanded_names_and_ordered_neutral_attributes_preserve_parser_facts() {
    let parsed = html::parse_document(
        "<html><body><svg><foreignObject xlink:href='qualified' data-kind='svg'></foreignObject></svg><math><mi data-kind='math'>x</mi></math></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("foreign-content document parses");
    let index =
        SelectorDomIndex::try_from_document(&parsed.document).expect("valid parser document");
    let document_element = index.document_element().expect("parser document element");
    assert_eq!(index.element_local_name(document_element), "html");
    assert_eq!(
        index.element_namespace(document_element),
        html::ElementNamespace::Html
    );
    let html_body = index
        .elements()
        .find(|element| index.element_local_name(*element) == "body")
        .expect("HTML body");
    let foreign_object = index
        .elements()
        .find(|element| index.element_local_name(*element) == "foreignObject")
        .expect("foreignObject");
    let math_mi = index
        .elements()
        .find(|element| index.element_local_name(*element) == "mi")
        .expect("MathML mi");

    assert_eq!(index.element_local_name(html_body), "body");
    assert_eq!(
        index.element_namespace(html_body),
        html::ElementNamespace::Html
    );
    assert_eq!(index.element_local_name(foreign_object), "foreignObject");
    assert_eq!(
        index.element_namespace(foreign_object),
        html::ElementNamespace::Svg
    );
    assert_eq!(index.element_local_name(math_mi), "mi");
    assert_eq!(
        index.element_namespace(math_mi),
        html::ElementNamespace::MathMl
    );
    let attributes = index.attributes(foreign_object).collect::<Vec<_>>();
    assert_eq!(attributes.len(), 2);
    assert_eq!(attributes[0].namespace(), html::AttributeNamespace::XLink);
    assert_eq!(attributes[0].local_name(), "href");
    assert_eq!(attributes[0].value(), "qualified");
    assert_eq!(attributes[1].namespace(), html::AttributeNamespace::None);
    assert_eq!(attributes[1].local_name(), "data-kind");
    assert_eq!(attributes[1].value(), "svg");
}

#[test]
fn parser_template_contents_are_excluded_from_ordinary_child_facts() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><template>inert<span>descendant</span></template></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("template document parses");
    let index =
        SelectorDomIndex::try_from_document(&parsed.document).expect("valid parser document");
    let template = index
        .elements()
        .find(|element| index.element_local_name(*element) == "template")
        .expect("template host");

    assert!(
        index
            .elements()
            .all(|element| index.element_local_name(element) != "span")
    );
    assert_eq!(index.first_element_child(template), None);
    assert_eq!(index.direct_text_children(template).next(), None);
}

#[test]
fn ordinary_template_host_children_remain_on_the_ordinary_child_axis() {
    let inert_span = element("span", Vec::new(), vec![text("inert")]);
    let ordinary_div = element("div", Vec::new(), vec![text("ordinary descendant")]);
    let template = html::internal::template_element_from_parts(
        Id(1),
        html::internal::html_name("template"),
        Vec::new(),
        Vec::new(),
        Id(2),
        vec![inert_span],
        vec![text("ordinary text"), ordinary_div],
    );
    let dom = doc(vec![template]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid template projection");
    let ids = index.elements().collect::<Vec<_>>();

    assert_eq!(
        ids.len(),
        2,
        "associated fragment descendant stays excluded"
    );
    assert_eq!(index.element_local_name(ids[0]), "template");
    assert_eq!(index.element_local_name(ids[1]), "div");
    assert_eq!(index.first_element_child(ids[0]), Some(ids[1]));
    assert_eq!(
        index.direct_text_children(ids[0]).collect::<Vec<_>>(),
        vec!["ordinary text"]
    );
}

#[test]
#[cfg(target_pointer_width = "64")]
fn indexed_element_record_stays_within_reviewed_64_bit_budget() {
    const REVIEWED_MAXIMUM_SIZE_BYTES: usize = 48;
    let current_measured_size_bytes = SelectorDomIndex::indexed_element_size_for_test();

    assert!(
        current_measured_size_bytes <= REVIEWED_MAXIMUM_SIZE_BYTES,
        "IndexedElement currently measures {current_measured_size_bytes} bytes, exceeding the reviewed 64-bit budget of {REVIEWED_MAXIMUM_SIZE_BYTES} bytes"
    );
}

#[test]
fn selector_element_iterator_uses_checked_zero_based_bounds() {
    let dom = doc(vec![element(
        "html",
        Vec::new(),
        vec![element("body", Vec::new(), Vec::new())],
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid document projection");
    let mut elements = index.elements();

    assert_eq!(elements.len(), 2);
    assert_eq!(elements.next().map(|element| element.get()), Some(1));
    assert_eq!(elements.len(), 1);
    assert_eq!(elements.next().map(|element| element.get()), Some(2));
    assert_eq!(elements.next(), None);
    assert_eq!(elements.len(), 0);
}

#[test]
fn selector_element_iterator_yields_the_maximum_id_once_then_terminates() {
    let exclusive_end =
        usize::try_from(u32::MAX).expect("u32 selector bound fits usize on supported targets");
    let mut elements =
        SelectorDomElementIter::for_validated_bounds_for_test(exclusive_end - 2, exclusive_end)
            .expect("maximum representable element bound is valid");

    assert_eq!(elements.len(), 2);
    assert_eq!(
        elements.next().map(|element| element.get()),
        Some(u32::MAX - 1)
    );
    assert_eq!(elements.len(), 1);
    assert_eq!(elements.next().map(|element| element.get()), Some(u32::MAX));
    assert_eq!(elements.len(), 0);
    assert_eq!(elements.next(), None);
    assert_eq!(
        elements.next(),
        None,
        "terminated iterator remains terminated"
    );
    #[cfg(target_pointer_width = "64")]
    assert!(
        SelectorDomElementIter::for_validated_bounds_for_test(exclusive_end, exclusive_end + 1,)
            .is_none(),
        "bounds beyond the ID representation are rejected instead of saturated"
    );
}

fn expect_build_error(
    result: Result<SelectorDomIndex<'_>, SelectorDomBuildError>,
) -> SelectorDomBuildError {
    match result {
        Ok(_) => panic!("selector DOM construction unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn expect_bounded_error(
    result: Result<SelectorDomIndex<'_>, BoundedSelectorDomConstructionError>,
) -> BoundedSelectorDomConstructionError {
    match result {
        Ok(_) => panic!("bounded selector DOM construction unexpectedly succeeded"),
        Err(error) => error,
    }
}
