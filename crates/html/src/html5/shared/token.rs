//! HTML5 token model.

use super::{AtomId, TextSpan};

/// HTML attribute with interned name and optional value.
///
/// Determinism contract:
/// - Attributes on a `StartTag` are stored in encounter order.
/// - The tokenizer does not sort attributes and does not use hash-based storage.
/// - Duplicate attributes in a single start tag are dropped after the first
///   occurrence (HTML tokenizer "first-wins" behavior).
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

/// Text payload for character/text token emission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextValue {
    /// Borrowed span into the decoded input buffer.
    Span(TextSpan),
    /// Owned value (e.g., after decoding/replacement or compaction).
    Owned(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Doctype {
        /// Name atomized with HTML ASCII-folding rules.
        name: Option<AtomId>,
        /// Core v0 policy: doctype IDs are owned strings.
        ///
        /// Rationale: IDs are sparse in Core v0 and this avoids span-lifetime coupling
        /// for declaration subfields while tokenizer state coverage is still maturing.
        /// Future milestones may migrate these to `TextValue` if compaction/zero-copy
        /// pressure justifies it.
        public_id: Option<String>,
        /// Core v0 policy: doctype IDs are owned strings (see `public_id` note).
        system_id: Option<String>,
        force_quirks: bool,
    },
    StartTag {
        name: AtomId,
        attrs: Vec<Attribute>,
        self_closing: bool,
    },
    EndTag {
        name: AtomId,
    },
    Comment {
        text: TextValue,
    },
    /// Text token in the HTML5 stream.
    ///
    /// Determinism contract:
    /// - Order is source order.
    /// - Text payload storage (`Span` vs `Owned`) is an implementation detail; semantic
    ///   token content must be identical across equivalent runs.
    Text {
        text: TextValue,
    },
    Eof,
}
