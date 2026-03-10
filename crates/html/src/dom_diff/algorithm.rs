use super::{DomDiffError, DomDiffState};
use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::{Id, Node};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug)]
enum PrevNodeInfo {
    Document {
        doctype: Option<String>,
        children: Vec<Id>,
        parent: Option<Id>,
    },
    Element {
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
        children: Vec<Id>,
        parent: Option<Id>,
    },
    Text {
        text: String,
        parent: Option<Id>,
    },
    Comment {
        text: String,
        parent: Option<Id>,
    },
}

pub(super) fn diff_dom_with_state_impl(
    prev: &Node,
    next: &Node,
    state: &mut DomDiffState,
) -> Result<Vec<DomPatch>, DomDiffError> {
    if !root_is_compatible(prev, next) {
        let mut next_ids = HashSet::new();
        if !collect_ids(next, &mut next_ids) {
            return Err(DomDiffError::InvalidRoot("duplicate ids in next"));
        }
        state.reset(&next_ids);
        return reset_stream(next);
    }

    let mut prev_map = HashMap::new();
    build_prev_map(prev, None, &mut prev_map);
    let mut next_ids = HashSet::new();
    if !collect_ids(next, &mut next_ids) {
        return Err(DomDiffError::InvalidRoot("duplicate ids in next"));
    }

    let mut patches = Vec::new();
    emit_removals(prev, &next_ids, &mut patches)?;

    let mut need_reset = false;
    emit_updates(
        next,
        None,
        &prev_map,
        &next_ids,
        &state.allocated,
        &mut patches,
        &mut need_reset,
    )?;

    if need_reset {
        state.reset(&next_ids);
        return reset_stream(next);
    }

    state.update_live(&next_ids);
    Ok(patches)
}

pub(super) fn diff_from_empty_impl(
    next: &Node,
    state: &mut DomDiffState,
) -> Result<Vec<DomPatch>, DomDiffError> {
    let mut next_ids = HashSet::new();
    if !collect_ids(next, &mut next_ids) {
        return Err(DomDiffError::InvalidRoot("duplicate ids in next"));
    }
    state.reset(&next_ids);
    reset_stream(next)
}

fn reset_stream(next: &Node) -> Result<Vec<DomPatch>, DomDiffError> {
    let mut patches = vec![DomPatch::Clear];
    emit_create_subtree(next, None, &mut patches)?;
    Ok(patches)
}

fn build_prev_map(node: &Node, parent: Option<Id>, map: &mut HashMap<Id, PrevNodeInfo>) {
    match node {
        Node::Document {
            id,
            doctype,
            children,
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Document {
                    doctype: doctype.clone(),
                    children: children.iter().map(Node::id).collect(),
                    parent,
                },
            );
            for child in children {
                build_prev_map(child, Some(*id), map);
            }
        }
        Node::Element {
            id,
            name,
            attributes,
            children,
            ..
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Element {
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                    children: children.iter().map(Node::id).collect(),
                    parent,
                },
            );
            for child in children {
                build_prev_map(child, Some(*id), map);
            }
        }
        Node::Text { id, text } => {
            map.insert(
                *id,
                PrevNodeInfo::Text {
                    text: text.clone(),
                    parent,
                },
            );
        }
        Node::Comment { id, text } => {
            map.insert(
                *id,
                PrevNodeInfo::Comment {
                    text: text.clone(),
                    parent,
                },
            );
        }
    }
}

