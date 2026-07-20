//! Browser/runtime-owned retained render identity model.
//!
//! These identities describe runtime continuity for render artifacts that may
//! survive across frame updates. They are deliberately separate from DOM node
//! identity and from frame-local layout, paint, traversal, and stacking IDs.

use html::{Node, internal::Id};
use std::collections::{BTreeMap, BTreeSet};

/// Browser/runtime-owned retained identity domain for one active document.
///
/// A full document replacement starts a new domain. Retained render IDs are
/// only comparable within the same domain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RetainedRenderIdentityDomain(u64);

impl RetainedRenderIdentityDomain {
    pub const fn initial() -> Self {
        Self(0)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("retained render identity domain exhausted"),
        )
    }
}

/// Browser/runtime-owned retained render identity.
///
/// This is not a DOM node ID, traversal index, layout `BoxId`, paint operation
/// index, or `StackingContextId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RetainedRenderId(u64);

impl RetainedRenderId {
    #[cfg(test)]
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Currently representable retained render artifact kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedRenderArtifactKind {
    /// A render artifact whose current minimal provenance is a live DOM node.
    DomBackedRenderNode,
}

/// Provenance anchor for a retained render identity.
///
/// DOM identity helps locate the current live artifact, but it is not itself a
/// retained render identity and does not prove continuity across full document
/// replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedRenderAnchor {
    DomNode(Id),
}

/// Deterministic debug-facing retained render identity description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedRenderIdentity {
    pub id: RetainedRenderId,
    pub kind: RetainedRenderArtifactKind,
    pub anchor: RetainedRenderAnchor,
}

/// Private browser/runtime reconciliation key.
///
/// Other subsystems should consume `RetainedRenderIdentity` later if needed,
/// not this key. Keeping it private prevents layout/paint/cache code from
/// depending on the current minimal DOM-anchor matching strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RetainedRenderIdentityKey {
    anchor_node_id: u32,
}

/// Browser/runtime-owned retained render identity allocation state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RetainedRenderIdentityMap {
    domain: RetainedRenderIdentityDomain,
    next_id: u64,
    by_key: BTreeMap<RetainedRenderIdentityKey, RetainedRenderId>,
}

impl RetainedRenderIdentityMap {
    pub(crate) fn new() -> Self {
        Self {
            domain: RetainedRenderIdentityDomain::initial(),
            next_id: 1,
            by_key: BTreeMap::new(),
        }
    }

    pub(crate) fn reset_for_navigation(&mut self) {
        self.domain = RetainedRenderIdentityDomain::initial();
        self.next_id = 1;
        self.by_key.clear();
    }

    pub(crate) fn reset_for_document_replacement(&mut self) {
        self.domain = self.domain.next();
        self.next_id = 1;
        self.by_key.clear();
    }

    pub(crate) fn reconcile_live_dom(&mut self, dom: &Node) {
        let mut live_keys = Vec::new();
        collect_retained_render_identity_keys(dom, &mut live_keys);

        let live_set = live_keys.iter().copied().collect::<BTreeSet<_>>();
        self.by_key.retain(|key, _| live_set.contains(key));

        for key in live_keys {
            if !self.by_key.contains_key(&key) {
                let id = RetainedRenderId(self.next_id);
                self.next_id = self
                    .next_id
                    .checked_add(1)
                    .expect("retained render identity allocator exhausted");
                self.by_key.insert(key, id);
            }
        }
    }

    pub(crate) fn domain(&self) -> RetainedRenderIdentityDomain {
        self.domain
    }

    pub(crate) fn identities(&self) -> Vec<RetainedRenderIdentity> {
        let mut identities = self
            .by_key
            .iter()
            .map(|(key, id)| RetainedRenderIdentity {
                id: *id,
                kind: RetainedRenderArtifactKind::DomBackedRenderNode,
                anchor: RetainedRenderAnchor::DomNode(Id(key.anchor_node_id)),
            })
            .collect::<Vec<_>>();
        identities.sort_by_key(|identity| identity.id);
        identities
    }
}

