use crate::types::{Id, Node};

pub(super) fn html_name(name: &str) -> crate::ExpandedElementName {
    crate::test_support::html_name(name)
}

pub(super) fn element(name: &str, children: Vec<Node>) -> Node {
    crate::Node::from_element_parts(
        Id::INVALID,
        html_name(name),
        Vec::new(),
        Vec::new(),
        None,
        children,
    )
}

pub(super) fn text(value: &str) -> Node {
    Node::Text {
        id: Id::INVALID,
        text: value.to_string(),
    }
}
