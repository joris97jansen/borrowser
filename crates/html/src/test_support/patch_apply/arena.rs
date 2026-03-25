use crate::dom_patch::PatchKey;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct TestPatchArena {
    pub(super) nodes: HashMap<PatchKey, TestNode>,
    pub(super) allocated: HashSet<PatchKey>,
    pub(super) root: Option<PatchKey>,
}

#[derive(Clone)]
pub(super) struct TestNode {
    pub(super) kind: TestKind,
    pub(super) parent: Option<PatchKey>,
    pub(super) children: Vec<PatchKey>,
}

#[derive(Clone)]
pub(super) enum TestKind {
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
