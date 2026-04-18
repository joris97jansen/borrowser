use crate::{ParseOptions, parse_stylesheet_with_options};
use html::{Node, internal::Id};
use std::sync::Arc;

pub(super) fn stylesheet(source: &str) -> crate::model::StylesheetParse {
    parse_stylesheet_with_options(source, &ParseOptions::stylesheet())
}

pub(super) fn element(
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    Node::Element {
        id: Id::INVALID,
        name: Arc::from(name),
        attributes: attributes
            .into_iter()
            .map(|(name, value)| (Arc::from(name), value.map(str::to_string)))
            .collect(),
        style: Vec::new(),
        children,
    }
}
