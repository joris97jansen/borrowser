use super::super::attach_styles;
use super::support::{element, stylesheet};

#[test]
fn attach_styles_projects_structured_winners_into_legacy_dom_style_vector() {
    let stylesheets = vec![stylesheet("div { color: blue !important; color: red; }")];
    let mut dom = element("div", Vec::new(), Vec::new());

    attach_styles(&mut dom, &stylesheets);

    let html::Node::Element { style, .. } = dom else {
        panic!("expected element");
    };
    assert_eq!(style, vec![("color".to_string(), "blue".to_string())]);
}

#[test]
fn attach_styles_clears_legacy_projection_when_style_resolution_hits_limits() {
    let oversized_inline_style = "color:red;".repeat(8_192);
    let mut dom = element(
        "div",
        vec![("style", Some(oversized_inline_style.as_str()))],
        Vec::new(),
    );
    let html::Node::Element { style, .. } = &mut dom else {
        panic!("expected element");
    };
    style.push(("color".to_string(), "stale".to_string()));

    attach_styles(&mut dom, &[]);

    let html::Node::Element { style, .. } = dom else {
        panic!("expected element");
    };
    assert!(style.is_empty());
}
