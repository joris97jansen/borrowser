use crate::attributes::ParserCreatedAttribute;
use crate::dom_patch::PatchKey;
use crate::names::ExpandedElementName;
use std::collections::{HashMap, HashSet};

use crate::types::ParserCreatedFragmentKind;

#[derive(Clone, Debug)]
pub(crate) enum PatchKind {
    Document {
        doctype: Option<String>,
    },
    DocumentType {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
    },
    Element {
        name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        template_contents: Option<PatchKey>,
    },
    DocumentFragment {
        kind: ParserCreatedFragmentKind,
        host: PatchKey,
    },
    Text {
        text: String,
    },
    Comment {
        text: String,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct PatchNode {
    pub(crate) kind: PatchKind,
    pub(crate) parent: Option<PatchKey>,
    pub(crate) children: Vec<PatchKey>,
}

impl PatchNode {
    pub(crate) fn allows_children(&self) -> bool {
        matches!(
            self.kind,
            PatchKind::Document { .. }
                | PatchKind::Element { .. }
                | PatchKind::DocumentFragment { .. }
        )
    }

    pub(crate) fn template_contents(&self) -> Option<PatchKey> {
        match self.kind {
            PatchKind::Element {
                template_contents, ..
            } => template_contents,
            _ => None,
        }
    }
}

/// Minimal patch-applier/validator shared by runtime-facing parser APIs and
/// test/fuzz harnesses.
///
/// The public validator stages batches for transactional application. Internal
/// trusted callers may instead use the deliberately in-place path when any
/// failure terminates the owning session and the private arena is discarded.
///
/// Allocation policy:
/// - `Clear` resets the live tree state
/// - `Clear` does not release historically allocated patch keys
/// - recreated content must therefore use fresh keys across the whole session
#[derive(Clone, Default)]
pub struct PatchValidationArena {
    pub(crate) nodes: HashMap<PatchKey, PatchNode>,
    pub(crate) allocated: HashSet<PatchKey>,
    pub(crate) root: Option<PatchKey>,
}

#[cfg(feature = "parser-conformance")]
impl PatchValidationArena {
    pub(crate) fn try_invariant_state_for_final_audit(
        &self,
        reserve: &mut impl FnMut(crate::conformance::ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<crate::html5::tree_builder::DomInvariantState, ()> {
        use crate::html5::tree_builder::{
            DomInvariantNode, DomInvariantNodeKind, DomInvariantState,
        };

        let mut maximum = None;
        for key in self.nodes.keys() {
            let index = usize::try_from(key.0).map_err(|_| ())?;
            maximum = Some(maximum.map_or(index, |current: usize| current.max(index)));
        }
        let length = match maximum {
            Some(index) => index.checked_add(1).ok_or(())?,
            None => 0,
        };
        let mut nodes = Vec::new();
        reserve(crate::conformance::ObservationReservationSite::FinalAuditPatchArenaStructuralProjection)?;
        nodes.try_reserve_exact(length).map_err(|_| ())?;
        nodes.resize_with(length, || None);
        for (key, node) in &self.nodes {
            let mut children = Vec::new();
            reserve(crate::conformance::ObservationReservationSite::FinalAuditPatchArenaStructuralProjection)?;
            children
                .try_reserve_exact(node.children.len())
                .map_err(|_| ())?;
            children.extend_from_slice(&node.children);
            let (kind, template_contents, fragment_host, is_template_element) = match &node.kind {
                PatchKind::Document { .. } => (DomInvariantNodeKind::Document, None, None, false),
                PatchKind::DocumentType { .. } => {
                    (DomInvariantNodeKind::DocumentType, None, None, false)
                }
                PatchKind::Element {
                    name,
                    template_contents,
                    ..
                } => (
                    DomInvariantNodeKind::Element,
                    *template_contents,
                    None,
                    name.is(crate::ElementNamespace::Html, "template"),
                ),
                PatchKind::DocumentFragment { kind, host } => (
                    DomInvariantNodeKind::DocumentFragment(*kind),
                    None,
                    Some(*host),
                    false,
                ),
                PatchKind::Text { .. } => (DomInvariantNodeKind::Text, None, None, false),
                PatchKind::Comment { .. } => (DomInvariantNodeKind::Comment, None, None, false),
                PatchKind::ProcessingInstruction { .. } => (
                    DomInvariantNodeKind::ProcessingInstruction,
                    None,
                    None,
                    false,
                ),
            };
            let index = usize::try_from(key.0).map_err(|_| ())?;
            let slot = nodes.get_mut(index).ok_or(())?;
            *slot = Some(DomInvariantNode {
                kind,
                parent: node.parent,
                children,
                template_contents,
                fragment_host,
                is_template_element,
            });
        }
        Ok(DomInvariantState {
            nodes,
            root: self.root,
        })
    }
}
