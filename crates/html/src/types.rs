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
}
