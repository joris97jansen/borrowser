use css::{
    ParseOptions, Rule, SelectorDomElementId, SelectorDomIndex, SelectorMatchDom,
    SelectorMatchingContext, SelectorMatchingEnvironment, compute_document_styles,
    parse_stylesheet_with_options,
};
use html::{DocumentMode, ElementNamespace, HtmlParseOptions, ParseOutput, parse_document};

fn parse_html(source: &str, expected_mode: DocumentMode) -> ParseOutput {
    let parsed = parse_document(source, HtmlParseOptions::default()).expect("HTML parses");
    assert_eq!(parsed.document_mode, expected_mode);
    parsed
}

fn find_element(
    index: &SelectorDomIndex<'_>,
    namespace: ElementNamespace,
    local_name: &str,
) -> SelectorDomElementId {
    index
        .elements()
        .find(|element| {
            index.element_namespace(*element) == namespace
                && index.element_local_name(*element) == local_name
        })
        .unwrap_or_else(|| panic!("missing {namespace:?} element {local_name:?}"))
}

fn selector_matches(
    index: &SelectorDomIndex<'_>,
    environment: SelectorMatchingEnvironment,
    element: SelectorDomElementId,
    selector_source: &str,
) -> bool {
    let stylesheet_source = format!("{selector_source} {{ color: red; }}");
    let parsed = parse_stylesheet_with_options(&stylesheet_source, &ParseOptions::stylesheet());
    assert!(
        parsed.diagnostics.is_empty(),
        "selector {selector_source:?} produced diagnostics: {:?}",
        parsed.diagnostics
    );
    let [Rule::Style(rule)] = parsed.stylesheet.rules.as_slice() else {
        panic!("selector fixture must produce exactly one style rule");
    };

    SelectorMatchingContext::new(index, environment)
        .match_selector_list(element, &rule.selectors)
        .expect("selector matching remains within default limits")
        .matched_any()
}

#[test]
fn parser_selected_document_mode_controls_id_and_class_values_only() {
    let cases = [
        (
            "<!doctype html><html><body><div id='Hero-é' class='Card-é'></div></body></html>",
            DocumentMode::NoQuirks,
            false,
        ),
        (
            "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\"><html><body><div id='Hero-é' class='Card-é'></div></body></html>",
            DocumentMode::LimitedQuirks,
            false,
        ),
        (
            "<!doctype foo><html><body><div id='Hero-é' class='Card-é'></div></body></html>",
            DocumentMode::Quirks,
            true,
        ),
    ];

    for (source, expected_mode, ascii_folded_selectors_match) in cases {
        let parsed = parse_html(source, expected_mode);
        let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
        let index =
            SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM builds");
        let target = find_element(&index, ElementNamespace::Html, "div");

        assert!(selector_matches(&index, environment, target, "#Hero-é"));
        assert!(selector_matches(&index, environment, target, ".Card-é"));
        assert_eq!(
            selector_matches(&index, environment, target, "#hero-é"),
            ascii_folded_selectors_match,
            "ID selector mismatch in {expected_mode}"
        );
        assert_eq!(
            selector_matches(&index, environment, target, ".card-é"),
            ascii_folded_selectors_match,
            "class selector mismatch in {expected_mode}"
        );

        assert!(
            !selector_matches(&index, environment, target, "#hero-É"),
            "ID matching must not Unicode-fold in {expected_mode}"
        );
        assert!(
            !selector_matches(&index, environment, target, ".card-É"),
            "class matching must not Unicode-fold in {expected_mode}"
        );

        assert!(selector_matches(
            &index,
            environment,
            target,
            "[id=\"Hero-é\"]"
        ));
        assert!(selector_matches(
            &index,
            environment,
            target,
            "[class~=\"Card-é\"]"
        ));
        assert!(
            !selector_matches(&index, environment, target, "[id=\"hero-é\"]"),
            "[id=...] must not inherit quirks ID-selector comparison"
        );
        assert!(
            !selector_matches(&index, environment, target, "[class~=\"card-é\"]"),
            "[class~=...] must not inherit quirks class-selector comparison"
        );
    }
}

