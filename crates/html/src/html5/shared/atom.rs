//! Atom table for canonicalized HTML tag/attribute names.

use std::collections::HashMap;
use std::sync::Arc;

/// Opaque atom identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AtomId(pub u32);

/// Document-level atom table.
///
/// Invariant: ASCII letters are stored in canonical lowercase form for
/// HTML-namespace matching. Non-ASCII code points are preserved as-is.
#[derive(Debug, Default)]
pub struct AtomTable {
    atoms: Vec<Arc<str>>,
    map: HashMap<String, AtomId>,
}

impl AtomTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a name, applying ASCII-lowercase folding for HTML matching.
    pub fn intern_ascii_folded(&mut self, name: &str) -> AtomId {
        if !name.bytes().any(|b| b.is_ascii_uppercase()) {
            if let Some(id) = self.map.get(name) {
                return *id;
            }
            let atom = Arc::<str>::from(name);
            let id = AtomId(self.atoms.len() as u32);
            self.atoms.push(Arc::clone(&atom));
            self.map.insert(name.to_string(), id);
            return id;
        }
        let folded = name.to_ascii_lowercase();
        if let Some(id) = self.map.get(folded.as_str()) {
            return *id;
        }
        let atom = Arc::<str>::from(folded.as_str());
        let id = AtomId(self.atoms.len() as u32);
        self.atoms.push(Arc::clone(&atom));
        self.map.insert(folded, id);
        id
    }

    /// Intern a tag name from UTF-8 bytes (HTML parsing).
    ///
    /// Policy: bytes are ASCII-lowercased; non-ASCII bytes are preserved as-is.
    /// Returns `AtomError::InvalidUtf8` if the bytes are not valid UTF-8.
    pub fn intern_html_tag_name_utf8_bytes(&mut self, name: &[u8]) -> Result<AtomId, AtomError> {
        let s = std::str::from_utf8(name).map_err(|_| AtomError::InvalidUtf8)?;
        Ok(self.intern_ascii_folded(s))
    }

    /// Intern an attribute name from UTF-8 bytes (HTML parsing).
    ///
    /// Policy: bytes are ASCII-lowercased; non-ASCII bytes are preserved as-is.
    /// Returns `AtomError::InvalidUtf8` if the bytes are not valid UTF-8.
    pub fn intern_html_attr_name_utf8_bytes(&mut self, name: &[u8]) -> Result<AtomId, AtomError> {
        let s = std::str::from_utf8(name).map_err(|_| AtomError::InvalidUtf8)?;
        Ok(self.intern_ascii_folded(s))
    }

    pub fn resolve(&self, id: AtomId) -> Option<&str> {
        self.atoms.get(id.0 as usize).map(|s| s.as_ref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomError {
    InvalidUtf8,
}
