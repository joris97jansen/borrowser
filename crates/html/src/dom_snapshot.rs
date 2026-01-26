use crate::Node;
use std::fmt::{self, Write};
use std::sync::OnceLock;

/// Deterministic DOM serialization and equality rules for streaming/corpus tests.
/// Not a public stable format; intended for internal test comparisons.
///
/// Equivalence rules:
/// - Node kinds must match.
/// - Element names must match.
/// - Attribute list order is significant; names and values must match.
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
        walk_snapshot(root, &options, &mut indent_level, &mut lines);
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
        for (i, line) in self.lines.iter().enumerate() {
            if i != 0 {
                f.write_str("\n")?;
            }
            f.write_str(line)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DomMismatch<'a> {
    path: String,
    detail: String,
    expected: String,
    actual: String,
    expected_node: &'a Node,
    actual_node: &'a Node,
    options: DomSnapshotOptions,
    expected_subtree: OnceLock<String>,
    actual_subtree: OnceLock<String>,
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
    let mut path = vec![node_label(expected)];
    compare_nodes(expected, actual, &options, &mut path)
}

fn compare_nodes<'a>(
    expected: &'a Node,
    actual: &'a Node,
    options: &DomSnapshotOptions,
    path: &mut Vec<String>,
) -> Result<(), Box<DomMismatch<'a>>> {
    match (expected, actual) {
        (
            Node::Document {
                id: expected_id,
                doctype: expected_doctype,
                children: expected_children,
            },
            Node::Document {
                id: actual_id,
                doctype: actual_doctype,
                children: actual_children,
            },
        ) => {
            if !options.ignore_ids && expected_id != actual_id {
                return Err(Box::new(mismatch(
                    path,
                    "document id",
                    expected,
                    actual,
                    options,
                )));
            }
            if expected_doctype != actual_doctype {
                return Err(Box::new(mismatch(
                    path, "doctype", expected, actual, options,
                )));
            }
            compare_children(
                expected,
                actual,
                expected_children,
                actual_children,
                options,
                path,
            )
        }
        (
            Node::Element {
                id: expected_id,
                name: expected_name,
                attributes: expected_attrs,
                style: expected_style,
                children: expected_children,
            },
            Node::Element {
                id: actual_id,
                name: actual_name,
                attributes: actual_attrs,
                style: actual_style,
                children: actual_children,
            },
        ) => {
            if !options.ignore_ids && expected_id != actual_id {
                return Err(Box::new(mismatch(
                    path,
                    "element id",
                    expected,
                    actual,
                    options,
                )));
            }
            if expected_name != actual_name {
                return Err(Box::new(mismatch(
                    path,
                    "element name",
                    expected,
                    actual,
                    options,
                )));
            }
            if expected_attrs.len() != actual_attrs.len() {
                return Err(Box::new(mismatch(
                    path,
                    "attribute count",
                    expected,
                    actual,
                    options,
                )));
            }
            for (i, (exp, act)) in expected_attrs.iter().zip(actual_attrs.iter()).enumerate() {
                if exp.0 != act.0 {
                    return Err(Box::new(mismatch(
                        path,
                        &format!("attribute name at index {i}"),
                        expected,
                        actual,
                        options,
                    )));
                }
                if exp.1 != act.1 {
                    return Err(Box::new(mismatch(
                        path,
                        &format!("attribute value at index {i}"),
                        expected,
                        actual,
                        options,
                    )));
                }
            }
            let ignore_style =
                options.ignore_empty_style && expected_style.is_empty() && actual_style.is_empty();
            if !ignore_style {
                if expected_style.len() != actual_style.len() {
                    return Err(Box::new(mismatch(
                        path,
                        "style entry count",
                        expected,
                        actual,
                        options,
                    )));
                }
                for (i, (exp, act)) in expected_style.iter().zip(actual_style.iter()).enumerate() {
                    if exp != act {
                        return Err(Box::new(mismatch(
                            path,
                            &format!("style entry at index {i}"),
                            expected,
                            actual,
                            options,
                        )));
                    }
                }
            }
            compare_children(
                expected,
                actual,
                expected_children,
                actual_children,
                options,
                path,
            )
        }
        (
            Node::Text {
                id: expected_id,
                text: expected_text,
            },
            Node::Text {
                id: actual_id,
                text: actual_text,
            },
        ) => {
            if !options.ignore_ids && expected_id != actual_id {
                return Err(Box::new(mismatch(
                    path, "text id", expected, actual, options,
                )));
            }
            if expected_text != actual_text {
                return Err(Box::new(mismatch(path, "text", expected, actual, options)));
            }
            Ok(())
        }
        (
            Node::Comment {
                id: expected_id,
                text: expected_text,
            },
            Node::Comment {
                id: actual_id,
                text: actual_text,
            },
        ) => {
            if !options.ignore_ids && expected_id != actual_id {
                return Err(Box::new(mismatch(
                    path,
                    "comment id",
                    expected,
                    actual,
                    options,
                )));
            }
            if expected_text != actual_text {
                return Err(Box::new(mismatch(
                    path, "comment", expected, actual, options,
                )));
            }
            Ok(())
        }
        _ => Err(Box::new(mismatch(
            path,
            "node kind",
            expected,
            actual,
            options,
        ))),
    }
}

