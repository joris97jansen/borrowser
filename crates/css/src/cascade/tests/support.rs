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
    html::internal::node_element_from_parts(
        Id::INVALID,
        Arc::from(name),
        attributes
            .into_iter()
            .map(|(name, value)| (Arc::from(name), value.map(str::to_string)))
            .collect(),
        Vec::new(),
        children,
    )
}
