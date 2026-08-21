use css::{
    DocumentSelectorMatchingDiagnostic, DocumentSelectorMatchingDiagnosticFailure,
    DocumentSelectorMatchingDiagnosticLimits, InvalidSelectorReason, ParseOptions, Rule,
    SelectorDomElementId, SelectorDomIndex, SelectorListMatchOutcome, SelectorMatchDom,
    SelectorMatchability, SelectorMatchingContext, SelectorMatchingEnvironment,
    SelectorMatchingLimitError, SelectorMatchingLimits, StylesheetCollectionInput,
    StylesheetConditionInput, StylesheetOrder, StylesheetSourceId,
    document_selector_matching_diagnostic, parse_stylesheet_with_options,
};
use html::{
    AttributeNamespace, DocumentMode, ElementNamespace, HtmlParseOptions, Node, ParseOutput,
    parse_document,
};

fn parse_html(source: &str, expected_mode: DocumentMode) -> ParseOutput {
    let output = parse_document(source, HtmlParseOptions::default()).expect("HTML parses");
    assert_eq!(output.document_mode, expected_mode);
    output
}

fn element_by_id(index: &SelectorDomIndex<'_>, id: &str) -> SelectorDomElementId {
    index
        .elements()
        .find(|element| {
            index.attributes(*element).any(|attribute| {
                attribute.namespace() == AttributeNamespace::None
                    && attribute.local_name() == "id"
                    && attribute.value() == id
            })
        })
        .unwrap_or_else(|| panic!("missing element id {id}"))
}

fn parser_node_by_id<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
    if node.element().is_some_and(|element| {
        element.attributes().iter().any(|attribute| {
            attribute.namespace() == AttributeNamespace::None
                && attribute.local_name() == "id"
                && attribute.value() == id
        })
    }) {
        return Some(node);
    }
    node.children()?
        .iter()
        .find_map(|child| parser_node_by_id(child, id))
}

fn selector_outcome(
    index: &SelectorDomIndex<'_>,
    environment: SelectorMatchingEnvironment,
    element: SelectorDomElementId,
    selector: &str,
) -> SelectorListMatchOutcome {
    let stylesheet = parse_stylesheet_with_options(
        &format!("{selector} {{ color: red; }}"),
        &ParseOptions::stylesheet(),
    );
    let [Rule::Style(rule)] = stylesheet.stylesheet.rules.as_slice() else {
        panic!("one style rule expected for {selector}");
    };
    SelectorMatchingContext::new(index, environment)
        .match_selector_list(element, &rule.selectors)
        .expect("default matcher budget")
}

fn assert_matches(
    index: &SelectorDomIndex<'_>,
    environment: SelectorMatchingEnvironment,
    element: SelectorDomElementId,
    selectors: &[&str],
) {
    for selector in selectors {
        assert!(
            selector_outcome(index, environment, element, selector).matched_any(),
            "expected {selector} to match"
        );
    }
}

#[test]
fn parser_dom_proves_simple_compound_attribute_list_and_combinator_matrix() {
    let parsed = parse_html(
        concat!(
            "<!doctype html><html><body>",
            "<main id=scope><div id=target class='alpha beta' data-exact='Value' ",
            "data-list='one two' lang='en-US' data-prefix='prefix-tail' ",
            "data-suffix='head-suffix' data-sub='a-middle-z'></div></main>",
            "<section id=siblings><b id=first></b>text<!--gap--><?pi data?>",
            "<i id=second></i><em id=third></em></section>",
            "</body></html>"
        ),
        DocumentMode::NoQuirks,
    );
    let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
    let sibling_children = parser_node_by_id(&parsed.document, "siblings")
        .and_then(Node::children)
        .expect("parser-created sibling fixture element");
    let first_index = sibling_children
        .iter()
        .position(|node| parser_node_by_id(node, "first").is_some())
        .expect("first sibling element");
    let second_index = sibling_children
        .iter()
        .position(|node| parser_node_by_id(node, "second").is_some())
        .expect("second sibling element");
    let intervening = &sibling_children[first_index + 1..second_index];
    assert!(
        intervening
            .iter()
            .any(|node| matches!(node, Node::Text { .. }))
    );
    assert!(
        intervening
            .iter()
            .any(|node| matches!(node, Node::Comment { .. }))
    );
    assert!(
        intervening
            .iter()
            .any(|node| matches!(node, Node::ProcessingInstruction { .. }))
    );
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM");
    let target = element_by_id(&index, "target");
    assert_matches(
        &index,
        environment,
        target,
        &[
            "*",
            "DIV",
            "#target",
            ".alpha",
            "[data-exact]",
            "[data-exact=Value]",
            "[data-list~=two]",
            "[lang|=en]",
            "[data-prefix^=prefix]",
            "[data-suffix$=suffix]",
            "[data-sub*=middle]",
            "div#target.alpha[data-exact=Value]",
            "aside, #target, footer",
            "body #target",
            "main > #target",
        ],
    );
    assert!(!selector_outcome(&index, environment, target, "span").matched_any());

    let second = element_by_id(&index, "second");
    let third = element_by_id(&index, "third");
    assert_matches(&index, environment, second, &["#first + #second"]);
    assert_matches(
        &index,
        environment,
        third,
        &["#first ~ #third", "#second + #third"],
    );
}