impl Default for RetainedRenderIdentityMap {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_retained_render_identity_keys(node: &Node, keys: &mut Vec<RetainedRenderIdentityKey>) {
    match node {
        Node::Document { id, children, .. } => {
            push_live_anchor(*id, keys);
            for child in children {
                collect_retained_render_identity_keys(child, keys);
            }
        }
        Node::Element { .. } if html::internal::template_contents(node).is_some() => {}
        Node::Element { element } => {
            push_live_anchor(element.id(), keys);
            for child in element.children() {
                collect_retained_render_identity_keys(child, keys);
            }
        }
        Node::Text { id, .. } => push_live_anchor(*id, keys),
        Node::Comment { .. } | Node::DocumentType { .. } => {}
    }
}

fn push_live_anchor(id: Id, keys: &mut Vec<RetainedRenderIdentityKey>) {
    if id == Id::INVALID {
        return;
    }
    keys.push(RetainedRenderIdentityKey {
        anchor_node_id: id.0,
    });
}

pub(crate) fn retained_render_artifact_kind_debug_label(
    kind: RetainedRenderArtifactKind,
) -> &'static str {
    match kind {
        RetainedRenderArtifactKind::DomBackedRenderNode => "dom-backed-render-node",
    }
}

pub(crate) fn retained_render_anchor_debug_label(anchor: RetainedRenderAnchor) -> String {
    match anchor {
        RetainedRenderAnchor::DomNode(id) => format!("dom-node({})", id.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_created_template_host_and_contents_receive_no_retained_identity() {
        let template = html::internal::template_element_from_parts(
            Id(4),
            html::internal::html_name("template"),
            Vec::new(),
            Vec::new(),
            Id(5),
            vec![Node::Text {
                id: Id(6),
                text: "inert".to_string(),
            }],
            Vec::new(),
        );
        let active = html::internal::node_element_from_parts(
            Id(7),
            html::internal::html_name("p"),
            Vec::new(),
            Vec::new(),
            vec![Node::Text {
                id: Id(8),
                text: "active".to_string(),
            }],
        );
        let document = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![template, active],
        };

        let mut identities = RetainedRenderIdentityMap::new();
        identities.reconcile_live_dom(&document);
        let anchors = identities
            .identities()
            .into_iter()
            .map(|identity| identity.anchor)
            .collect::<Vec<_>>();

        assert_eq!(
            anchors,
            vec![
                RetainedRenderAnchor::DomNode(Id(1)),
                RetainedRenderAnchor::DomNode(Id(7)),
                RetainedRenderAnchor::DomNode(Id(8)),
            ]
        );
    }

    #[test]
    fn mixed_namespace_dom_keeps_retained_identity_independent_from_layout_participation() {
        let foreign_html_child = html::internal::node_element_from_parts(
            Id(6),
            html::internal::html_name("div"),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let foreign_object = html::internal::node_element_from_parts(
            Id(5),
            html::internal::expanded_name(html::ElementNamespace::Svg, "foreignObject"),
            Vec::new(),
            Vec::new(),
            vec![foreign_html_child],
        );
        let svg = html::internal::node_element_from_parts(
            Id(4),
            html::internal::expanded_name(html::ElementNamespace::Svg, "svg"),
            Vec::new(),
            Vec::new(),
            vec![foreign_object],
        );
        let document = Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![html::internal::node_element_from_parts(
                Id(2),
                html::internal::html_name("html"),
                Vec::new(),
                Vec::new(),
                vec![html::internal::node_element_from_parts(
                    Id(3),
                    html::internal::html_name("body"),
                    Vec::new(),
                    Vec::new(),
                    vec![svg],
                )],
            )],
        };
        let mut identities = RetainedRenderIdentityMap::new();

        identities.reconcile_live_dom(&document);

        let anchors = identities
            .identities()
            .into_iter()
            .map(|identity| identity.anchor)
            .collect::<Vec<_>>();
        assert_eq!(
            anchors,
            (1..=6)
                .map(|id| RetainedRenderAnchor::DomNode(Id(id)))
                .collect::<Vec<_>>()
        );
    }
}
