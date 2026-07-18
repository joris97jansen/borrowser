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
            TestKind::Document { .. }
            | TestKind::Element { .. }
            | TestKind::DocumentFragment { .. } => Ok(()),
            TestKind::DocumentType { .. } | TestKind::Text { .. } | TestKind::Comment { .. } => {
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
        let mut visited = HashSet::new();
        self.reaches(child, parent, &mut visited)
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
            if !matches!(
                node.kind,
                TestKind::Document { .. }
                    | TestKind::Element { .. }
                    | TestKind::DocumentFragment { .. }
            ) && !node.children.is_empty()
            {
                return Err(format!("non-container node {key:?} has children"));
            }
            if matches!(node.kind, TestKind::DocumentType { .. }) && node.parent != self.root {
                return Err(format!("doctype node {key:?} must be a document child"));
            }
            match &node.kind {
                TestKind::Element {
                    name,
                    template_contents: Some(contents),
                    ..
                } => {
                    if name.as_ref() != "template" {
                        return Err(format!(
                            "non-template element {key:?} owns template contents"
                        ));
                    }
                    let Some(contents_node) = self.nodes.get(contents) else {
                        return Err(format!("dangling template contents {contents:?}"));
                    };
                    if !matches!(
                        contents_node.kind,
                        TestKind::DocumentFragment {
                            kind: crate::types::ParserCreatedFragmentKind::TemplateContents,
                            host,
                        } if host == *key
                    ) {
                        return Err(format!("template association mismatch for {key:?}"));
                    }
                }
                TestKind::DocumentFragment { kind, host } => {
                    if *kind != crate::types::ParserCreatedFragmentKind::TemplateContents {
                        return Err(format!(
                            "template contents {key:?} has unsupported fragment kind {kind:?}"
                        ));
                    }
                    if node.parent.is_some() {
                        return Err(format!("template contents {key:?} has an ordinary parent"));
                    }
                    let Some(host_node) = self.nodes.get(host) else {
                        return Err(format!("dangling template host {host:?}"));
                    };
                    if !matches!(host_node.kind, TestKind::Element { template_contents: Some(contents), .. } if contents == *key)
                    {
                        return Err(format!(
                            "template contents back-reference mismatch for {key:?}"
                        ));
                    }
                }
                _ => {}
            }
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

        if let Some(root) = self.root {
            let mut reachable = HashSet::new();
            self.collect_reachable(root, &mut reachable)?;
            if reachable.len() != self.nodes.len() {
                return Err("unreachable node in full patch model".to_string());
            }
        } else if !self.nodes.is_empty() {
            return Err("non-empty patch model has no root".to_string());
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
        if let TestKind::Element {
            template_contents: Some(contents),
            ..
        } = node.kind
        {
            self.assert_acyclic_from(contents, visited, visiting)?;
        }
        visiting.remove(&key);
        visited.insert(key);
        Ok(())
    }

    fn reaches(
        &self,
        from: PatchKey,
        target: PatchKey,
        visited: &mut HashSet<PatchKey>,
    ) -> ArenaResult<bool> {
        if from == target {
            return Ok(true);
        }
        if !visited.insert(from) {
            return Ok(false);
        }
        let node = self
            .nodes
            .get(&from)
            .ok_or_else(|| "missing node".to_string())?;
        if let TestKind::Element {
            template_contents: Some(contents),
            ..
        } = node.kind
            && self.reaches(contents, target, visited)?
        {
            return Ok(true);
        }
        for child in &node.children {
            if self.reaches(*child, target, visited)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn collect_reachable(
        &self,
        key: PatchKey,
        reachable: &mut HashSet<PatchKey>,
    ) -> ArenaResult<()> {
        if !reachable.insert(key) {
            return Ok(());
        }
        let node = self
            .nodes
            .get(&key)
            .ok_or_else(|| "missing node".to_string())?;
        if let TestKind::Element {
            template_contents: Some(contents),
            ..
        } = node.kind
        {
            self.collect_reachable(contents, reachable)?;
        }
        for child in &node.children {
            self.collect_reachable(*child, reachable)?;
        }
        Ok(())
    }
}
