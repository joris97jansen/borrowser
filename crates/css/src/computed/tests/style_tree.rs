use super::support::*;
use super::*;

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
    let Node::Element {
        style, children, ..
    } = &dom
    else {
        panic!("expected element");
    };
    assert!(style.is_empty());
    let Node::Element {
        style: child_style, ..
    } = &children[0]
    else {
        panic!("expected child element");
    };
    assert!(child_style.is_empty());
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
