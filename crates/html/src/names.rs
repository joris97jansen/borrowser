//! Canonical parser-created element and local-name model.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ElementNamespace {
    Html,
    Svg,
    MathMl,
}

impl ElementNamespace {
    pub const fn snapshot_name(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Svg => "svg",
            Self::MathMl => "mathml",
        }
    }
}

/// Opaque identifier bound to one [`NameInterner`] domain.
///
/// The packed representation keeps hot stack/cache keys compact while making
/// raw numeric equality across parser sessions impossible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NameAtomId(u64);

impl NameAtomId {
    fn new(interner_id: u32, index: u32) -> Self {
        Self((u64::from(interner_id) << 32) | u64::from(index))
    }

    pub(crate) fn interner_id(self) -> u32 {
        (self.0 >> 32) as u32
    }

    pub(crate) fn index(self) -> u32 {
        self.0 as u32
    }

    #[cfg(test)]
    pub(crate) fn for_test(interner_id: u32, index: u32) -> Self {
        Self::new(interner_id, index)
    }
}

/// Exact interned local name retained by parser-created DOM values.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InternedLocalName(Arc<str>);

impl InternedLocalName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_arc(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }

    #[cfg(test)]
    pub(crate) fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl AsRef<str> for InternedLocalName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExpandedElementName {
    namespace: ElementNamespace,
    local_name: InternedLocalName,
}

impl ExpandedElementName {
    pub fn new(namespace: ElementNamespace, local_name: InternedLocalName) -> Self {
        Self {
            namespace,
            local_name,
        }
    }

    pub fn namespace(&self) -> ElementNamespace {
        self.namespace
    }

    pub fn local_name(&self) -> &InternedLocalName {
        &self.local_name
    }

    pub fn local_name_str(&self) -> &str {
        self.local_name.as_str()
    }

    pub fn is(&self, namespace: ElementNamespace, local_name: &str) -> bool {
        self.namespace == namespace && self.local_name.as_str() == local_name
    }

    pub fn is_html(&self, local_name: &str) -> bool {
        self.is(ElementNamespace::Html, local_name)
    }
}

/// Per-document exact-name interner.
///
/// HTML tokenizer names enter through ASCII folding. SVG-adjusted names enter
/// through exact interning, so canonical mixed case is retained and allocated
/// at most once per document.
#[derive(Debug)]
pub struct NameInterner {
    id: u32,
    atoms: Vec<Arc<str>>,
    by_text: HashMap<Arc<str>, NameAtomId>,
}

