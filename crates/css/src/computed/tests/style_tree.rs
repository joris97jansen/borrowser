use super::support::*;
use super::*;
use crate::StylePhaseOutput;

#[test]
fn build_style_tree_with_stylesheets_uses_structured_pipeline_without_mutating_dom() {
    let stylesheets = vec![stylesheet("div { color: blue; } span { width: 5px; }")];
    let dom = element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let styled = build_style_tree_with_stylesheets(&dom, &stylesheets).expect("styled document");

    assert_eq!(styled.style.color(), (0, 0, 255, 255));
    assert_eq!(styled.children[0].style.color(), (0, 0, 255, 255));
    assert_eq!(
        styled.children[0].style.width(),
        Some(LengthPercentage::Length(Length::Px(5.0)))
    );
    let Node::Element { element } = &dom else {
        panic!("expected element");
    };
    assert!(element.style().is_empty());
    let Node::Element { element: child } = &element.children()[0] else {
        panic!("expected child element");
    };
    assert!(child.style().is_empty());
}

#[test]
fn style_tree_preserves_processing_instruction_as_a_non_element_leaf() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><p>before<?Exact-Target alpha ? beta?>after</p></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("PI document parses");
    let styled = build_style_tree(&parsed.document, None);
    let snapshot = StylePhaseOutput::new(styled).to_debug_snapshot();

    assert!(snapshot.contains(
        "kind=processing-instruction target=\"Exact-Target\" data=\"alpha ? beta\" children=0"
    ));
    assert!(snapshot.contains("kind=text text=\"before\""));
    assert!(snapshot.contains("kind=text text=\"after\""));
}

#[test]
fn build_style_tree_from_computed_styles_rejects_mismatched_document_style() {
    let source_dom = element("main", Vec::new(), Vec::new());
    let target_dom = element("section", Vec::new(), Vec::new());
    let computed = compute_document_styles(&source_dom, &[]).expect("computed document");

    let error = match build_style_tree_from_computed_styles(&target_dom, &computed) {
        Ok(_) => panic!("mismatched computed document style must be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        ComputedStyleResolutionError::ComputedElementNameMismatch {
            element_index: 0,
            expected: "section".to_string(),
            actual: "main".to_string(),
        }
    );
}

#[test]
fn build_style_tree_from_computed_styles_rejects_selector_identity_mismatch() {
    let dom = element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );
    let mut computed = compute_document_styles(&dom, &[]).expect("computed document");
    let expected = computed.entries[1].selector_element_id;
    let actual = computed.entries[0].selector_element_id;
    computed.entries[1].selector_element_id = actual;

    let error = match build_style_tree_from_computed_styles(&dom, &computed) {
        Ok(_) => panic!("selector identity mismatch must be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        ComputedStyleResolutionError::ComputedElementIdentityMismatch {
            element_index: 1,
            expected,
            actual,
        }
    );
}

#[test]
fn build_style_tree_from_computed_styles_rejects_namespace_mismatch() {
    let source_dom = element("div", Vec::new(), Vec::new());
    let target_dom = namespaced_element(html::ElementNamespace::Svg, "div", Vec::new(), Vec::new());
    let computed = compute_document_styles(&source_dom, &[]).expect("computed document");

    let error = match build_style_tree_from_computed_styles(&target_dom, &computed) {
        Ok(_) => panic!("namespace-mismatched computed document style must be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        ComputedStyleResolutionError::ComputedElementNamespaceMismatch {
            element_index: 0,
            expected: html::ElementNamespace::Svg,
            actual: html::ElementNamespace::Html,
        }
    );
}

#[test]
fn template_host_is_styled_while_template_contents_are_excluded() {
    let inert_span = element("span", vec![("class", Some("inert"))], Vec::new());
    let dom = html::internal::template_element_from_parts(
        Id(1),
        html::internal::html_name("template"),
        Vec::new(),
        Vec::new(),
        Id(2),
        vec![inert_span],
        Vec::new(),
    );
    let stylesheets = vec![stylesheet(
        "template { color: blue; } span.inert { width: 99px; }",
    )];

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed template host");
    assert_eq!(
        computed.entries.len(),
        1,
        "selector indexing must not cross the template-contents association"
    );
    let styled = build_style_tree_with_stylesheets(&dom, &stylesheets).expect("styled template");
    assert_eq!(styled.style.color(), (0, 0, 255, 255));
    assert!(
        styled.children.is_empty(),
        "active style-tree construction must exclude fragment descendants"
    );
}
