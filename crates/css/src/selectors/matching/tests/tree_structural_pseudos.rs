use super::super::{
    SelectorDomElementId, SelectorDomIndex, SelectorMatchDom, SelectorMatchingContext,
};
use super::support::{comment, doc, element, matching_environment, parse_selector_result, text};
use html::{ElementNamespace, internal::Id};

fn matches(index: &SelectorDomIndex<'_>, element: SelectorDomElementId, source: &str) -> bool {
    let context = SelectorMatchingContext::new(index, matching_environment());
    context
        .match_selector_list(element, &parse_selector_result(source))
        .expect("bounded selector matching")
        .matched_any()
}

#[test]
fn root_uses_document_element_identity_while_parentless_subtree_roots_do_not() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><main></main></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("document parses");
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector index");
    let html = index.document_element().expect("document element");
    let body = index.elements().nth(1).expect("body");

    assert!(matches(&index, html, ":root"));
    assert!(matches(&index, html, ":first-child:last-child:only-child"));
    assert!(!matches(&index, body, ":root"));

    let subtree = element("section", Vec::new(), Vec::new());
    let root = subtree.element().expect("element subtree root");
    let subtree_index =
        SelectorDomIndex::try_from_element_subtree(root).expect("subtree selector index");
    let subtree_root = subtree_index.elements().next().expect("subtree root");

    assert!(!matches(&subtree_index, subtree_root, ":root"));
    assert!(matches(
        &subtree_index,
        subtree_root,
        ":first-child:last-child:only-child"
    ));
}

#[test]
fn child_position_pseudos_use_element_sibling_axes_and_work_in_complex_selectors() {
    let pi = html::internal::processing_instruction_from_parts(
        Id(90),
        "target".to_string(),
        "data".to_string(),
    )
    .expect("processing instruction");
    let dom = doc(vec![element(
        "html",
        Vec::new(),
        vec![element(
            "body",
            Vec::new(),
            vec![
                text("before"),
                element("section", vec![("class", Some("first"))], Vec::new()),
                comment("between"),
                pi,
                element("section", vec![("class", Some("middle"))], Vec::new()),
                text("between"),
                element(
                    "section",
                    vec![("class", Some("last"))],
                    vec![element("p", Vec::new(), Vec::new())],
                ),
                comment("after"),
            ],
        )],
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("selector index");
    let sections = index
        .elements()
        .filter(|element| index.element_local_name(*element) == "section")
        .collect::<Vec<_>>();
    let paragraph = index
        .elements()
        .find(|element| index.element_local_name(*element) == "p")
        .expect("paragraph");

    assert!(matches(&index, sections[0], "section.first:first-child"));
    assert!(!matches(&index, sections[0], ":last-child"));
    assert!(!matches(&index, sections[1], ":first-child, :last-child"));
    assert!(matches(&index, sections[2], "body > section:last-child"));
    assert!(matches(
        &index,
        paragraph,
        "section:last-child > p:only-child"
    ));
    assert!(matches(
        &index,
        paragraph,
        ":first-child:last-child:only-child"
    ));
}

#[test]
fn empty_uses_only_ordinary_elements_and_exact_direct_document_whitespace_text() {
    let pi = || {
        html::internal::processing_instruction_from_parts(
            Id(91),
            "target".to_string(),
            "data".to_string(),
        )
        .expect("processing instruction")
    };
    let cases = vec![
        ("none", Vec::new(), true),
        ("zero", vec![text("")], true),
        ("tab", vec![text("\t")], true),
        ("lf", vec![text("\n")], true),
        ("ff", vec![text("\u{000c}")], true),
        ("cr", vec![text("\r")], true),
        ("space", vec![text(" ")], true),
        ("mixed", vec![text(" \t\n\u{000c}\r ")], true),
        ("text", vec![text("content")], false),
        ("nbsp", vec![text("\u{00a0}")], false),
        ("emspace", vec![text("\u{2003}")], false),
        (
            "element",
            vec![element("span", Vec::new(), Vec::new())],
            false,
        ),
        ("comment", vec![comment("ignored")], true),
        ("pi", vec![pi()], true),
        (
            "mixed-neutral",
            vec![comment("ignored"), text(" \n"), pi()],
            true,
        ),
    ];
    let mut expectations = Vec::with_capacity(cases.len());
    let children = cases
        .into_iter()
        .map(|(name, children, expected)| {
            expectations.push((name, expected));
            element("div", vec![("id", Some(name))], children)
        })
        .collect();
    let dom = doc(vec![element("html", Vec::new(), children)]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("selector index");
    let divs = index
        .elements()
        .filter(|element| index.element_local_name(*element) == "div")
        .collect::<Vec<_>>();

    for ((name, expected), element) in expectations.iter().zip(divs) {
        assert_eq!(
            matches(&index, element, ":empty"),
            *expected,
            "unexpected :empty result for {name}",
        );
    }
}

#[test]
fn empty_excludes_template_contents_but_observes_real_ordinary_template_children() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><template><span>content</span>text</template></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("template document parses");
    let parsed_index =
        SelectorDomIndex::try_from_document(&parsed.document).expect("selector index");
    let template = parsed_index
        .elements()
        .find(|element| parsed_index.element_local_name(*element) == "template")
        .expect("template host");
    assert!(matches(&parsed_index, template, "template:empty"));

    let template = html::internal::template_element_from_parts(
        Id(10),
        html::internal::expanded_name(ElementNamespace::Html, "template"),
        Vec::new(),
        Vec::new(),
        Id(11),
        vec![element("span", Vec::new(), Vec::new())],
        vec![text("ordinary")],
    );
    let dom = doc(vec![template]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("selector index");
    let template = index.document_element().expect("template document element");
    assert!(!matches(&index, template, "template:empty"));
}

#[test]
fn parser_created_dom_supports_structural_pseudos_in_compounds_lists_and_combinators() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><section><p></p></section><section><p>x</p></section></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("document parses");
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector index");
    let paragraphs = index
        .elements()
        .filter(|element| index.element_local_name(*element) == "p")
        .collect::<Vec<_>>();

    assert!(matches(
        &index,
        paragraphs[0],
        "article, section:first-child > p:empty"
    ));
    assert!(!matches(
        &index,
        paragraphs[1],
        "article, section:first-child > p:empty"
    ));
}
