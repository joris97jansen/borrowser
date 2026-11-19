use std::collections::HashSet;

use crate::dom_patch::{DomPatch, PatchKey};

use super::error::{ArenaResult, PatchValidationError};
use super::model::{PatchKind, PatchNode, PatchValidationArena};

#[cfg(test)]
mod tests;

/// Temporary storage for the independent complete structural checker.
#[derive(Default)]
pub(super) struct PatchValidationScratch {
    child_keys: Vec<(PatchKey, usize)>,
    visited: HashSet<PatchKey>,
    visiting: HashSet<PatchKey>,
}

impl PatchValidationScratch {
    fn clear(&mut self) {
        self.child_keys.clear();
        self.visited.clear();
        self.visiting.clear();
    }

    fn first_duplicate_child(&mut self, children: &[PatchKey]) -> Option<usize> {
        self.child_keys.clear();
        if children.len() < 2 {
            return None;
        }
        self.child_keys.extend(children.iter().copied().zip(0..));
        self.child_keys.sort_unstable();
        // Keep the original first-error position even when several keys repeat.
        self.child_keys
            .windows(2)
            .filter(|pair| pair[0].0 == pair[1].0)
            .map(|pair| pair[1].1)
            .min()
    }
}

impl PatchValidationArena {
    pub(super) fn complete_trusted_batch(
        &self,
        patches: &[DomPatch],
        document_order: Option<PatchKey>,
    ) -> ArenaResult<()> {
        #[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
        super::work::record(|w| w.completion_batches += 1);
        let Some((root, root_node)) = self.checked_root()? else {
            return Ok(());
        };
        if document_order == Some(root) {
            self.assert_document_child_kind_invariants(root_node)?;
        }
        // Only ordinary creates can leave a detached component. A move can
        // also leave a pre-existing doctype under the wrong parent. Inspect
        // their final live identities: later operations can repair or remove them.
        for patch in patches {
            #[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
            super::work::record(|w| w.completion_patches += 1);
            let key = match patch {
                DomPatch::CreateDocumentType { key, .. }
                | DomPatch::CreateElement { key, .. }
                | DomPatch::CreateText { key, .. }
                | DomPatch::CreateComment { key, .. }
                | DomPatch::CreateProcessingInstruction { key, .. }
                | DomPatch::AppendChild { child: key, .. }
                | DomPatch::InsertBefore { child: key, .. } => key,
                DomPatch::Clear
                | DomPatch::CreateDocument { .. }
                | DomPatch::CreateTemplateContents { .. }
                | DomPatch::RemoveNode { .. }
                | DomPatch::SetAttributes { .. }
                | DomPatch::SetText { .. }
                | DomPatch::AppendText { .. } => continue,
            };
            let Some(node) = self.nodes.get(key) else {
                continue;
            };
            if matches!(node.kind, PatchKind::DocumentType { .. }) && node.parent != Some(root) {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("doctype node {key:?} must be a document child"),
                ));
            }
            if node.parent.is_none() {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("detached non-root node {key:?}"),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn assert_invariants(
        &self,
        scratch: &mut PatchValidationScratch,
    ) -> ArenaResult<()> {
        #[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
        super::work::record(|w| w.full_checks += 1);
        scratch.clear();
        let result = self.assert_invariants_with_scratch(scratch);
        // The inner audit may return early through `?`; never retain its keys.
        scratch.clear();
        result
    }

    fn checked_root(&self) -> ArenaResult<Option<(PatchKey, &PatchNode)>> {
        if self.nodes.is_empty() {
            if self.root.is_some() {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    "root must be absent when no nodes remain",
                ));
            }
            return Ok(None);
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
        Ok(Some((root, root_node)))
    }

    fn assert_invariants_with_scratch(
        &self,
        scratch: &mut PatchValidationScratch,
    ) -> ArenaResult<()> {
        let Some((root, root_node)) = self.checked_root()? else {
            return Ok(());
        };
        self.assert_document_child_kind_invariants(root_node)?;

        for (key, node) in &self.nodes {
            #[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
            super::work::record(|w| w.audit_nodes += 1);
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
                    .filter(|child| {
                        #[cfg(all(
                            test,
                            not(feature = "perf-tests"),
                            not(feature = "count-alloc")
                        ))]
                        super::work::record(|w| w.reverse_comparisons += 1);
                        **child == *key
                    })
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

            let first_duplicate = scratch.first_duplicate_child(&node.children);
            for (index, child) in node.children.iter().enumerate() {
                #[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
                super::work::record(|w| w.audit_edges += 1);
                if !self.nodes.contains_key(child) {
                    return Err(PatchValidationError::new(
                        "post-apply invariants",
                        format!("dangling child reference {child:?} under {key:?}"),
                    ));
                }

                if first_duplicate == Some(index) {
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

        self.assert_acyclic_from(root, &mut scratch.visited, &mut scratch.visiting)?;
        if scratch.visited.len() != self.nodes.len() {
            let mut unreachable = self
                .nodes
                .keys()
                .filter(|key| !scratch.visited.contains(key))
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

    fn assert_document_child_kind_invariants(&self, root_node: &PatchNode) -> ArenaResult<()> {
        let mut doctype = None;
        let mut first_element = None;
        for child in &root_node.children {
            #[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
            super::work::record(|w| w.document_children += 1);
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
