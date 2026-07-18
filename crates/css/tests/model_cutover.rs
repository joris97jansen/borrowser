use css::syntax::{
    parse_stylesheet_with_options as parse_syntax_stylesheet_with_options,
    serialize_stylesheet_parse_for_snapshot as serialize_syntax_stylesheet_parse_for_snapshot,
};
use css::{
    ParseOptions, Rule, StylesheetParse, attach_styles, parse_stylesheet_with_options,
    serialize_stylesheet_parse_for_snapshot,
};
use html::{Node, internal::Id};
use std::sync::Arc;

#[test]
fn crate_root_stylesheet_parser_is_model_first() {
    let parse: StylesheetParse =
        parse_stylesheet_with_options("div { color: blue; }", &ParseOptions::stylesheet());

    assert!(matches!(
        parse.stylesheet.rules.as_slice(),
        [Rule::Style(_)]
    ));
    assert_eq!(
        serialize_stylesheet_parse_for_snapshot(&parse),
        serialize_stylesheet_parse_for_snapshot(&css::model::parse_stylesheet_with_options(
            "div { color: blue; }",
            &ParseOptions::stylesheet(),
        )),
    );

    let syntax_parse =
        parse_syntax_stylesheet_with_options("div { color: blue; }", &ParseOptions::stylesheet());
    assert_ne!(
        serialize_stylesheet_parse_for_snapshot(&parse),
        serialize_syntax_stylesheet_parse_for_snapshot(&syntax_parse),
    );
}

#[test]
fn attach_styles_accepts_model_parse_results_from_root_entrypoint() {
    let stylesheets = vec![
        parse_stylesheet_with_options("div { color: red; }", &ParseOptions::stylesheet()),
        parse_stylesheet_with_options(".hero { color: blue; }", &ParseOptions::stylesheet()),
    ];
    let mut dom = html::internal::node_element_from_parts(
        Id::INVALID,
        Arc::from("div"),
        vec![(Arc::from("class"), Some("hero".to_string()))],
        Vec::new(),
        Vec::new(),
    );

    attach_styles(&mut dom, &stylesheets);

    let Node::Element { element } = dom else {
        panic!("expected element");
    };
    assert_eq!(element.style(), [("color".to_string(), "blue".to_string())]);
}
