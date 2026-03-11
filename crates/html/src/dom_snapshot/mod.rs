use crate::Node;
use std::fmt;

mod compare;
mod mismatch;
mod serialize;

pub use mismatch::DomMismatch;

#[cfg(test)]
mod tests;

/// Deterministic DOM serialization and equality rules for streaming/corpus tests.
/// Not a public stable format; intended for internal test comparisons.
/// This format is used by HTML5 golden fixture and WPT DOM test comparisons.
///
/// Serialization rules:
/// - Child order follows tree order.
/// - Attributes are rendered in lexical name order with deterministic tie-breaks.
/// - Escaping is platform-independent (`\n`, `\r`, `\t`, `\\`, `\"`, `\u{HEX}`).
///
/// Equivalence rules:
/// - Node kinds must match.
/// - Element names must match.
/// - Attribute list order is not significant; attributes are compared using the
///   same canonical ordering as snapshot serialization.
/// - Text nodes must match exactly (post entity decode).
/// - Comments and doctypes must match exactly.
/// - IDs and empty style vectors can be ignored by options.
#[derive(Clone, Copy, Debug)]
pub struct DomSnapshotOptions {
    pub ignore_ids: bool,
    pub ignore_empty_style: bool,
}

impl Default for DomSnapshotOptions {
    fn default() -> Self {
        Self {
            ignore_ids: true,
            ignore_empty_style: true,
        }
    }
}

#[derive(Debug)]
pub struct DomSnapshot {
    lines: Vec<String>,
}

impl DomSnapshot {
    pub fn new(root: &Node, options: DomSnapshotOptions) -> Self {
        let mut lines = Vec::new();
        let mut indent_level = 0usize;
        serialize::walk_snapshot(root, &options, &mut indent_level, &mut lines);
        Self { lines }
    }

    pub fn as_lines(&self) -> &[String] {
        &self.lines
    }

    pub fn render(&self) -> String {
        self.lines.join("\n")
    }
}

impl fmt::Display for DomSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, line) in self.lines.iter().enumerate() {
            if index != 0 {
                f.write_str("\n")?;
            }
            f.write_str(line)?;
        }
        Ok(())
    }
}

pub fn assert_dom_eq(expected: &Node, actual: &Node, options: DomSnapshotOptions) {
    if let Err(mismatch) = compare_dom(expected, actual, options) {
        panic!("{mismatch}");
    }
}

pub fn compare_dom<'a>(
    expected: &'a Node,
    actual: &'a Node,
    options: DomSnapshotOptions,
) -> Result<(), Box<DomMismatch<'a>>> {
    #[cfg(feature = "parse-guards")]
    crate::parse_guards::record_dom_snapshot_compare();
    let mut path = vec![serialize::node_label(expected)];
    compare::compare_nodes(expected, actual, &options, &mut path)
}
