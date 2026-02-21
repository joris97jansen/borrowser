//! Atom table for canonicalized HTML tag/attribute names.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Opaque atom identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AtomId(pub u32);

/// Document-level atom table.
///
/// Invariant: ASCII letters are stored in canonical lowercase form for
/// HTML-namespace matching. Non-ASCII code points are preserved as-is.
#[derive(Debug)]
pub struct AtomTable {
    id: u64,
    atoms: Vec<Arc<str>>,
    map: HashMap<String, AtomId>,
}

impl AtomTable {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            atoms: Vec::new(),
            map: HashMap::new(),
        }
    }

    fn next_id(&self) -> Result<AtomId, AtomError> {
        let idx: u32 = self
            .atoms
            .len()
            .try_into()
            .map_err(|_| AtomError::OutOfIds)?;
        Ok(AtomId(idx))
    }

    /// Intern a name, applying ASCII-lowercase folding for HTML matching.
    pub fn intern_ascii_folded(&mut self, name: &str) -> Result<AtomId, AtomError> {
        if !name.bytes().any(|b| b.is_ascii_uppercase()) {
            if let Some(id) = self.map.get(name) {
                return Ok(*id);
            }
            let atom = Arc::<str>::from(name);
            let id = self.next_id()?;
            self.atoms.push(Arc::clone(&atom));
            self.map.insert(name.to_string(), id);
            return Ok(id);
        }
        let folded = name.to_ascii_lowercase();
        if let Some(id) = self.map.get(folded.as_str()) {
            return Ok(*id);
        }
        let atom = Arc::<str>::from(folded.as_str());
        let id = self.next_id()?;
        self.atoms.push(Arc::clone(&atom));
        self.map.insert(folded, id);
        Ok(id)
    }

    /// Intern a tag name from UTF-8 bytes (HTML parsing).
    ///
    /// Policy: bytes are ASCII-lowercased; non-ASCII bytes are preserved as-is.
    /// Returns `AtomError::InvalidUtf8` if the bytes are not valid UTF-8.
    pub fn intern_html_tag_name_utf8_bytes(&mut self, name: &[u8]) -> Result<AtomId, AtomError> {
        let s = std::str::from_utf8(name).map_err(|_| AtomError::InvalidUtf8)?;
        self.intern_ascii_folded(s)
    }

    /// Intern an attribute name from UTF-8 bytes (HTML parsing).
    ///
    /// Policy: bytes are ASCII-lowercased; non-ASCII bytes are preserved as-is.
    /// Returns `AtomError::InvalidUtf8` if the bytes are not valid UTF-8.
    pub fn intern_html_attr_name_utf8_bytes(&mut self, name: &[u8]) -> Result<AtomId, AtomError> {
        let s = std::str::from_utf8(name).map_err(|_| AtomError::InvalidUtf8)?;
        self.intern_ascii_folded(s)
    }

    pub fn resolve(&self, id: AtomId) -> Option<&str> {
        self.atoms.get(id.0 as usize).map(|s| s.as_ref())
    }

    /// Resolve an atom id to a cloned canonical `Arc<str>`.
    ///
    /// This enables zero-reallocation reuse of interned names in downstream
    /// structures (e.g., patch emission).
    pub fn resolve_arc(&self, id: AtomId) -> Option<Arc<str>> {
        self.atoms.get(id.0 as usize).cloned()
    }

    /// Stable per-instance identifier used to enforce document-level binding
    /// invariants across tokenizer/tree-builder components.
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Default for AtomTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomError {
    InvalidUtf8,
    OutOfIds,
}