impl NameInterner {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        let id = NEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("name interner identity domain exhausted");
        Self {
            id,
            atoms: Vec::new(),
            by_text: HashMap::new(),
        }
    }

    pub fn intern_ascii_folded(&mut self, name: &str) -> Result<NameAtomId, NameInternerError> {
        let folded = if name.bytes().any(|byte| byte.is_ascii_uppercase()) {
            Cow::Owned(name.to_ascii_lowercase())
        } else {
            Cow::Borrowed(name)
        };
        self.intern_exact(folded.as_ref())
    }

    pub fn intern_exact(&mut self, name: &str) -> Result<NameAtomId, NameInternerError> {
        if let Some(id) = self.by_text.get(name) {
            return Ok(*id);
        }
        let index: u32 = self
            .atoms
            .len()
            .try_into()
            .map_err(|_| NameInternerError::OutOfIds)?;
        let atom = Arc::<str>::from(name);
        let id = NameAtomId::new(self.id, index);
        self.atoms.push(Arc::clone(&atom));
        self.by_text.insert(atom, id);
        Ok(id)
    }

    pub fn intern_html_tag_name_utf8_bytes(
        &mut self,
        name: &[u8],
    ) -> Result<NameAtomId, NameInternerError> {
        let name = std::str::from_utf8(name).map_err(|_| NameInternerError::InvalidUtf8)?;
        self.intern_ascii_folded(name)
    }

    pub fn intern_html_attr_name_utf8_bytes(
        &mut self,
        name: &[u8],
    ) -> Result<NameAtomId, NameInternerError> {
        let name = std::str::from_utf8(name).map_err(|_| NameInternerError::InvalidUtf8)?;
        self.intern_ascii_folded(name)
    }

    /// Legacy HTML-only convenience retained for the legacy parser allocator
    /// tests. General parser code uses the fallible APIs above.
    pub fn intern_ascii_lowercase(&mut self, value: &str) -> NameAtomId {
        assert!(value.is_ascii(), "HTML-only atom input must be ASCII");
        self.intern_ascii_folded(value)
            .expect("legacy HTML atom table exhausted")
    }

    pub fn resolve(&self, id: NameAtomId) -> Option<&str> {
        (id.interner_id() == self.id)
            .then_some(())
            .and_then(|()| self.atoms.get(id.index() as usize))
            .map(AsRef::as_ref)
    }

    pub fn lookup_exact(&self, name: &str) -> Option<NameAtomId> {
        self.by_text.get(name).copied()
    }

    pub fn resolve_arc(&self, id: NameAtomId) -> Option<Arc<str>> {
        (id.interner_id() == self.id)
            .then_some(())
            .and_then(|()| self.atoms.get(id.index() as usize))
            .cloned()
    }

    pub fn resolve_local_name(&self, id: NameAtomId) -> Option<InternedLocalName> {
        self.resolve_arc(id).map(InternedLocalName)
    }

    pub fn expanded_name(
        &self,
        namespace: ElementNamespace,
        id: NameAtomId,
    ) -> Option<ExpandedElementName> {
        self.resolve_local_name(id)
            .map(|local_name| ExpandedElementName::new(namespace, local_name))
    }

    pub fn id(&self) -> u64 {
        u64::from(self.id)
    }

    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    #[cfg(feature = "debug-stats")]
    pub(crate) fn debug_atom_count(&self) -> usize {
        self.atoms.len()
    }

    #[cfg(test)]
    pub(crate) fn debug_validate(&self) {
        debug_assert_eq!(self.atoms.len(), self.by_text.len());
        for (index, atom) in self.atoms.iter().enumerate() {
            debug_assert_eq!(
                self.by_text.get(atom),
                Some(&NameAtomId::new(self.id, index as u32)),
                "name interner map/arena binding mismatch"
            );
        }
    }
}

impl Default for NameInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameInternerError {
    InvalidUtf8,
    OutOfIds,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_and_exact_names_have_distinct_canonical_case() {
        let mut names = NameInterner::new();
        let folded = names.intern_ascii_folded("foreignObject").unwrap();
        let exact = names.intern_exact("foreignObject").unwrap();

        assert_eq!(names.resolve(folded), Some("foreignobject"));
        assert_eq!(names.resolve(exact), Some("foreignObject"));
        assert_ne!(folded, exact);
    }

    #[test]
    fn equal_exact_names_reuse_canonical_storage() {
        let mut names = NameInterner::new();
        let first = names.intern_exact("linearGradient").unwrap();
        let second = names.intern_exact("linearGradient").unwrap();
        let first = names.resolve_local_name(first).unwrap();
        let second = names.resolve_local_name(second).unwrap();

        assert!(first.shares_storage_with(&second));
        assert_eq!(first.as_str(), "linearGradient");
    }

    #[test]
    fn atom_identity_is_bound_to_one_interner_domain() {
        let mut first = NameInterner::new();
        let mut second = NameInterner::new();
        let first_div = first.intern_exact("div").unwrap();
        let second_div = second.intern_exact("div").unwrap();

        assert_ne!(first_div, second_div);
        assert_eq!(first.resolve(first_div), Some("div"));
        assert_eq!(second.resolve(second_div), Some("div"));
        assert_eq!(first.resolve(second_div), None);
        assert_eq!(second.resolve(first_div), None);
    }
}
