use super::arena::TestKind;
use super::{ArenaResult, TestPatchArena};
use crate::dom_patch::PatchKey;
use std::collections::HashSet;

impl TestPatchArena {
    pub(super) fn ensure_container(&self, key: PatchKey, context: &str) -> ArenaResult<()> {
        let Some(node) = self.nodes.get(&key) else {
            return Err(format!("missing node in {context}"));
        };
        match node.kind {
            TestKind::Document { .. } | TestKind::Element { .. } => Ok(()),
            TestKind::Text { .. } | TestKind::Comment { .. } => {
                Err(format!("{context} must be a container"))
            }
        }
    }

    pub(super) fn node_parent(&self, key: PatchKey) -> ArenaResult<Option<PatchKey>> {
        self.nodes
            .get(&key)
            .map(|node| node.parent)
            .ok_or_else(|| "missing node".to_string())
    }

    pub(super) fn is_document_node(&self, key: PatchKey) -> ArenaResult<bool> {
        self.nodes
            .get(&key)
            .map(|node| matches!(node.kind, TestKind::Document { .. }))
            .ok_or_else(|| "missing node".to_string())
    }

    pub(super) fn is_document_root_element(&self, key: PatchKey) -> ArenaResult<bool> {
        let Some(root) = self.root else {
            return Ok(false);
        };
        let Some(node) = self.nodes.get(&key) else {
            return Err("missing node".to_string());
        };
        Ok(node.parent == Some(root) && matches!(node.kind, TestKind::Element { .. }))
    }

    pub(super) fn would_create_cycle(
        &self,
        parent: PatchKey,
        child: PatchKey,
    ) -> ArenaResult<bool> {
        let mut cursor = Some(parent);
        while let Some(current) = cursor {
            if current == child {
                return Ok(true);
            }
            cursor = self.node_parent(current)?;
        }
        Ok(false)
    }

    pub(super) fn assert_invariants(&self) -> ArenaResult<()> {
        if let Some(root) = self.root {
            let Some(root_node) = self.nodes.get(&root) else {
                return Err("missing root node".to_string());
            };
            if root_node.parent.is_some() {
                return Err("root node must not have a parent".to_string());
            }
        }

        for (key, node) in &self.nodes {
            if let Some(parent) = node.parent {
                let Some(parent_node) = self.nodes.get(&parent) else {
                    return Err(format!("dangling parent reference for {key:?}"));
                };
                let matches = parent_node
                    .children
                    .iter()
                    .filter(|child| **child == *key)
                    .count();
                if matches != 1 {
                    return Err(format!(
                        "parent/child mismatch for {key:?}: expected exactly one reference from {parent:?}, found {matches}"
                    ));
                }
            }

            let mut unique_children = HashSet::new();
            for child in &node.children {
                if !self.nodes.contains_key(child) {
                    return Err(format!("dangling child reference {child:?} under {key:?}"));
                }
                if !unique_children.insert(*child) {
                    return Err(format!("duplicate child reference {child:?} under {key:?}"));
                }
                let child_parent = self
                    .nodes
                    .get(child)
                    .and_then(|child_node| child_node.parent)
                    .ok_or_else(|| format!("child {child:?} missing parent back-reference"))?;
                if child_parent != *key {
                    return Err(format!(
                        "child {child:?} parent mismatch: expected {key:?}, found {child_parent:?}"
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

    pub(super) fn assert_acyclic_from(
        &self,
        key: PatchKey,
        visited: &mut HashSet<PatchKey>,
        visiting: &mut HashSet<PatchKey>,
    ) -> ArenaResult<()> {
        if visited.contains(&key) {
            return Ok(());
        }
        if !visiting.insert(key) {
            return Err(format!("cycle detected at {key:?}"));
        }
        let node = self
            .nodes
            .get(&key)
            .ok_or_else(|| format!("missing node during cycle check: {key:?}"))?;
        for child in &node.children {
            self.assert_acyclic_from(*child, visited, visiting)?;
        }
        visiting.remove(&key);
        visited.insert(key);
        Ok(())
    }
}