fn compare_children<'a>(
    expected_parent: &'a Node,
    actual_parent: &'a Node,
    expected: &'a [Node],
    actual: &'a [Node],
    options: &DomSnapshotOptions,
    path: &mut Vec<String>,
) -> Result<(), Box<DomMismatch<'a>>> {
    if expected.len() != actual.len() {
        return Err(Box::new(mismatch(
            path,
            &format!(
                "child count (expected {}, actual {})",
                expected.len(),
                actual.len()
            ),
            expected_parent,
            actual_parent,
            options,
        )));
    }
    for (idx, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
        path.push(format!("{}[{}]", node_label(exp), idx));
        let result = compare_nodes(exp, act, options, path);
        path.pop();
        result?;
    }
    Ok(())
}

fn mismatch<'a>(
    path: &[String],
    detail: &str,
    expected: &'a Node,
    actual: &'a Node,
    options: &DomSnapshotOptions,
) -> DomMismatch<'a> {
    let path = format!("/{}", path.join("/"));
    let expected_line = format_node_line(expected, options);
    let actual_line = format_node_line(actual, options);
    DomMismatch {
        path,
        detail: detail.to_string(),
        expected: truncate_line(expected_line, 160),
        actual: truncate_line(actual_line, 160),
        expected_node: expected,
        actual_node: actual,
        options: *options,
        expected_subtree: OnceLock::new(),
        actual_subtree: OnceLock::new(),
    }
}

fn node_label(node: &Node) -> String {
    match node {
        Node::Document { .. } => "#document".to_string(),
        Node::Element {
            name, attributes, ..
        } => {
            let mut label = String::from(name.as_ref());
            let id_attr = attributes
                .iter()
                .find(|(key, _)| key.as_ref() == "id")
                .and_then(|(_, value)| value.as_deref())
                .filter(|value| !value.is_empty());
            let class_attr = attributes
                .iter()
                .find(|(key, _)| key.as_ref() == "class")
                .and_then(|(_, value)| value.as_deref())
                .filter(|value| !value.is_empty());
            if let Some(id_value) = id_attr {
                label.push('#');
                write_escaped(&mut label, id_value);
            } else if let Some(class_value) = class_attr {
                label.push_str(".class=");
                write_escaped(&mut label, class_value);
            }
            label
        }
        Node::Text { .. } => "#text".to_string(),
        Node::Comment { .. } => "#comment".to_string(),
    }
}

fn truncate_line(mut line: String, max_len: usize) -> String {
    if line.len() > max_len {
        line.truncate(max_len.saturating_sub(3));
        line.push_str("...");
    }
    line
}

