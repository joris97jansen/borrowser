use crate::types::{Id, Node};
use std::sync::Arc;

pub(super) fn element(name: &str, children: Vec<Node>) -> Node {
    crate::Node::from_element_parts(
        Id::INVALID,
        Arc::from(name),
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
