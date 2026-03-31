use crate::dom_patch::PatchKey;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) enum PatchKind {
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
pub(crate) struct PatchNode {
    pub(crate) kind: PatchKind,
    pub(crate) parent: Option<PatchKey>,
    pub(crate) children: Vec<PatchKey>,
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
