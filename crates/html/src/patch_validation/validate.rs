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

        for (key, node) in &self.nodes {
            if matches!(node.kind, PatchKind::Document { .. }) && *key != root {
                return Err(PatchValidationError::new(
                    "post-apply invariants",
                    format!("document node {key:?} must be the declared root {root:?}"),
                ));
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
            } else if *key != root {
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
        for key in self.nodes.keys().copied() {
            self.assert_acyclic_from(key, &mut visited, &mut visiting)?;
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

        visiting.remove(&key);
        visited.insert(key);
        Ok(())
    }
}
