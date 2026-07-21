use crate::dom_patch::PatchKey;
use crate::types::ParserCreatedFragmentKind;
use crate::{ExpandedElementName, ParserCreatedAttribute};
use std::collections::{HashMap, HashSet};

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