#[test]
fn parser_created_html_and_foreign_elements_use_distinct_name_and_value_rules() {
    let parsed = parse_html(
        concat!(
            "<!doctype html><html><body>",
            "<div type='BuTtOn-é' data-kind='VaLuE-é'></div>",
            "<svg><g type='BuTtOn-é' data-kind='VaLuE-é' xlink:href='Qualified'></g>",
            "<foreignObject type='BuTtOn-é' data-kind='VaLuE-é'>",
            "<section type='BuTtOn-é' data-kind='VaLuE-é'></section>",
            "</foreignObject></svg>",
            "<math><mi type='BuTtOn-é' data-kind='VaLuE-é'></mi></math>",
            "</body></html>",
        ),
        DocumentMode::NoQuirks,
    );
    let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM builds");

    let html_div = find_element(&index, ElementNamespace::Html, "div");
    let svg_g = find_element(&index, ElementNamespace::Svg, "g");
    let foreign_object = find_element(&index, ElementNamespace::Svg, "foreignObject");
    let integration_child = find_element(&index, ElementNamespace::Html, "section");
    let math_mi = find_element(&index, ElementNamespace::MathMl, "mi");

    assert!(selector_matches(
        &index,
        environment,
        html_div,
        "DIV[TYPE=\"button-é\"]"
    ));
    assert!(!selector_matches(
        &index,
        environment,
        html_div,
        "div[type=\"button-É\"]"
    ));
    assert!(!selector_matches(
        &index,
        environment,
        html_div,
        "div[data-kind=\"value-é\"]"
    ));
    assert!(selector_matches(
        &index,
        environment,
        html_div,
        "div[data-kind=\"VaLuE-é\"]"
    ));

    for (element, exact_type_selector) in [
        (svg_g, "g"),
        (foreign_object, "foreignObject"),
        (math_mi, "mi"),
    ] {
        assert!(selector_matches(
            &index,
            environment,
            element,
            &format!("{exact_type_selector}[type=\"BuTtOn-é\"]")
        ));
        assert!(!selector_matches(
            &index,
            environment,
            element,
            &format!("{exact_type_selector}[type=\"button-é\"]")
        ));
        assert!(!selector_matches(
            &index,
            environment,
            element,
            &format!("{exact_type_selector}[TYPE=\"BuTtOn-é\"]")
        ));
    }

    assert!(!selector_matches(
        &index,
        environment,
        foreign_object,
        "foreignobject[type=\"BuTtOn-é\"]"
    ));
    assert!(selector_matches(
        &index,
        environment,
        integration_child,
        "SECTION[TYPE=\"button-é\"]"
    ));
    assert!(
        !selector_matches(&index, environment, svg_g, "g[href]"),
        "an unqualified attribute selector must not match xlink:href"
    );
}

#[test]
fn parser_mode_and_host_language_policy_reach_computed_style() {
    let parsed = parse_html(
        concat!(
            "<!doctype foo><html><body>",
            "<p id='Hero'></p>",
            "<p class='Callout'></p>",
            "<p type='BUTTON'></p>",
            "<p id='Literal'></p>",
            "</body></html>",
        ),
        DocumentMode::Quirks,
    );
    let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM builds");
    let paragraphs = index
        .elements()
        .filter(|element| {
            index.element_namespace(*element) == ElementNamespace::Html
                && index.element_local_name(*element) == "p"
        })
        .collect::<Vec<_>>();
    assert_eq!(paragraphs.len(), 4);

    let stylesheet = parse_stylesheet_with_options(
        concat!(
            "#hero { color: red; }",
            ".callout { color: blue; }",
            "[type=button] { color: green; }",
            "[id=literal] { color: white; }",
        ),
        &ParseOptions::stylesheet(),
    );
    assert!(stylesheet.diagnostics.is_empty());

    let computed = compute_document_styles(&parsed.document, environment, &[stylesheet])
        .expect("computed styles resolve");
    assert_eq!(computed.matching_environment(), environment);

    let colors = paragraphs
        .into_iter()
        .map(|element| {
            computed
                .get(element)
                .expect("paragraph has computed style")
                .style()
                .color()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        colors,
        [
            (255, 0, 0, 255),
            (0, 0, 255, 255),
            (0, 128, 0, 255),
            (0, 0, 0, 255),
        ]
    );
}
