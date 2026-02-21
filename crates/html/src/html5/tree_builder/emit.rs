//! Patch emission helpers.

use crate::dom_patch::{DomPatch, PatchKey};
use std::sync::Arc;

pub(crate) fn emit_create_element(
    patches: &mut Vec<DomPatch>,
    key: PatchKey,
    name: Arc<str>,
    attributes: Vec<(Arc<str>, Option<String>)>,
) {
    patches.push(DomPatch::CreateElement {
        key,
        name,
        attributes,
    });
}

pub(crate) fn emit_append_child(patches: &mut Vec<DomPatch>, parent: PatchKey, child: PatchKey) {
    patches.push(DomPatch::AppendChild { parent, child });
}
