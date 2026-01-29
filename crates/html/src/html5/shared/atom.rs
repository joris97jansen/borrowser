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
    map: HashMap<Arc<str>, AtomId>,
}

impl AtomTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a name, applying ASCII-lowercase folding for HTML matching.
    pub fn intern_ascii_folded(&mut self, name: &str) -> AtomId {
        let folded = if name.bytes().any(|b| b.is_ascii_uppercase()) {
            name.to_ascii_lowercase()
        } else {
            name.to_string()
        };
        if let Some(id) = self.map.get(folded.as_str()) {
            return *id;
        }
        let atom = Arc::<str>::from(folded.as_str());
        let id = AtomId(self.atoms.len() as u32);
        self.atoms.push(Arc::clone(&atom));
        self.map.insert(atom, id);
        id
    }

    pub fn resolve(&self, id: AtomId) -> Option<&str> {
        self.atoms.get(id.0 as usize).map(|s| s.as_ref())
    }
}