fn collect_ids(node: &Node, out: &mut HashSet<Id>) -> bool {
    if !out.insert(node.id()) {
        return false;
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                if !collect_ids(child, out) {
                    return false;
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
    true
}

fn emit_removals(
    node: &Node,
    next_ids: &HashSet<Id>,
    patches: &mut Vec<DomPatch>,
) -> Result<(), DomDiffError> {
    if !next_ids.contains(&node.id()) {
        patches.push(DomPatch::RemoveNode {
            key: patch_key(node.id())?,
        });
        return Ok(());
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_removals(child, next_ids, patches)?;
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
    Ok(())
}

fn emit_updates(
    node: &Node,
    parent_key: Option<PatchKey>,
    prev_map: &HashMap<Id, PrevNodeInfo>,
    next_ids: &HashSet<Id>,
    allocated: &HashSet<Id>,
    patches: &mut Vec<DomPatch>,
    need_reset: &mut bool,
) -> Result<(), DomDiffError> {
    let id = node.id();
    let key = patch_key(id)?;
    let is_new = !prev_map.contains_key(&id);

    if is_new {
        if allocated.contains(&id) {
            *need_reset = true;
            return Ok(());
        }
        emit_create_node(node, key, patches)?;
        if let Some(parent) = parent_key {
            patches.push(DomPatch::AppendChild { parent, child: key });
        } else if !matches!(node, Node::Document { .. }) {
            return Err(DomDiffError::InvalidRoot("root must be Document"));
        }
    } else if let Some(prev) = prev_map.get(&id) {
        let expected_parent = parent_key.map(id_from_patch_key);
        let prev_parent = match prev {
            PrevNodeInfo::Document { parent, .. }
            | PrevNodeInfo::Element { parent, .. }
            | PrevNodeInfo::Text { parent, .. }
            | PrevNodeInfo::Comment { parent, .. } => *parent,
        };
        if prev_parent != expected_parent {
            *need_reset = true;
            return Ok(());
        }
        match (prev, node) {
            (
                PrevNodeInfo::Document { doctype, .. },
                Node::Document {
                    doctype: next_doctype,
                    ..
                },
            ) => {
                if doctype != next_doctype {
                    *need_reset = true;
                    return Ok(());
                }
            }
            (
                PrevNodeInfo::Element {
                    name, attributes, ..
                },
                Node::Element {
                    name: next_name,
                    attributes: next_attrs,
                    ..
                },
            ) => {
                if name != next_name {
                    *need_reset = true;
                    return Ok(());
                }
                if attributes != next_attrs {
                    patches.push(DomPatch::SetAttributes {
                        key,
                        attributes: next_attrs.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Text { text, .. },
                Node::Text {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    patches.push(DomPatch::SetText {
                        key,
                        text: next_text.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Comment { text, .. },
                Node::Comment {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    *need_reset = true;
                    return Ok(());
                }
            }
            _ => {
                *need_reset = true;
                return Ok(());
            }
        }
    }

    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            if !is_new {
                let prev_children_live = match prev_map.get(&id) {
                    Some(PrevNodeInfo::Document { children, .. })
                    | Some(PrevNodeInfo::Element { children, .. }) => children
                        .iter()
                        .copied()
                        .filter(|child| next_ids.contains(child))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                let next_children = children.iter().map(Node::id).collect::<Vec<_>>();
                if next_children.len() < prev_children_live.len() {
                    *need_reset = true;
                    return Ok(());
                }
                if next_children[..prev_children_live.len()] != prev_children_live[..] {
                    *need_reset = true;
                    return Ok(());
                }
            }
            for child in children {
                emit_updates(
                    child,
                    Some(key),
                    prev_map,
                    next_ids,
                    allocated,
                    patches,
                    need_reset,
                )?;
                if *need_reset {
                    return Ok(());
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
    Ok(())
}

fn emit_create_node(
    node: &Node,
    key: PatchKey,
    patches: &mut Vec<DomPatch>,
) -> Result<(), DomDiffError> {
    match node {
        Node::Document { doctype, .. } => {
            patches.push(DomPatch::CreateDocument {
                key,
                doctype: doctype.clone(),
            });
        }
        Node::Element {
            name, attributes, ..
        } => {
            patches.push(DomPatch::CreateElement {
                key,
                name: Arc::clone(name),
                attributes: attributes.clone(),
            });
        }
        Node::Text { text, .. } => {
            patches.push(DomPatch::CreateText {
                key,
                text: text.clone(),
            });
        }
        Node::Comment { text, .. } => {
            patches.push(DomPatch::CreateComment {
                key,
                text: text.clone(),
            });
        }
    }
    Ok(())
}

fn emit_create_subtree(
    node: &Node,
    parent_key: Option<PatchKey>,
    patches: &mut Vec<DomPatch>,
) -> Result<(), DomDiffError> {
    let key = patch_key(node.id())?;
    emit_create_node(node, key, patches)?;
    if let Some(parent) = parent_key {
        patches.push(DomPatch::AppendChild { parent, child: key });
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_create_subtree(child, Some(key), patches)?;
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
    Ok(())
}

fn patch_key(id: Id) -> Result<PatchKey, DomDiffError> {
    if id == Id::INVALID {
        return Err(DomDiffError::InvalidKey(id));
    }
    Ok(PatchKey::from_id(id))
}

fn id_from_patch_key(key: PatchKey) -> Id {
    Id(key.0)
}

fn root_is_compatible(prev: &Node, next: &Node) -> bool {
    matches!(
        (prev, next),
        (Node::Document { id: a, .. }, Node::Document { id: b, .. }) if a == b
    )
}
