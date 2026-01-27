//! DOM and tokenization implementation types.
//!
//! This module is not intended as the stable public API surface.
//! Publicly supported types are re-exported from `html::lib.rs`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

pub type NodeId = u32;

/// Stable node identity for DOM nodes within a document's lifetime.
///
/// Internal API: consumers should avoid depending on this type directly until
/// patching and ownership contracts stabilize.
///
/// Invariants:
/// - Newly created nodes always receive a fresh ID.
/// - IDs are stable across patches for the document's lifetime.
/// - IDs map 1:1 to live DOM nodes and are never reused within a document lifetime.
/// - When deletion is introduced, deleted IDs are never reused.
/// - IDs are assigned by the owning DOM builder/patch applier.
/// - `0` is reserved to represent "unassigned/invalid" during construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(pub NodeId);

impl Id {
    /// Reserved sentinel for "unassigned/invalid" identity.
    pub const INVALID: Id = Id(0);

    #[allow(dead_code)]
    pub(crate) fn from_key(key: NodeKey) -> Self {
        Id(key.0)
    }
}

/// Patch-layer name for stable node identity.
///
/// Invariants:
/// - Keys are stable for the lifetime of a document.
/// - Keys are never reused within a document lifetime.
/// - When deletion is introduced, deleted keys are never reused.
/// - `NodeKey(0)` is reserved as invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey(pub u32);

impl NodeKey {
    pub const INVALID: NodeKey = NodeKey(0);

    #[allow(dead_code)]
    pub(crate) fn try_from_id(id: Id) -> Result<Self, &'static str> {
        if id == Id::INVALID {
            return Err("Id::INVALID cannot be converted to NodeKey");
        }
        Ok(NodeKey(id.0))
    }
}

