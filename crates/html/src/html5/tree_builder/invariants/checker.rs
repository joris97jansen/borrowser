use std::collections::HashSet;

use crate::dom_patch::{DomPatch, PatchKey};

use super::errors::{DomInvariantError, PatchInvariantError};
use super::model::{DomInvariantNodeKind, DomInvariantState};

pub fn check_dom_invariants(dom: &DomInvariantState) -> Result<(), DomInvariantError> {
    if dom.root.is_none() && dom.nodes.iter().any(Option::is_some) {
        return Err(DomInvariantError::MissingRootForNonEmptyState);
    }

    if let Some(root) = dom.root {
        let Some(root_node) = dom.node(root) else {
            return Err(DomInvariantError::RootNodeMissing { root });
        };
        if let Some(parent) = root_node.parent {
            return Err(DomInvariantError::RootHasParent { root, parent });
        }
        if !matches!(root_node.kind, DomInvariantNodeKind::Document) {
            return Err(DomInvariantError::RootIsNotDocument {
                root,
                actual: root_node.kind,
            });
        }
        check_document_child_kind_invariants(dom, root, root_node.children())?;
    }

    for (index, maybe_node) in dom.nodes.iter().enumerate() {
        let Some(node) = maybe_node else {
            continue;
        };
        let key = PatchKey(index as u32);

        if matches!(node.kind, DomInvariantNodeKind::Document) && dom.root != Some(key) {
            return Err(DomInvariantError::DocumentNodeNotRoot {
                key,
                actual_parent: node.parent,
            });
        }
        if !node.kind.is_container() && !node.children.is_empty() {
            return Err(DomInvariantError::NonContainerHasChildren {
                key,
                kind: node.kind,
                child_count: node.children.len(),
            });
        }
        if matches!(node.kind, DomInvariantNodeKind::DocumentType) && node.parent != dom.root {
            return Err(DomInvariantError::DocumentTypeNotDocumentChild {
                key,
                actual_parent: node.parent,
            });
        }

        if let Some(contents) = node.template_contents {
            let Some(contents_node) = dom.node(contents) else {
                return Err(DomInvariantError::TemplateAssociation {
                    detail: format!("host {key:?} references missing contents {contents:?}"),
                });
            };
            if contents_node.kind
                != DomInvariantNodeKind::DocumentFragment(
                    crate::types::ParserCreatedFragmentKind::TemplateContents,
                )
                || contents_node.fragment_host != Some(key)
            {
                return Err(DomInvariantError::TemplateAssociation {
                    detail: format!("host {key:?} and contents {contents:?} disagree"),
                });
            }
        }
        if matches!(node.kind, DomInvariantNodeKind::DocumentFragment(_)) {
            if node.parent.is_some() {
                return Err(DomInvariantError::TemplateAssociation {
                    detail: format!("contents root {key:?} has an ordinary parent"),
                });
            }
            let Some(host) = node.fragment_host else {
                return Err(DomInvariantError::TemplateAssociation {
                    detail: format!("contents root {key:?} has no host"),
                });
            };
            if dom.node(host).and_then(|node| node.template_contents()) != Some(key) {
                return Err(DomInvariantError::TemplateAssociation {
                    detail: format!("contents root {key:?} is not referenced by host {host:?}"),
                });
            }
        }

        match node.parent {
            Some(parent) => {
                let Some(parent_node) = dom.node(parent) else {
                    return Err(DomInvariantError::DanglingParentReference { key, parent });
                };
                let matches = parent_node
                    .children
                    .iter()
                    .filter(|child| **child == key)
                    .count();
                if matches != 1 {
                    return Err(DomInvariantError::ParentChildMismatch {
                        key,
                        parent,
                        matches,
                    });
                }
            }
            None if dom.root != Some(key)
                && !matches!(node.kind, DomInvariantNodeKind::DocumentFragment(_)) =>
            {
                return Err(DomInvariantError::DetachedNonRootNode { key });
            }
            None => {}
        }

        let mut unique_children = HashSet::new();
        for child in &node.children {
            let Some(child_node) = dom.node(*child) else {
                return Err(DomInvariantError::DanglingChildReference {
                    parent: key,
                    child: *child,
                });
            };
            if !unique_children.insert(*child) {
                return Err(DomInvariantError::DuplicateChildReference {
                    parent: key,
                    child: *child,
                });
            }
            if child_node.parent != Some(key) {
                return Err(DomInvariantError::ChildParentMismatch {
                    parent: key,
                    child: *child,
                    actual_parent: child_node.parent,
                });
            }
        }
    }

    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    if let Some(root) = dom.root {
        assert_acyclic_from(dom, root, &mut visited, &mut visiting)?;
    }
    for (index, maybe_node) in dom.nodes.iter().enumerate() {
        if maybe_node.is_some() && !visited.contains(&PatchKey(index as u32)) {
            return Err(DomInvariantError::UnreachableNode {
                key: PatchKey(index as u32),
            });
        }
    }

    Ok(())
}

