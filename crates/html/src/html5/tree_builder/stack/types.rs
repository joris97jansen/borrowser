use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;
use crate::names::ElementNamespace;

/// Opaque semantic cache key for one expanded element name.
///
/// `AtomId` is already bound to its exact-name interner domain, so this is the
/// compact stack/cache projection of `ExpandedElementName`, not a second name
/// identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExpandedNameKey {
    namespace: ElementNamespace,
    local_name: AtomId,
}

impl ExpandedNameKey {
    pub(crate) fn new(namespace: ElementNamespace, local_name: AtomId) -> Self {
        Self {
            namespace,
            local_name,
        }
    }

    pub(crate) fn namespace(self) -> ElementNamespace {
        self.namespace
    }

    pub(crate) fn local_name(self) -> AtomId {
        self.local_name
    }
}

/// Stable element identity used by Core-v0 tree-builder state.
///
/// Identity is arena-handle based (`PatchKey`) and atom-name based (`AtomId`);
/// no hash maps are required in hot paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ElementIdentity {
    pub(crate) key: PatchKey,
    pub(crate) expanded_name: ExpandedNameKey,
}

/// Entry in the stack of open elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpenElement {
    pub(crate) identity: ElementIdentity,
}

/// Result of a semantic stack removal by stable parser identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactOpenElementRemoval {
    pub(crate) removed: OpenElement,
    pub(crate) index: usize,
    pub(crate) was_current: bool,
}

/// Stable result of the single reverse stack scan used by the InBody
/// "any other end tag" algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpenElementMatch {
    pub(crate) index: usize,
    pub(crate) element: OpenElement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InBodyEndTagScan {
    Matched(OpenElementMatch),
    BlockedBySpecial { index: usize, element: OpenElement },
}

impl OpenElement {
    pub(crate) fn new_html(key: PatchKey, name: AtomId) -> Self {
        Self {
            identity: ElementIdentity {
                key,
                expanded_name: ExpandedNameKey::new(ElementNamespace::Html, name),
            },
        }
    }

    pub(crate) fn new_foreign(key: PatchKey, namespace: ElementNamespace, name: AtomId) -> Self {
        debug_assert!(namespace != ElementNamespace::Html);
        Self {
            identity: ElementIdentity {
                key,
                expanded_name: ExpandedNameKey::new(namespace, name),
            },
        }
    }

    pub(crate) fn key(self) -> PatchKey {
        self.identity.key
    }

    pub(crate) fn name(self) -> AtomId {
        self.identity.expanded_name.local_name()
    }

    pub(crate) fn namespace(self) -> ElementNamespace {
        self.identity.expanded_name.namespace()
    }

    pub(crate) fn expanded_name_key(self) -> ExpandedNameKey {
        self.identity.expanded_name
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
/// AE11 namespace-aware scope boundary set from the pinned WHATWG profile.
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
    pub(crate) select: AtomId,
    pub(crate) button: AtomId,
    pub(crate) ol: AtomId,
    pub(crate) ul: AtomId,
    pub(crate) math_mi: AtomId,
    pub(crate) math_mo: AtomId,
    pub(crate) math_mn: AtomId,
    pub(crate) math_ms: AtomId,
    pub(crate) math_mtext: AtomId,
    pub(crate) math_annotation_xml: AtomId,
    pub(crate) svg_foreign_object: AtomId,
    pub(crate) svg_desc: AtomId,
    pub(crate) svg_title: AtomId,
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
