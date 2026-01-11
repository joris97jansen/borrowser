use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

pub type NodeId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(pub NodeId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AtomId(pub u32);

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
    pub fn intern_ascii_lowercase(&mut self, value: &str) -> AtomId {
        let lookup = if value.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(value.to_ascii_lowercase())
        } else {
            Cow::Borrowed(value)
        };

        if let Some(id) = self.map.get(lookup.as_ref()) {
            return *id;
        }

        let atom = Arc::<str>::from(lookup.as_ref());
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
}

#[derive(Debug)]
pub enum Token {
    Doctype(String),
    StartTag {
        name: AtomId,
        attributes: Vec<(AtomId, Option<String>)>,
        self_closing: bool,
    },
    EndTag(AtomId),
    Comment(String),
    Text(String),
}

#[derive(Debug)]
pub struct TokenStream {
    tokens: Vec<Token>,
    atoms: AtomTable,
}

impl TokenStream {
    pub fn new(tokens: Vec<Token>, atoms: AtomTable) -> Self {
        Self { tokens, atoms }
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn atoms(&self) -> &AtomTable {
        &self.atoms
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
        let lower = atoms.intern_ascii_lowercase("div");
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
}
