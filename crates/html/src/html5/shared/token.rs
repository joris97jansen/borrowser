//! HTML5 token model.

use super::{AtomId, TextSpan};

/// HTML attribute with interned name and optional value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: AtomId,
    pub value: Option<AttributeValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeValue {
    /// Borrowed span into the decoded input buffer.
    Span(TextSpan),
    /// Owned value (e.g., after entity decoding, normalization, or buffer compaction).
    Owned(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Doctype {
        name: Option<AtomId>,
        public_id: Option<String>,
        system_id: Option<String>,
        force_quirks: bool,
    },
    StartTag {
        name: AtomId,
        attributes: Vec<Attribute>,
        self_closing: bool,
    },
    EndTag {
        name: AtomId,
    },
    Comment {
        text: TextSpan,
    },
    Character {
        span: TextSpan,
    },
    Eof,
}
