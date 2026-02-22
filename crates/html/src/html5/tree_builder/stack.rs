//! Stack of open elements helpers.

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

/// Core-v0 stack of open elements with deterministic push/pop behavior.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenElementsStack {
    items: Vec<OpenElement>,
    max_depth: u32,
}

impl OpenElementsStack {
    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }

    pub(crate) fn push(&mut self, entry: OpenElement) {
        self.items.push(entry);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
    }

    pub(crate) fn current(&self) -> Option<OpenElement> {
        self.items.last().copied()
    }

    #[allow(
        dead_code,
        reason = "part of Core-v0 SOE API; used in test/internal paths"
    )]
    pub(crate) fn pop(&mut self) -> Option<OpenElement> {
        self.items.pop()
    }

    pub(crate) fn max_depth(&self) -> u32 {
        self.max_depth
    }

    #[cfg(any(test, feature = "internal-api"))]
    pub(crate) fn iter_names(&self) -> impl Iterator<Item = AtomId> + '_ {
        self.items.iter().map(|entry| entry.name())
    }

    #[cfg(any(test, feature = "internal-api"))]
    pub(crate) fn iter_keys(&self) -> impl Iterator<Item = PatchKey> + '_ {
        self.items.iter().map(|entry| entry.key())
    }

    #[allow(
        dead_code,
        reason = "part of Core-v0 SOE API; upcoming insertion-mode algorithms use scope probes"
    )]
    pub(crate) fn has_in_scope(&self, target: AtomId, kind: ScopeKind, tags: &ScopeTagSet) -> bool {
        self.find_in_scope_match_index(target, kind, tags).is_some()
    }

    /// Removes elements from the top down to and including `target` when it is
    /// visible in the requested scope, and returns the matched element.
    pub(crate) fn pop_until_including_in_scope(
        &mut self,
        target: AtomId,
        kind: ScopeKind,
        tags: &ScopeTagSet,
    ) -> Option<OpenElement> {
        let match_index = self.find_in_scope_match_index(target, kind, tags)?;
        self.items.truncate(match_index + 1);
        debug_assert!(!self.items.is_empty());
        self.items.pop()
    }

    fn find_in_scope_match_index(
        &self,
        target: AtomId,
        kind: ScopeKind,
        tags: &ScopeTagSet,
    ) -> Option<usize> {
        for index in (0..self.items.len()).rev() {
            let name = self.items[index].name();
            if name == target {
                return Some(index);
            }
            if is_scope_boundary(name, kind, tags) {
                return None;
            }
        }
        None
    }
}

