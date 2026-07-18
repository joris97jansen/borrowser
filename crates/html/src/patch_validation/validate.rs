use std::collections::HashSet;

use crate::dom_patch::PatchKey;

use super::error::{ArenaResult, PatchValidationError};
use super::model::{PatchKind, PatchValidationArena};

impl PatchValidationArena {
    pub(crate) fn assert_invariants(&self) -> ArenaResult<()> {
        if self.nodes.is_empty() {
            if self.root.is_some() {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    "root must be absent when no nodes remain",
                ));
            }
            return Ok(());
        }

        let root = self.root.ok_or_else(|| {
            PatchValidationError::new(
                "post-apply invariants",
                "non-empty patch arena must declare a document root",
            )
        })?;

        let root_node = self.nodes.get(&root).ok_or_else(|| {
            PatchValidationError::new("post-apply invariants", "declared root node is missing")
        })?;

        if !matches!(root_node.kind, PatchKind::Document { .. }) {
            return Err(PatchValidationError::new(
                "post-apply invariants",
                format!("root {root:?} must be a document node"),
            ));
        }

        if root_node.parent.is_some() {
            return Err(PatchValidationError::new(
                "post-apply invariants",
                "root node must not have a parent",
            ));
        }
        self.assert_document_child_kind_invariants(root, root_node)?;

        for (key, node) in &self.nodes {
            if matches!(node.kind, PatchKind::Document { .. }) && *key != root {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("document node {key:?} must be the declared root {root:?}"),
                ));
            }
            if !node.allows_children() && !node.children.is_empty() {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("non-container node {key:?} has children"),
                ));
            }
            if matches!(node.kind, PatchKind::DocumentType { .. }) && node.parent != Some(root) {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("doctype node {key:?} must be a document child"),
                ));
            }

            match &node.kind {
                PatchKind::Element {
                    template_contents: Some(contents),
                    ..
                } => {
                    let contents_node = self.nodes.get(contents).ok_or_else(|| {
                        PatchValidationError::new(
                            "post-apply invariants",
                            format!("template host {key:?} has missing contents {contents:?}"),
                        )
                    })?;
                    match contents_node.kind {
                        PatchKind::DocumentFragment {
                            kind: crate::types::ParserCreatedFragmentKind::TemplateContents,
                            host,
                        } if host == *key => {}
                        PatchKind::DocumentFragment { kind, host } => {
                            return Err(PatchValidationError::new(
                                "post-apply invariants",
                                format!(
                                    "template contents {contents:?} association mismatch: expected TemplateContents hosted by {key:?}, found {kind:?} hosted by {host:?}"
                                ),
                            ));
                        }
                        _ => {
                            return Err(PatchValidationError::new(
                                "post-apply invariants",
                                format!(
                                    "template host {key:?} association targets non-fragment {contents:?}"
                                ),
                            ));
                        }
                    }
                }
                PatchKind::DocumentFragment { kind, host } => {
                    if *kind != crate::types::ParserCreatedFragmentKind::TemplateContents {
                        return Err(PatchValidationError::new(
                            "post-apply invariants",
                            format!(
                                "template contents {key:?} has unsupported fragment kind {kind:?}"
                            ),
                        ));
                    }
                    if node.parent.is_some() {
                        return Err(PatchValidationError::new(
                            "post-apply invariants",
                            format!("template contents {key:?} must not have an ordinary parent"),
                        ));
                    }
                    let host_node = self.nodes.get(host).ok_or_else(|| {
                        PatchValidationError::new(
                            "post-apply invariants",
                            format!("template contents {key:?} has missing host {host:?}"),
                        )
                    })?;
                    if host_node.template_contents() != Some(*key) {
                        return Err(PatchValidationError::new(
                            "post-apply invariants",
                            format!(
                                "template contents {key:?} is not associated back from host {host:?}"
                            ),
                        ));
                    }
                }
                _ => {}
            }

            if let Some(parent) = node.parent {
                let parent_node = self.nodes.get(&parent).ok_or_else(|| {
                    PatchValidationError::new(
                        "post-apply invariants",
                        format!("dangling parent reference for {key:?}"),
                    )
                })?;

                let matches = parent_node
                    .children
                    .iter()
                    .filter(|child| **child == *key)
                    .count();

                if matches != 1 {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!(
                            "parent/child mismatch for {key:?}: expected exactly one reference from {parent:?}, found {matches}"
                        ),
                    ));
                }
            } else if *key != root && !matches!(node.kind, PatchKind::DocumentFragment { .. }) {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("detached non-root node {key:?}"),
                ));
            }

            let mut unique_children = HashSet::new();
            for child in &node.children {
                if !self.nodes.contains_key(child) {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!("dangling child reference {child:?} under {key:?}"),
                    ));
                }

                if !unique_children.insert(*child) {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!("duplicate child reference {child:?} under {key:?}"),
                    ));
                }

                let child_parent = self
                    .nodes
                    .get(child)
                    .and_then(|child_node| child_node.parent)
                    .ok_or_else(|| {
                        PatchValidationError::new(
                            "post-apply invariants",
                            format!("child {child:?} missing parent back-reference"),
                        )
                    })?;

                if child_parent != *key {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!(
                            "child {child:?} parent mismatch: expected {key:?}, found {child_parent:?}"
                        ),
                    ));
                }
            }
        }

        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        self.assert_acyclic_from(root, &mut visited, &mut visiting)?;
        if visited.len() != self.nodes.len() {
            let mut unreachable = self
                .nodes
                .keys()
                .filter(|key| !visited.contains(key))
                .copied()
                .collect::<Vec<_>>();
            unreachable.sort_unstable();
            return Err(PatchValidationError::new(
                "post-apply invariants",
                format!("nodes are unreachable from the document full model: {unreachable:?}"),
            ));
        }

        Ok(())
    }

    fn assert_document_child_kind_invariants(
        &self,
        _root: PatchKey,
        root_node: &super::model::PatchNode,
    ) -> ArenaResult<()> {
        let mut doctype = None;
        let mut first_element = None;
        for child in &root_node.children {
            let Some(child_node) = self.nodes.get(child) else {
                continue;
            };
            match child_node.kind {
                PatchKind::DocumentType { .. } => {
                    if let Some(existing) = doctype {
                        return Err(PatchValidationError::new(
                            "post-apply invariants",
                            format!("duplicate doctype nodes {existing:?} and {child:?}"),
                        ));
                    }
                    if let Some(element) = first_element {
                        return Err(PatchValidationError::new(
                            "post-apply invariants",
                            format!(
                                "doctype node {child:?} appears after document element {element:?}"
                            ),
                        ));
                    }
                    doctype = Some(*child);
                }
                PatchKind::Element { .. } if first_element.is_none() => {
                    first_element = Some(*child);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn assert_acyclic_from(
        &self,
        key: PatchKey,
        visited: &mut HashSet<PatchKey>,
        visiting: &mut HashSet<PatchKey>,
    ) -> ArenaResult<()> {
        if visited.contains(&key) {
            return Ok(());
        }

        if !visiting.insert(key) {
            return Err(PatchValidationError::new(
                "post-apply invariants",
                format!("cycle detected at {key:?}"),
            ));
        }

        let node = self.nodes.get(&key).ok_or_else(|| {
            PatchValidationError::new(
                "post-apply invariants",
                format!("missing node during cycle check: {key:?}"),
            )
        })?;

        for child in &node.children {
            self.assert_acyclic_from(*child, visited, visiting)?;
        }
        if let Some(contents) = node.template_contents() {
            self.assert_acyclic_from(contents, visited, visiting)?;
        }

        visiting.remove(&key);
        visited.insert(key);
        Ok(())
    }
}
