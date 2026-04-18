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
