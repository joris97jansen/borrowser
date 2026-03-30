use crate::dom_patch::PatchKey;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(super) enum PatchKind {
    Document {
        doctype: Option<String>,
    },
    Element {
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
    },
    Text {
        text: String,
    },
    Comment {
        text: String,
    },
}

#[derive(Clone, Debug)]
pub(super) struct PatchNode {
    pub(super) kind: PatchKind,
    pub(super) parent: Option<PatchKey>,
    pub(super) children: Vec<PatchKey>,
}

/// Minimal patch-applier/validator for fuzzing and test lanes.
///
/// The arena applies batches atomically, validates the resulting structure after
/// every batch, and can materialize the final simplified DOM when needed.
///
/// Allocation policy:
/// - `Clear` resets the live tree state
/// - `Clear` does not release historically allocated patch keys
/// - recreated content must therefore use fresh keys across the whole session
#[derive(Clone, Default)]
pub struct PatchValidationArena {
    pub(super) nodes: HashMap<PatchKey, PatchNode>,
    pub(super) allocated: HashSet<PatchKey>,
    pub(super) root: Option<PatchKey>,
}
