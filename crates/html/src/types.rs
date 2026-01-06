pub type NodeId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(pub NodeId);

#[derive(Debug)]
pub enum Token {
    Doctype(String),
    StartTag {
        name: String,
        attributes: Vec<(String, Option<String>)>,
        self_closing: bool,
    },
    EndTag(String),
    Comment(String),
    Text(String),
}

#[derive(Debug)]
pub enum Node {
    Document {
        id: Id,
        doctype: Option<String>,
        children: Vec<Node>,
    },
    Element {
        id: Id,
        name: String,
        // Keep as Vec to preserve source order and allow duplicates; use helpers for lookups.
        attributes: Vec<(String, Option<String>)>,
        style: Vec<(String, String)>,
        children: Vec<Node>,
    },
    Text {
        id: Id,
        text: String,
    },
    Comment {
        id: Id,
        text: String,
    },
}

impl Node {
    pub fn id(&self) -> Id {
        match self {
            Node::Document { id, .. } => *id,
            Node::Element { id, .. } => *id,
            Node::Text { id, .. } => *id,
            Node::Comment { id, .. } => *id,
        }
    }

    pub fn set_id(&mut self, new_id: Id) {
        match self {
            Node::Document { id, .. } => *id = new_id,
            Node::Element { id, .. } => *id = new_id,
            Node::Text { id, .. } => *id = new_id,
            Node::Comment { id, .. } => *id = new_id,
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Node::Document { children, .. } => Some(children),
            Node::Element { children, .. } => Some(children),
            _ => None,
        }
    }

    pub fn has_attr(&self, name: &str) -> bool {
        matches!(self, Node::Element { attributes, .. }
            if attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)))
    }

    pub fn attr_has_token(&self, attr: &str, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        self.attr(attr)
            .is_some_and(|v| v.split_whitespace().any(|t| t.eq_ignore_ascii_case(token)))
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        match self {
            Node::Element { attributes, .. } => attributes
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .and_then(|(_, v)| v.as_deref()),
            _ => None,
        }
    }
}
