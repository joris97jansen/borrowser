use crate::{ParseOptions, parse_stylesheet_with_options};
use html::{Node, internal::Id};

pub(super) fn stylesheet(source: &str) -> crate::model::StylesheetParse {
    parse_stylesheet_with_options(source, &ParseOptions::stylesheet())
}

pub(super) fn element(
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    namespaced_element(html::ElementNamespace::Html, name, attributes, children)
}

pub(super) fn namespaced_element(
    namespace: html::ElementNamespace,
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    html::internal::node_element_from_parts(
        Id::INVALID,
        html::internal::expanded_name(namespace, name),
        attributes
            .into_iter()
            .map(|(name, value)| {
                html::internal::unqualified_attribute(name, value.unwrap_or_default())
            })
            .collect(),
        Vec::new(),
        children,
    )
}
