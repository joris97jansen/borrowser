use super::super::{LegacyStyleAttachmentError, attach_styles, try_attach_styles};
use super::support::{document, element, matching_environment, stylesheet};

#[test]
fn attach_styles_projects_structured_winners_into_legacy_dom_style_vector() {
    let stylesheets = vec![stylesheet("div { color: blue !important; color: red; }")];
    let mut dom = element("div", Vec::new(), Vec::new());

    attach_styles(&mut dom, matching_environment(), &stylesheets);

    let html::Node::Element { element } = dom else {
        panic!("expected compatibility element root");
    };
    assert_eq!(element.style(), [("color".to_string(), "blue".to_string())]);
}

#[test]
fn attach_styles_clears_legacy_projection_when_style_resolution_hits_limits() {
    let oversized_inline_style = "color:red;".repeat(8_192);
    let mut dom = document(element(
        "div",
        vec![("style", Some(oversized_inline_style.as_str()))],
        Vec::new(),
    ));
    let html::Node::Document { children, .. } = &mut dom else {
        panic!("expected document");
    };
    let [html::Node::Element { element }] = children.as_mut_slice() else {
        panic!("expected document element");
    };
    element
        .style_mut()
        .push(("color".to_string(), "stale".to_string()));

    attach_styles(&mut dom, matching_environment(), &[]);

    let html::Node::Document { children, .. } = dom else {
        panic!("expected document");
    };
    let [html::Node::Element { element }] = children.as_slice() else {
        panic!("expected document element");
    };
    assert!(element.style().is_empty());
}

#[test]
fn try_attach_styles_preserves_typed_failure_for_authoritative_callers() {
    let oversized_inline_style = "color:red;".repeat(8_192);
    let mut dom = document(element(
        "div",
        vec![("style", Some(oversized_inline_style.as_str()))],
        Vec::new(),
    ));
    assert!(matches!(
        try_attach_styles(&mut dom, matching_environment(), &[]),
        Err(LegacyStyleAttachmentError::StyleResolution(_))
    ));
}

#[test]
fn attach_styles_deliberately_degrades_selector_dom_build_failures() {
    let mut dom = document(document(element("div", Vec::new(), Vec::new())));
    let html::Node::Document { children, .. } = &mut dom else {
        panic!("expected outer document");
    };
    let [
        html::Node::Document {
            children: nested_children,
            ..
        },
    ] = children.as_mut_slice()
    else {
        panic!("expected nested document");
    };
    let [html::Node::Element { element }] = nested_children.as_mut_slice() else {
        panic!("expected nested element");
    };
    element
        .style_mut()
        .push(("color".to_string(), "stale".to_string()));

    attach_styles(&mut dom, matching_environment(), &[]);

    let html::Node::Document { children, .. } = dom else {
        panic!("expected outer document");
    };
    let [
        html::Node::Document {
            children: nested_children,
            ..
        },
    ] = children.as_slice()
    else {
        panic!("expected nested document");
    };
    let [html::Node::Element { element }] = nested_children.as_slice() else {
        panic!("expected nested element");
    };
    assert!(element.style().is_empty());
}