#[test]
fn parser_dom_proves_html_value_policy_namespace_boundaries_and_adjusted_svg_names() {
    let parsed = parse_html(
        concat!(
            "<!doctype html><html><body>",
            "<div id=html type=BuTtOn rel='Foo BAR' lang=EN-us media=ScReEnOnly ",
            "target=MyFrame align=LeFtEdge data-case=VaLuE></div>",
            "<svg><g id=svg type=BuTtOn></g><foreignObject id=fo>",
            "<section id=integration type=BuTtOn></section>",
            "</foreignObject></svg>",
            "<math><mi id=math type=BuTtOn></mi></math>",
            "</body></html>"
        ),
        DocumentMode::NoQuirks,
    );
    let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM");
    let html = element_by_id(&index, "html");
    assert_matches(
        &index,
        environment,
        html,
        &[
            "DIV[TYPE=button]",
            "[rel~=bar]",
            "[lang|=en]",
            "[media^=screen]",
            "[target$=frame]",
            "[align*=fte]",
        ],
    );
    assert!(!selector_outcome(&index, environment, html, "[data-case=value]").matched_any());

    let svg = element_by_id(&index, "svg");
    let foreign_object = element_by_id(&index, "fo");
    let integration = element_by_id(&index, "integration");
    let math = element_by_id(&index, "math");
    assert_eq!(index.element_namespace(svg), ElementNamespace::Svg);
    assert_eq!(
        index.element_namespace(foreign_object),
        ElementNamespace::Svg
    );
    assert_eq!(index.element_local_name(foreign_object), "foreignObject");
    assert_eq!(index.element_namespace(integration), ElementNamespace::Html);
    assert_eq!(index.element_namespace(math), ElementNamespace::MathMl);
    assert!(selector_outcome(&index, environment, foreign_object, "foreignObject").matched_any());
    assert!(!selector_outcome(&index, environment, foreign_object, "foreignobject").matched_any());
    assert!(!selector_outcome(&index, environment, svg, "g[type=button]").matched_any());
    assert!(
        selector_outcome(&index, environment, integration, "SECTION[type=button]").matched_any()
    );
    assert!(!selector_outcome(&index, environment, math, "MI[type=BuTtOn]").matched_any());
}

#[test]
fn parser_selected_modes_drive_id_and_class_matching_environment() {
    let cases = [
        (
            "<!doctype html><html><body><p id=Hero class=Card></p></body></html>",
            DocumentMode::NoQuirks,
            false,
        ),
        (
            "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\"><html><body><p id=Hero class=Card></p></body></html>",
            DocumentMode::LimitedQuirks,
            false,
        ),
        (
            "<!doctype foo><html><body><p id=Hero class=Card></p></body></html>",
            DocumentMode::Quirks,
            true,
        ),
    ];
    for (source, expected_mode, folded_match) in cases {
        let parsed = parse_html(source, expected_mode);
        let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
        let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM");
        let target = element_by_id(&index, "Hero");
        assert_eq!(
            selector_outcome(&index, environment, target, "#hero.card").matched_any(),
            folded_match
        );
    }
}

