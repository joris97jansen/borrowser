use super::{DomDiffError, DomDiffState};
use crate::attributes::ParserCreatedAttribute;
use crate::dom_patch::{DomPatch, PatchKey};
use crate::names::ExpandedElementName;
use crate::traverse::full_model_preorder;
use crate::types::{DocumentFragmentNode, Id, Node};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
enum PrevNodeInfo {
    Document {
        doctype: Option<String>,
        children: Vec<Id>,
        parent: Option<Id>,
    },
    DocumentType {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        parent: Option<Id>,
    },
    Element {
        name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        children: Vec<Id>,
        template_contents: Option<Id>,
        parent: Option<Id>,
    },
    DocumentFragment {
        children: Vec<Id>,
    },
    Text {
        text: String,
        parent: Option<Id>,
    },
    Comment {
        text: String,
        parent: Option<Id>,
    },
    ProcessingInstruction {
        target: String,
        data: String,
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

fn build_prev_map(node: &Node, _parent: Option<Id>, map: &mut HashMap<Id, PrevNodeInfo>) {
    for visit in full_model_preorder(node) {
        let info = match visit.entry {
            crate::traverse::FullModelNodeRef::Node(Node::Document {
                doctype, children, ..
            }) => PrevNodeInfo::Document {
                doctype: doctype.clone(),
                children: children.iter().map(Node::id).collect(),
                parent: visit.container,
            },
            crate::traverse::FullModelNodeRef::Node(Node::DocumentType {
                name,
                public_id,
                system_id,
                ..
            }) => PrevNodeInfo::DocumentType {
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
                parent: visit.container,
            },
            crate::traverse::FullModelNodeRef::Node(Node::Element { element }) => {
                PrevNodeInfo::Element {
                    name: element.expanded_name().clone(),
                    attributes: element.attributes().to_vec(),
                    children: element.children().iter().map(Node::id).collect(),
                    template_contents: element.template_contents().map(DocumentFragmentNode::id),
                    parent: visit.container,
                }
            }
            crate::traverse::FullModelNodeRef::DocumentFragment(fragment) => {
                PrevNodeInfo::DocumentFragment {
                    children: fragment.children().iter().map(Node::id).collect(),
                }
            }
            crate::traverse::FullModelNodeRef::Node(Node::Text { text, .. }) => {
                PrevNodeInfo::Text {
                    text: text.clone(),
                    parent: visit.container,
                }
            }
            crate::traverse::FullModelNodeRef::Node(Node::Comment { text, .. }) => {
                PrevNodeInfo::Comment {
                    text: text.clone(),
                    parent: visit.container,
                }
            }
            crate::traverse::FullModelNodeRef::Node(Node::ProcessingInstruction {
                processing_instruction,
            }) => PrevNodeInfo::ProcessingInstruction {
                target: processing_instruction.target().to_string(),
                data: processing_instruction.data().to_string(),
                parent: visit.container,
            },
        };
        map.insert(visit.entry.id(), info);
    }
}

fn collect_ids(node: &Node, out: &mut HashSet<Id>) -> bool {
    full_model_preorder(node).all(|visit| out.insert(visit.entry.id()))
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
        Node::Document { children, .. } => {
            for child in children {
                emit_removals(child, next_ids, patches)?;
            }
        }
        Node::Element { element } => {
            if let Some(contents) = element.template_contents() {
                for child in contents.children() {
                    emit_removals(child, next_ids, patches)?;
                }
            }
            for child in element.children() {
                emit_removals(child, next_ids, patches)?;
            }
        }
        Node::DocumentType { .. }
        | Node::Text { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. } => {}
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
        if let Node::Element { element } = node
            && let Some(contents) = element.template_contents()
            && allocated.contains(&contents.id())
        {
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
            | PrevNodeInfo::DocumentType { parent, .. }
            | PrevNodeInfo::Element { parent, .. }
            | PrevNodeInfo::Text { parent, .. }
            | PrevNodeInfo::Comment { parent, .. }
            | PrevNodeInfo::ProcessingInstruction { parent, .. } => *parent,
            PrevNodeInfo::DocumentFragment { .. } => {
                *need_reset = true;
                return Ok(());
            }
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
                PrevNodeInfo::DocumentType {
                    name,
                    public_id,
                    system_id,
                    ..
                },
                Node::DocumentType {
                    name: next_name,
                    public_id: next_public_id,
                    system_id: next_system_id,
                    ..
                },
            ) => {
                if name != next_name || public_id != next_public_id || system_id != next_system_id {
                    *need_reset = true;
                    return Ok(());
                }
            }
            (
                PrevNodeInfo::Element {
                    name,
                    attributes,
                    template_contents,
                    ..
                },
                Node::Element { element },
            ) => {
                let next_name = element.expanded_name();
                let next_attrs = element.attributes();
                let next_template_contents = element.template_contents();
                if name != next_name {
                    *need_reset = true;
                    return Ok(());
                }
                if attributes != next_attrs {
                    patches.push(DomPatch::SetAttributes {
                        key,
                        attributes: next_attrs.to_vec(),
                    });
                }
                if *template_contents != next_template_contents.map(DocumentFragmentNode::id) {
                    *need_reset = true;
                    return Ok(());
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
            (
                PrevNodeInfo::ProcessingInstruction { target, data, .. },
                Node::ProcessingInstruction {
                    processing_instruction,
                },
            ) => {
                if target != processing_instruction.target()
                    || data != processing_instruction.data()
                {
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
        Node::Document { children, .. } => {
            if !is_new {
                let prev_children_live = match prev_map.get(&id) {
                    Some(PrevNodeInfo::Document { children, .. }) => children
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
        Node::Element { element } => {
            if let Some(contents) = element.template_contents() {
                emit_fragment_updates(
                    contents, is_new, prev_map, next_ids, allocated, patches, need_reset,
                )?;
                if *need_reset {
                    return Ok(());
                }
            }
            if !is_new {
                let prev_children_live = match prev_map.get(&id) {
                    Some(PrevNodeInfo::Element { children, .. }) => children
                        .iter()
                        .copied()
                        .filter(|child| next_ids.contains(child))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                let next_children = element.children().iter().map(Node::id).collect::<Vec<_>>();
                if next_children.len() < prev_children_live.len()
                    || next_children[..prev_children_live.len()] != prev_children_live[..]
                {
                    *need_reset = true;
                    return Ok(());
                }
            }
            for child in element.children() {
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
        Node::DocumentType { .. }
        | Node::Text { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. } => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_fragment_updates(
    fragment: &DocumentFragmentNode,
    host_is_new: bool,
    prev_map: &HashMap<Id, PrevNodeInfo>,
    next_ids: &HashSet<Id>,
    allocated: &HashSet<Id>,
    patches: &mut Vec<DomPatch>,
    need_reset: &mut bool,
) -> Result<(), DomDiffError> {
    let fragment_id = fragment.id();
    let fragment_key = patch_key(fragment_id)?;
    if !host_is_new {
        let Some(PrevNodeInfo::DocumentFragment { children }) = prev_map.get(&fragment_id) else {
            *need_reset = true;
            return Ok(());
        };
        let live = children
            .iter()
            .copied()
            .filter(|child| next_ids.contains(child))
            .collect::<Vec<_>>();
        let next = fragment.children().iter().map(Node::id).collect::<Vec<_>>();
        if next.len() < live.len() || next[..live.len()] != live[..] {
            *need_reset = true;
            return Ok(());
        }
    }
    for child in fragment.children() {
        emit_updates(
            child,
            Some(fragment_key),
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
        Node::DocumentType {
            name,
            public_id,
            system_id,
            ..
        } => {
            patches.push(DomPatch::CreateDocumentType {
                key,
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            });
        }
        Node::Element { element } => {
            patches.push(DomPatch::CreateElement {
                key,
                name: element.expanded_name().clone(),
                attributes: element.attributes().to_vec(),
            });
            if let Some(contents) = element.template_contents() {
                patches.push(DomPatch::CreateTemplateContents {
                    host: key,
                    contents: patch_key(contents.id())?,
                });
            }
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
        Node::ProcessingInstruction {
            processing_instruction,
        } => {
            patches.push(DomPatch::CreateProcessingInstruction {
                key,
                target: processing_instruction.target().to_string(),
                data: processing_instruction.data().to_string(),
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
        Node::Document { children, .. } => {
            for child in children {
                emit_create_subtree(child, Some(key), patches)?;
            }
        }
        Node::Element { element } => {
            if let Some(contents) = element.template_contents() {
                let contents_key = patch_key(contents.id())?;
                for child in contents.children() {
                    emit_create_subtree(child, Some(contents_key), patches)?;
                }
            }
            for child in element.children() {
                emit_create_subtree(child, Some(key), patches)?;
            }
        }
        Node::DocumentType { .. }
        | Node::Text { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. } => {}
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