impl From<NodeKey> for Id {
    fn from(key: NodeKey) -> Self {
        Id(key.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AtomId(pub u32);

/// Interns ASCII, lowercase tag/attribute names with one allocation per distinct name.
/// Invariants: stored names are canonical ASCII lowercase and interned once per table.
/// Lookup by `&str` is allocation-free via `Borrow<str>` on `Arc<str>`.
#[derive(Debug, Default)]
pub struct AtomTable {
    atoms: Vec<Arc<str>>,
    map: HashMap<Arc<str>, AtomId>,
}

impl AtomTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns ASCII-lowercased tag and attribute names to avoid per-token allocations.
    /// Tradeoff: tokenization carries a per-document atom table for name resolution.
    /// Caller must ensure the input is ASCII-only; non-ASCII is outside current invariants.
    pub fn intern_ascii_lowercase(&mut self, value: &str) -> AtomId {
        assert!(value.is_ascii(), "AtomTable only supports ASCII names");
        let lookup = if value.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(value.to_ascii_lowercase())
        } else {
            Cow::Borrowed(value)
        };

        if let Some(id) = self.map.get(lookup.as_ref()) {
            return *id;
        }

        let atom = Arc::<str>::from(lookup.as_ref());
        debug_assert!(
            self.atoms.len() < u32::MAX as usize,
            "atom table exceeded AtomId capacity"
        );
        let id = AtomId(self.atoms.len() as u32);
        self.atoms.push(Arc::clone(&atom));
        self.map.insert(atom, id);
        id
    }

    pub fn resolve(&self, id: AtomId) -> &str {
        self.atoms
            .get(id.0 as usize)
            .map(|s| s.as_ref())
            .expect("atom id out of range")
    }

    pub fn resolve_arc(&self, id: AtomId) -> Arc<str> {
        Arc::clone(self.atoms.get(id.0 as usize).expect("atom id out of range"))
    }

    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    #[cfg(any(test, debug_assertions))]
    #[allow(dead_code)] // Debug-only invariant checker; may be called ad-hoc
    pub(crate) fn debug_validate(&self) {
        debug_assert_eq!(
            self.atoms.len(),
            self.map.len(),
            "atom table map/vec size mismatch"
        );
        for (idx, atom) in self.atoms.iter().enumerate() {
            let id = AtomId(idx as u32);
            let mapped = self.map.get(atom).expect("atom table missing map entry");
            debug_assert_eq!(*mapped, id, "atom table id mismatch");
            debug_assert!(
                atom.bytes()
                    .all(|b| b.is_ascii() && !b.is_ascii_uppercase()),
                "atom table contains non-ascii or non-lowercase value"
            );
        }
    }
}

#[derive(Debug)]
pub enum Token {
    Doctype(TextPayload),
    StartTag {
        name: AtomId,
        attributes: Vec<(AtomId, Option<AttributeValue>)>,
        self_closing: bool,
    },
    EndTag(AtomId),
    Comment(TextPayload),
    /// Byte range into the tokenizer's source; valid only while the source is append-only.
    /// Dropping prefixes requires a different storage model or shifting ranges.
    TextSpan {
        range: Range<usize>,
    },
    TextOwned {
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub enum AttributeValue {
    /// Byte range into the tokenizer source for values that are unchanged.
    Span { range: Range<usize> },
    /// Allocated value for decoded entities or synthesized values.
    Owned(String),
}

impl AttributeValue {
    pub fn as_str<'a>(&'a self, source: &'a str) -> &'a str {
        match self {
            AttributeValue::Span { range } => {
                debug_assert!(
                    source.is_char_boundary(range.start) && source.is_char_boundary(range.end),
                    "attribute value span must be on UTF-8 boundaries"
                );
                &source[range.clone()]
            }
            AttributeValue::Owned(value) => value.as_str(),
        }
    }

    pub fn into_owned(self, source: &str) -> String {
        match self {
            AttributeValue::Span { range } => source[range].to_string(),
            AttributeValue::Owned(value) => value,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TextPayload {
    /// Byte range into the tokenizer source for values that are unchanged.
    Span { range: Range<usize> },
    /// Allocated value for decoded or synthesized strings.
    Owned(String),
}

impl TextPayload {
    pub fn as_str<'a>(&'a self, source: &'a str) -> &'a str {
        match self {
            TextPayload::Span { range } => {
                debug_assert!(
                    source.is_char_boundary(range.start) && source.is_char_boundary(range.end),
                    "text span must be on UTF-8 boundaries"
                );
                &source[range.clone()]
            }
            TextPayload::Owned(value) => value.as_str(),
        }
    }

    pub fn into_owned(self, source: &str) -> String {
        match self {
            TextPayload::Span { range } => source[range].to_string(),
            TextPayload::Owned(value) => value,
        }
    }
}

#[derive(Debug)]
pub struct TokenStream {
    tokens: Vec<Token>,
    atoms: AtomTable,
    source: Arc<str>,
    text_pool: Vec<String>,
}

impl TokenStream {
    pub fn new(
        tokens: Vec<Token>,
        atoms: AtomTable,
        source: Arc<str>,
        text_pool: Vec<String>,
    ) -> Self {
        Self {
            tokens,
            atoms,
            source,
            text_pool,
        }
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn atoms(&self) -> &AtomTable {
        &self.atoms
    }

    pub fn source(&self) -> &str {
        self.source.as_ref()
    }

    pub fn text(&self, token: &Token) -> Option<&str> {
        match token {
            Token::TextSpan { range } => {
                debug_assert!(
                    self.source.is_char_boundary(range.start)
                        && self.source.is_char_boundary(range.end),
                    "text span must be on UTF-8 boundaries"
                );
                Some(&self.source[range.clone()])
            }
            Token::TextOwned { index } => self.text_pool.get(*index).map(|s| s.as_str()),
            _ => None,
        }
    }

    pub fn attr_value<'a>(&'a self, value: &'a AttributeValue) -> &'a str {
        value.as_str(self.source.as_ref())
    }

    pub fn payload_text<'a>(&'a self, payload: &'a TextPayload) -> &'a str {
        payload.as_str(self.source.as_ref())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Token> {
        self.tokens.iter()
    }
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
        name: Arc<str>,
        // Keep as Vec to preserve source order and allow duplicates; use helpers for lookups.
        attributes: Vec<(Arc<str>, Option<String>)>,
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

    /// Updates only the ID field; must not mutate or reorder child storage.
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

#[cfg(test)]
mod tests {
    use super::AtomTable;
    use std::sync::Arc;

    #[test]
    fn intern_ascii_lowercase_is_case_insensitive() {
        let mut atoms = AtomTable::new();
        let upper = atoms.intern_ascii_lowercase("DIV");
        let mixed = atoms.intern_ascii_lowercase("DiV");
        let lower = atoms.intern_ascii_lowercase("div");
        assert_eq!(upper, mixed);
        assert_eq!(upper, lower);
        assert_eq!(atoms.len(), 1);
    }

    #[test]
    fn intern_stores_canonical_lowercase_value() {
        let mut atoms = AtomTable::new();
        let id = atoms.intern_ascii_lowercase("DiV");
        assert_eq!(atoms.resolve(id), "div");
    }

    #[test]
    fn resolve_arc_reuses_allocation() {
        let mut atoms = AtomTable::new();
        let id = atoms.intern_ascii_lowercase("div");
        let a = atoms.resolve_arc(id);
        let b = atoms.resolve_arc(id);
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn different_atoms_do_not_share_allocation() {
        let mut atoms = AtomTable::new();
        let a_id = atoms.intern_ascii_lowercase("div");
        let b_id = atoms.intern_ascii_lowercase("span");
        assert_ne!(a_id, b_id);
        let a = atoms.resolve_arc(a_id);
        let b = atoms.resolve_arc(b_id);
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn intern_stress_keeps_table_consistent() {
        let mut atoms = AtomTable::new();
        for i in 0..10_000usize {
            let name = format!("tag{i}");
            let id = atoms.intern_ascii_lowercase(&name);
            let upper = name.to_ascii_uppercase();
            let id2 = atoms.intern_ascii_lowercase(&upper);
            assert_eq!(id, id2);
            assert_eq!(atoms.resolve(id), name.as_str());
        }
        assert_eq!(atoms.atoms.len(), 10_000);
        assert_eq!(atoms.map.len(), 10_000);
        atoms.debug_validate();
    }
}