#[test]
fn parser_dom_proves_static_pseudos_and_template_boundary() {
    let parsed = parse_html(
        concat!(
            "<!doctype html><html id=root><body>",
            "<section id=only-wrap><p id=only></p></section>",
            "<section id=many><p id=first></p><p id=middle><!--comment--><?pi data?></p>",
            "<p id=last>x</p></section>",
            "<template id=host><span id=in-template></span></template>",
            "</body></html>"
        ),
        DocumentMode::NoQuirks,
    );
    let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM");
    assert_matches(
        &index,
        environment,
        element_by_id(&index, "root"),
        &[":root"],
    );
    assert_matches(
        &index,
        environment,
        element_by_id(&index, "only"),
        &[":empty", ":first-child", ":last-child", ":only-child"],
    );
    assert_matches(
        &index,
        environment,
        element_by_id(&index, "first"),
        &[":first-child", ":empty"],
    );
    assert_matches(
        &index,
        environment,
        element_by_id(&index, "middle"),
        &[":empty"],
    );
    assert!(
        !selector_outcome(&index, environment, element_by_id(&index, "last"), ":empty")
            .matched_any()
    );
    assert!(index.elements().all(|element| {
        !index
            .attributes(element)
            .any(|attribute| attribute.value() == "in-template")
    }));
    let host = element_by_id(&index, "host");
    assert_matches(&index, environment, host, &[":empty"]);
    assert!(!selector_outcome(&index, environment, host, "template span").matched_any());
}

#[test]
fn unsupported_malformed_parser_limit_and_matcher_limit_remain_distinct() {
    let parsed = parse_html(
        "<!doctype html><html><body><div><span id=target></span></div></body></html>",
        DocumentMode::NoQuirks,
    );
    let environment = SelectorMatchingEnvironment::new(parsed.document_mode);
    let index = SelectorDomIndex::try_from_document(&parsed.document).expect("selector DOM");
    let target = element_by_id(&index, "target");

    for selector in [
        ":hover", ":active", ":focus", ":visited", ":target", "::before",
    ] {
        let outcome = selector_outcome(&index, environment, target, selector);
        assert_eq!(outcome.matchability(), SelectorMatchability::Unsupported);
        assert!(!outcome.matched_any());
    }
    for selector in ["> span", "span >", "span..bad"] {
        let outcome = selector_outcome(&index, environment, target, selector);
        assert_eq!(outcome.matchability(), SelectorMatchability::Invalid);
        assert!(!outcome.matched_any());
    }

    let mut options = ParseOptions::stylesheet();
    options.limits.max_selectors_per_rule = 0;
    let limited = parse_stylesheet_with_options("span {}", &options);
    let [Rule::Style(rule)] = limited.stylesheet.rules.as_slice() else {
        panic!("limited selector remains a style rule");
    };
    let Some(invalid) = rule.selectors.invalid() else {
        panic!("parser limit must be invalid, not unsupported or absent");
    };
    assert_eq!(
        invalid.reason(),
        InvalidSelectorReason::ResourceLimitExceeded
    );
    let outcome = SelectorMatchingContext::new(&index, environment)
        .match_selector_list(target, &rule.selectors)
        .expect("parser invalidity is not a matcher error");
    assert_eq!(outcome.matchability(), SelectorMatchability::Invalid);

    let stylesheet = parse_stylesheet_with_options("body span {}", &ParseOptions::stylesheet());
    let diagnostic = document_selector_matching_diagnostic(
        &parsed.document,
        environment,
        &[StylesheetCollectionInput::author(
            StylesheetSourceId::compatibility_generation_index(0),
            StylesheetOrder::new(0),
            &stylesheet,
            StylesheetConditionInput::None,
        )],
        DocumentSelectorMatchingDiagnosticLimits {
            selector_matching: SelectorMatchingLimits {
                max_axis_steps_per_match: 0,
            },
            ..Default::default()
        },
    );
    assert!(matches!(
        diagnostic,
        DocumentSelectorMatchingDiagnostic::Failed(
            DocumentSelectorMatchingDiagnosticFailure::SelectorMatching {
                error: SelectorMatchingLimitError::AxisStepLimitExceeded { limit: 0 },
                ..
            }
        )
    ));
}
