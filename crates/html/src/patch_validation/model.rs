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
/// The arena applies batches atomically, validates the resulting structure after
/// every batch, and can materialize the final DOM when needed.
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