fn is_scope_boundary(name: AtomId, kind: ScopeKind, tags: &ScopeTagSet) -> bool {
    match kind {
        ScopeKind::InScope => {
            name == tags.html
                || name == tags.table
                || name == tags.template
                || name == tags.td
                || name == tags.th
                || name == tags.caption
                || name == tags.marquee
                || name == tags.object
                || name == tags.applet
        }
        ScopeKind::Button => {
            is_scope_boundary(name, ScopeKind::InScope, tags) || name == tags.button
        }
        ScopeKind::ListItem => {
            is_scope_boundary(name, ScopeKind::InScope, tags) || name == tags.ol || name == tags.ul
        }
        ScopeKind::Table => name == tags.html || name == tags.table || name == tags.template,
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenElement, OpenElementsStack, ScopeKind, ScopeTagSet};
    use crate::dom_patch::PatchKey;
    use crate::html5::shared::DocumentParseContext;

    fn make_scope_tags(ctx: &mut DocumentParseContext) -> ScopeTagSet {
        ScopeTagSet {
            html: ctx.atoms.intern_ascii_folded("html").expect("atom"),
            table: ctx.atoms.intern_ascii_folded("table").expect("atom"),
            template: ctx.atoms.intern_ascii_folded("template").expect("atom"),
            td: ctx.atoms.intern_ascii_folded("td").expect("atom"),
            th: ctx.atoms.intern_ascii_folded("th").expect("atom"),
            caption: ctx.atoms.intern_ascii_folded("caption").expect("atom"),
            marquee: ctx.atoms.intern_ascii_folded("marquee").expect("atom"),
            object: ctx.atoms.intern_ascii_folded("object").expect("atom"),
            applet: ctx.atoms.intern_ascii_folded("applet").expect("atom"),
            button: ctx.atoms.intern_ascii_folded("button").expect("atom"),
            ol: ctx.atoms.intern_ascii_folded("ol").expect("atom"),
            ul: ctx.atoms.intern_ascii_folded("ul").expect("atom"),
        }
    }

    #[test]
    fn open_elements_push_pop_and_current_are_deterministic() {
        let mut ctx = DocumentParseContext::new();
        let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
        let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
        let mut stack = OpenElementsStack::default();

        assert!(stack.current().is_none());
        stack.push(OpenElement::new(PatchKey(1), div));
        assert_eq!(stack.current().map(|n| n.key()), Some(PatchKey(1)));
        stack.push(OpenElement::new(PatchKey(2), span));
        assert_eq!(stack.current().map(|n| n.key()), Some(PatchKey(2)));
        assert_eq!(stack.pop().map(|n| n.key()), Some(PatchKey(2)));
        assert_eq!(stack.current().map(|n| n.key()), Some(PatchKey(1)));
    }

    #[test]
    fn open_elements_in_scope_checks_follow_core_v0_boundaries() {
        let mut ctx = DocumentParseContext::new();
        let tags = make_scope_tags(&mut ctx);
        let p = ctx.atoms.intern_ascii_folded("p").expect("atom");
        let li = ctx.atoms.intern_ascii_folded("li").expect("atom");

        let mut stack = OpenElementsStack::default();
        stack.push(OpenElement::new(PatchKey(1), tags.html));
        stack.push(OpenElement::new(PatchKey(2), p));
        assert!(stack.has_in_scope(p, ScopeKind::InScope, &tags));

        // Table is a scope boundary for baseline "in scope", so li below html
        // should not be observable once table is on top.
        stack.push(OpenElement::new(PatchKey(3), tags.table));
        assert!(!stack.has_in_scope(li, ScopeKind::InScope, &tags));

        let mut list_stack = OpenElementsStack::default();
        list_stack.push(OpenElement::new(PatchKey(1), tags.html));
        list_stack.push(OpenElement::new(PatchKey(2), li));
        assert!(list_stack.has_in_scope(li, ScopeKind::ListItem, &tags));
        list_stack.push(OpenElement::new(PatchKey(3), tags.ul));
        assert!(!list_stack.has_in_scope(li, ScopeKind::ListItem, &tags));

        let mut button_stack = OpenElementsStack::default();
        button_stack.push(OpenElement::new(PatchKey(1), tags.html));
        button_stack.push(OpenElement::new(PatchKey(2), p));
        assert!(button_stack.has_in_scope(p, ScopeKind::Button, &tags));
        button_stack.push(OpenElement::new(PatchKey(3), tags.button));
        assert!(!button_stack.has_in_scope(p, ScopeKind::Button, &tags));

        let mut table_scope_stack = OpenElementsStack::default();
        table_scope_stack.push(OpenElement::new(PatchKey(1), tags.html));
        table_scope_stack.push(OpenElement::new(PatchKey(2), p));
        assert!(table_scope_stack.has_in_scope(p, ScopeKind::Table, &tags));
        table_scope_stack.push(OpenElement::new(PatchKey(3), tags.table));
        assert!(!table_scope_stack.has_in_scope(p, ScopeKind::Table, &tags));
    }

    #[test]
    fn open_elements_target_before_boundary_is_visible() {
        let mut ctx = DocumentParseContext::new();
        let tags = make_scope_tags(&mut ctx);
        let p = ctx.atoms.intern_ascii_folded("p").expect("atom");
        let mut stack = OpenElementsStack::default();

        // Target above boundary: should be found.
        stack.push(OpenElement::new(PatchKey(1), tags.html));
        stack.push(OpenElement::new(PatchKey(2), tags.table));
        stack.push(OpenElement::new(PatchKey(3), p));
        assert!(stack.has_in_scope(p, ScopeKind::InScope, &tags));

        // Boundary above target: should hide target.
        stack.push(OpenElement::new(PatchKey(4), tags.template));
        assert!(!stack.has_in_scope(p, ScopeKind::InScope, &tags));
    }

    #[test]
    fn pop_until_including_in_scope_returns_matched_element() {
        let mut ctx = DocumentParseContext::new();
        let tags = make_scope_tags(&mut ctx);
        let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
        let span = ctx.atoms.intern_ascii_folded("span").expect("atom");
        let mut stack = OpenElementsStack::default();
        stack.push(OpenElement::new(PatchKey(1), tags.html));
        stack.push(OpenElement::new(PatchKey(2), div));
        stack.push(OpenElement::new(PatchKey(3), span));

        let popped = stack.pop_until_including_in_scope(div, ScopeKind::InScope, &tags);
        assert_eq!(popped.map(|entry| entry.key()), Some(PatchKey(2)));
        assert_eq!(popped.map(|entry| entry.name()), Some(div));
        assert_eq!(stack.current().map(|entry| entry.key()), Some(PatchKey(1)));

        let not_found = stack.pop_until_including_in_scope(div, ScopeKind::InScope, &tags);
        assert!(not_found.is_none());
    }
}