fn check_document_child_kind_invariants(
    dom: &DomInvariantState,
    _root: PatchKey,
    children: &[PatchKey],
) -> Result<(), DomInvariantError> {
    let mut doctype = None;
    let mut first_element = None;
    for child in children {
        let Some(child_node) = dom.node(*child) else {
            continue;
        };
        match child_node.kind {
            DomInvariantNodeKind::DocumentType => {
                if let Some(existing) = doctype {
                    return Err(DomInvariantError::DuplicateDocumentType {
                        existing,
                        duplicate: *child,
                    });
                }
                if let Some(element) = first_element {
                    return Err(DomInvariantError::DocumentTypeAfterDocumentElement {
                        doctype: *child,
                        element,
                    });
                }
                doctype = Some(*child);
            }
            DomInvariantNodeKind::Element if first_element.is_none() => {
                first_element = Some(*child);
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn check_patch_invariants(
    patches: &[DomPatch],
    dom_state: &DomInvariantState,
) -> Result<DomInvariantState, PatchInvariantError> {
    check_dom_invariants(dom_state).map_err(PatchInvariantError::InvalidBaseline)?;

    let mut staged = dom_state.clone();
    let clear_batch = matches!(patches.first(), Some(DomPatch::Clear));

    for (patch_index, patch) in patches.iter().enumerate() {
        match patch {
            DomPatch::Clear => {
                if patch_index != 0 {
                    return Err(PatchInvariantError::ClearMustBeFirst { patch_index });
                }
                staged.clear();
            }
            DomPatch::CreateDocument { key, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Document, patch_index)?;
            }
            DomPatch::CreateDocumentType { key, .. } => {
                staged.insert_created_node(
                    *key,
                    DomInvariantNodeKind::DocumentType,
                    patch_index,
                )?;
            }
            DomPatch::CreateElement { key, name, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Element, patch_index)?;
                staged.mark_element_name(
                    *key,
                    name.is(crate::names::ElementNamespace::Html, "template"),
                )?;
            }
            DomPatch::CreateTemplateContents { host, contents } => {
                staged.apply_create_template_contents(patch_index, *host, *contents)?;
            }
            DomPatch::CreateText { key, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Text, patch_index)?;
            }
            DomPatch::CreateComment { key, .. } => {
                staged.insert_created_node(*key, DomInvariantNodeKind::Comment, patch_index)?;
            }
            DomPatch::AppendChild { parent, child } => {
                staged.apply_append_child(patch_index, *parent, *child)?;
            }
            DomPatch::InsertBefore {
                parent,
                child,
                before,
            } => {
                staged.apply_insert_before(patch_index, *parent, *child, *before)?;
            }
            DomPatch::RemoveNode { key } => {
                staged.apply_remove_node(patch_index, *key)?;
            }
            DomPatch::SetAttributes { key, .. } => {
                staged.apply_kind_checked_patch(
                    patch_index,
                    *key,
                    "SetAttributes",
                    DomInvariantNodeKind::Element,
                )?;
            }
            DomPatch::SetText { key, .. } => {
                staged.apply_kind_checked_patch(
                    patch_index,
                    *key,
                    "SetText",
                    DomInvariantNodeKind::Text,
                )?;
            }
            DomPatch::AppendText { key, .. } => {
                staged.apply_kind_checked_patch(
                    patch_index,
                    *key,
                    "AppendText",
                    DomInvariantNodeKind::Text,
                )?;
            }
        }
    }

    if clear_batch && staged.root.is_none() {
        return Err(PatchInvariantError::ClearBatchMustReestablishDocument);
    }
    check_dom_invariants(&staged).map_err(PatchInvariantError::FinalDomInvariantViolation)?;
    Ok(staged)
}

fn assert_acyclic_from(
    dom: &DomInvariantState,
    key: PatchKey,
    visited: &mut HashSet<PatchKey>,
    visiting: &mut HashSet<PatchKey>,
) -> Result<(), DomInvariantError> {
    if visited.contains(&key) {
        return Ok(());
    }
    if !visiting.insert(key) {
        return Err(DomInvariantError::CycleDetected { key });
    }
    let Some(node) = dom.node(key) else {
        return Err(DomInvariantError::DanglingChildReference {
            parent: key,
            child: key,
        });
    };
    for child in &node.children {
        assert_acyclic_from(dom, *child, visited, visiting)?;
    }
    if let Some(contents) = node.template_contents {
        assert_acyclic_from(dom, contents, visited, visiting)?;
    }
    visiting.remove(&key);
    visited.insert(key);
    Ok(())
}
