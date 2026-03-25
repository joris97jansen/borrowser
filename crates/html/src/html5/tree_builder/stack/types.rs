use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;

/// Stable element identity used by Core-v0 tree-builder state.
///
/// Identity is arena-handle based (`PatchKey`) and atom-name based (`AtomId`);
/// no hash maps are required in hot paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ElementIdentity {
    pub(crate) key: PatchKey,
    pub(crate) name: AtomId,
}

/// Entry in the stack of open elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpenElement {
    pub(crate) identity: ElementIdentity,
}

impl OpenElement {
    pub(crate) fn new(key: PatchKey, name: AtomId) -> Self {
        Self {
            identity: ElementIdentity { key, name },
        }
    }

    pub(crate) fn key(self) -> PatchKey {
        self.identity.key
    }

    pub(crate) fn name(self) -> AtomId {
        self.identity.name
    }
}

/// Scope classes required by Core-v0 end-tag handling scaffolding.
///
/// Scope flavor is chosen by the caller algorithm context (for example, an
/// InBody end-tag path), not as a universal property of a tag name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScopeKind {
    /// HTML "in scope" baseline.
    InScope,
    /// HTML "in button scope".
    Button,
    /// HTML "in list-item scope".
    ListItem,
    /// HTML "in table scope".
    Table,
}

/// Atom IDs used to evaluate Core-v0 scope boundaries.
///
/// Core v0 note: this boundary set is intentionally incomplete relative to the
/// full WHATWG algorithm and will be expanded in follow-up milestones.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScopeTagSet {
    pub(crate) html: AtomId,
    pub(crate) table: AtomId,
    pub(crate) template: AtomId,
    pub(crate) td: AtomId,
    pub(crate) th: AtomId,
    pub(crate) caption: AtomId,
    pub(crate) marquee: AtomId,
    pub(crate) object: AtomId,
    pub(crate) applet: AtomId,
    pub(crate) button: AtomId,
    pub(crate) ol: AtomId,
    pub(crate) ul: AtomId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FosterParentingAnchorIndices {
    pub(crate) html_index: Option<usize>,
    pub(crate) table_index: Option<usize>,
    pub(crate) template_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScopeKeyMatch {
    InScope(usize),
    OutOfScope,
    Missing,
}