fn walk_snapshot(
    node: &Node,
    options: &DomSnapshotOptions,
    indent_level: &mut usize,
    out: &mut Vec<String>,
) {
    let mut line = String::new();
    const INDENT_STEP: usize = 2;
    let spaces = indent_level.saturating_mul(INDENT_STEP);
    #[allow(clippy::manual_repeat_n)]
    line.extend(std::iter::repeat(' ').take(spaces));
    write_node_line(&mut line, node, options);
    out.push(line);
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            *indent_level += 1;
            for child in children {
                walk_snapshot(child, options, indent_level, out);
            }
            debug_assert!(
                *indent_level > 0,
                "indent level underflow at {}",
                node_label(node)
            );
            *indent_level -= 1;
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn format_node_line(node: &Node, options: &DomSnapshotOptions) -> String {
    let mut line = String::new();
    write_node_line(&mut line, node, options);
    line
}

fn write_node_line(out: &mut String, node: &Node, options: &DomSnapshotOptions) {
    match node {
        Node::Document { doctype, id, .. } => {
            out.push_str("#document");
            if let Some(dt) = doctype {
                out.push_str(" doctype=\"");
                write_escaped(out, dt);
                out.push('"');
            }
            if !options.ignore_ids {
                out.push_str(" id=");
                let _ = write!(out, "{}", id.0);
            }
        }
        Node::Element {
            id,
            name,
            attributes,
            style,
            ..
        } => {
            out.push('<');
            out.push_str(name);
            for (attr, value) in attributes {
                out.push(' ');
                out.push_str(attr);
                if let Some(value) = value {
                    out.push('=');
                    out.push('"');
                    write_escaped(out, value);
                    out.push('"');
                }
            }
            if !options.ignore_ids {
                out.push_str(" data-node-id=\"");
                write!(out, "{}", id.0).ok();
                out.push('"');
            }
            let include_style = !(options.ignore_empty_style && style.is_empty());
            if include_style {
                out.push_str(" style=[");
                for (i, (k, v)) in style.iter().enumerate() {
                    if i != 0 {
                        out.push_str("; ");
                    }
                    out.push_str(k);
                    out.push_str(": ");
                    write_escaped(out, v);
                }
                out.push(']');
            }
            out.push('>');
        }
        Node::Text { text, id } => {
            out.push('"');
            write_escaped(out, text);
            out.push('"');
            if !options.ignore_ids {
                out.push_str(" id=");
                write!(out, "{}", id.0).ok();
            }
        }
        Node::Comment { text, id } => {
            out.push_str("<!-- ");
            write_escaped(out, text);
            out.push_str(" -->");
            if !options.ignore_ids {
                out.push_str(" id=");
                write!(out, "{}", id.0).ok();
            }
        }
    }
}

fn write_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ if ch.is_ascii() => out.push(ch),
            _ => {
                let _ = write!(out, "\\u{{{:X}}}", ch as u32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DomSnapshotOptions, assert_dom_eq, compare_dom};
    use crate::Node;
    use crate::types::Id;
    use std::sync::Arc;

    fn elem(name: &str, children: Vec<Node>) -> Node {
        Node::Element {
            id: Id(0),
            name: Arc::from(name),
            attributes: vec![(Arc::from("class"), Some("a b".to_string()))],
            style: Vec::new(),
            children,
        }
    }

    #[test]
    fn dom_eq_ignores_ids_by_default() {
        let expected = Node::Document {
            id: Id(1),
            doctype: Some("html".to_string()),
            children: vec![elem(
                "div",
                vec![Node::Text {
                    id: Id(2),
                    text: "hi".to_string(),
                }],
            )],
        };
        let actual = Node::Document {
            id: Id(99),
            doctype: Some("html".to_string()),
            children: vec![elem(
                "div",
                vec![Node::Text {
                    id: Id(77),
                    text: "hi".to_string(),
                }],
            )],
        };
        assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
    }

    #[test]
    fn dom_mismatch_points_to_text() {
        let expected = Node::Document {
            id: Id(0),
            doctype: None,
            children: vec![elem(
                "p",
                vec![Node::Text {
                    id: Id(0),
                    text: "a".to_string(),
                }],
            )],
        };
        let actual = Node::Document {
            id: Id(0),
            doctype: None,
            children: vec![elem(
                "p",
                vec![Node::Text {
                    id: Id(0),
                    text: "b".to_string(),
                }],
            )],
        };
        let err = compare_dom(&expected, &actual, DomSnapshotOptions::default())
            .expect_err("expected mismatch");
        assert!(err.to_string().contains("/#document"));
        assert!(err.to_string().contains("#text"));
    }

    #[test]
    fn dom_mismatch_path_includes_id_label() {
        let expected = Node::Document {
            id: Id(0),
            doctype: None,
            children: vec![Node::Element {
                id: Id(0),
                name: Arc::from("div"),
                attributes: vec![(Arc::from("id"), Some("main".to_string()))],
                style: Vec::new(),
                children: vec![Node::Text {
                    id: Id(0),
                    text: "a".to_string(),
                }],
            }],
        };
        let actual = Node::Document {
            id: Id(0),
            doctype: None,
            children: vec![Node::Element {
                id: Id(0),
                name: Arc::from("div"),
                attributes: vec![(Arc::from("id"), Some("main".to_string()))],
                style: Vec::new(),
                children: vec![Node::Text {
                    id: Id(0),
                    text: "b".to_string(),
                }],
            }],
        };
        let err = compare_dom(&expected, &actual, DomSnapshotOptions::default())
            .expect_err("expected mismatch");
        assert!(err.to_string().contains("div#main[0]"));
    }
}
