use super::{DomSnapshot, DomSnapshotOptions};
use crate::Node;
use std::fmt;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct DomMismatch<'a> {
    pub(super) path: String,
    pub(super) detail: String,
    pub(super) expected: String,
    pub(super) actual: String,
    pub(super) expected_node: &'a Node,
    pub(super) actual_node: &'a Node,
    pub(super) options: DomSnapshotOptions,
    pub(super) expected_subtree: OnceLock<String>,
    pub(super) actual_subtree: OnceLock<String>,
}

impl fmt::Display for DomMismatch<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let expected_subtree = self
            .expected_subtree
            .get_or_init(|| DomSnapshot::new(self.expected_node, self.options).render());
        let actual_subtree = self
            .actual_subtree
            .get_or_init(|| DomSnapshot::new(self.actual_node, self.options).render());
        writeln!(f, "DOM mismatch at {}: {}", self.path, self.detail)?;
        writeln!(f, "expected: {}", self.expected)?;
        writeln!(f, "actual:   {}", self.actual)?;
        writeln!(f, "expected subtree:\n{}", expected_subtree)?;
        writeln!(f, "actual subtree:\n{}", actual_subtree)?;
        Ok(())
    }
}

impl std::error::Error for DomMismatch<'_> {}
