//! Ordered HTML base facts from the authoritative Browser arena. No URL semantics.
use super::arena::{DomArena, NodeKind};
use html::{AttributeNamespace, DomPatch, ElementNamespace, PatchKey};

pub(crate) enum BaseElementChange {
    Reset,
    Selection(Option<(PatchKey, String)>),
}

pub(crate) struct BaseObservation {
    selected: Option<PatchKey>,
    pub(crate) changes: Vec<BaseElementChange>,
    #[cfg(test)]
    pub(crate) document_scans: usize,
    #[cfg(test)]
    pub(crate) subtree_visits: usize,
    #[cfg(test)]
    pub(crate) ancestry_visits: usize,
    #[cfg(test)]
    pub(crate) sibling_comparisons: usize,
}

pub(super) enum BaseEffect {
    None,
    Reselect,
    Insert(Option<PatchKey>),
    Attributes(PatchKey, Option<String>),
}

impl BaseObservation {
    pub(crate) fn new(selected: Option<PatchKey>) -> Self {
        Self {
            selected,
            changes: Vec::new(),
            #[cfg(test)]
            document_scans: 0,
            #[cfg(test)]
            subtree_visits: 0,
            #[cfg(test)]
            ancestry_visits: 0,
            #[cfg(test)]
            sibling_comparisons: 0,
        }
    }
    pub(super) fn reset(&mut self) {
        self.selected = None;
        self.changes.push(BaseElementChange::Reset);
    }
    pub(super) fn before(&mut self, arena: &DomArena, patch: &DomPatch) -> BaseEffect {
        match patch {
            DomPatch::AppendChild { child, .. } | DomPatch::InsertBefore { child, .. } => {
                // The operation preserves the child's internal subtree. A new
                // or disconnected subtree cannot contain the connected selected
                // base. A connected subtree containing the globally first base
                // necessarily has it as its own first candidate. Inspect the
                // affected subtree once, without walking the selected ancestry.
                let candidate = self.first_in_subtree(arena, *child);
                if candidate.is_some() && candidate == self.selected {
                    BaseEffect::Reselect
                } else {
                    BaseEffect::Insert(candidate)
                }
            }
            DomPatch::RemoveNode { key }
                if self.selected.is_some()
                    && self.first_in_subtree(arena, *key) == self.selected =>
            {
                BaseEffect::Reselect
            }
            DomPatch::SetAttributes { key, .. } if is_base(arena, *key) => {
                BaseEffect::Attributes(*key, href(arena, *key).map(str::to_owned))
            }
            _ => BaseEffect::None,
        }
    }
    pub(super) fn after(&mut self, arena: &DomArena, root: Option<PatchKey>, effect: BaseEffect) {
        match effect {
            BaseEffect::None => {}
            BaseEffect::Reselect => self.reselect(arena, root),
            BaseEffect::Insert(candidate) => {
                if let Some(candidate) = candidate
                    && root.is_some_and(|root| self.contains(arena, root, candidate))
                {
                    self.consider(arena, candidate);
                }
            }
            BaseEffect::Attributes(key, old) => {
                let new = href(arena, key);
                if old.as_deref() == new {
                    return;
                }
                if self.selected == Some(key) {
                    if new.is_some() {
                        self.publish(arena, Some(key));
                    } else {
                        self.reselect(arena, root);
                    }
                } else if new.is_some() && root.is_some_and(|root| self.contains(arena, root, key))
                {
                    self.consider(arena, key);
                }
            }
        }
    }
    fn consider(&mut self, arena: &DomArena, candidate: PatchKey) {
        if self
            .selected
            .is_none_or(|selected| self.precedes(arena, candidate, selected))
        {
            self.publish(arena, Some(candidate));
        }
    }
    fn reselect(&mut self, arena: &DomArena, root: Option<PatchKey>) {
        #[cfg(test)]
        {
            self.document_scans += 1;
        }
        let selected = root.and_then(|root| self.first_in_subtree(arena, root));
        if selected != self.selected {
            self.publish(arena, selected);
        }
    }
    fn publish(&mut self, arena: &DomArena, selected: Option<PatchKey>) {
        self.selected = selected;
        self.changes
            .push(BaseElementChange::Selection(selected.map(|key| {
                (
                    key,
                    href(arena, key).expect("selected base has href").to_owned(),
                )
            })));
    }
    fn first_in_subtree(&mut self, arena: &DomArena, root: PatchKey) -> Option<PatchKey> {
        let mut stack = vec![root];
        while let Some(key) = stack.pop() {
            #[cfg(test)]
            {
                self.subtree_visits += 1;
            }
            if href(arena, key).is_some() {
                return Some(key);
            }
            let node = &arena.nodes[*arena.live.get(&key)?];
            // Hosted template contents are not ordinary children.
            stack.extend(node.children.iter().rev().copied());
        }
        None
    }
}

fn is_base(arena: &DomArena, key: PatchKey) -> bool {
    arena.live.get(&key).is_some_and(|&index| {
        matches!(&arena.nodes[index].kind,
        NodeKind::Element { name, .. } if name.is(ElementNamespace::Html, "base"))
    })
}
fn href(arena: &DomArena, key: PatchKey) -> Option<&str> {
    let NodeKind::Element {
        name, attributes, ..
    } = &arena.nodes[*arena.live.get(&key)?].kind
    else {
        return None;
    };
    if !name.is(ElementNamespace::Html, "base") {
        return None;
    }
    attributes
        .iter()
        .find(|a| a.namespace() == AttributeNamespace::None && a.local_name() == "href")
        .map(html::ParserCreatedAttribute::value)
}
impl BaseObservation {
    #[inline]
    fn visit_ancestry(&mut self) {
        #[cfg(test)]
        {
            self.ancestry_visits += 1;
        }
    }
    #[inline]
    fn compare_sibling(&mut self) {
        #[cfg(test)]
        {
            self.sibling_comparisons += 1;
        }
    }

    fn contains(&mut self, arena: &DomArena, ancestor: PatchKey, mut key: PatchKey) -> bool {
        loop {
            self.visit_ancestry();
            if key == ancestor {
                return true;
            }
            let Some(parent) = arena
                .live
                .get(&key)
                .and_then(|&index| arena.nodes[index].parent)
            else {
                return false;
            };
            key = parent;
        }
    }
    fn precedes(&mut self, arena: &DomArena, a: PatchKey, b: PatchKey) -> bool {
        let mut path = |mut key| {
            let mut path = vec![key];
            self.visit_ancestry();
            while let Some(parent) = arena.nodes[arena.live[&key]].parent {
                self.visit_ancestry();
                path.push(parent);
                key = parent;
            }
            path.reverse();
            path
        };
        let a = path(a);
        let b = path(b);
        let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
        if common == a.len() {
            return a.len() < b.len();
        }
        if common == b.len() {
            return false;
        }
        let siblings = &arena.nodes[arena.live[&a[common - 1]]].children;
        // Ordinary parser appends do not search a growing sibling vector.
        self.compare_sibling();
        if siblings.last() == Some(&a[common]) {
            return false;
        }
        self.compare_sibling();
        if siblings.last() == Some(&b[common]) {
            return true;
        }
        let mut position = |needle| {
            siblings.iter().position(|key| {
                self.compare_sibling();
                *key == needle
            })
        };
        position(a[common]) < position(b[common])
    }
}
