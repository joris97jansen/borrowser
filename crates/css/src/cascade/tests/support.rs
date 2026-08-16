use crate::{ParseOptions, parse_stylesheet_with_options};
use html::{Node, internal::Id};

pub(super) const fn matching_environment() -> crate::SelectorMatchingEnvironment {
    crate::SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks)
}

pub(super) fn stylesheet(source: &str) -> crate::model::StylesheetParse {
    parse_stylesheet_with_options(source, &ParseOptions::stylesheet())
}

pub(super) fn document(root_element: Node) -> Node {
    Node::Document {
        id: Id::INVALID,
        doctype: None,
        children: vec![root_element],
    }
}

pub(super) fn document_element(
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    document(element(name, attributes, children))
}

pub(super) fn namespaced_document_element(
    namespace: html::ElementNamespace,
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    document(namespaced_element(namespace, name, attributes, children))
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
