use crate::types::{Id, Node};
use std::sync::Arc;

pub(super) fn element(name: &str, children: Vec<Node>) -> Node {
    Node::Element {
        id: Id::INVALID,
        name: Arc::from(name),
        attributes: Vec::new(),
        style: Vec::new(),
        children,
    }
}

pub(super) fn text(value: &str) -> Node {
    Node::Text {
        id: Id::INVALID,
        text: value.to_string(),
    }
}
