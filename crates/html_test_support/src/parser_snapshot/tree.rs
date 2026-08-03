use super::SnapshotData;
use super::lexical::{
    SnapshotReadError, SnapshotRecord, escape_quoted, fixed_fields, optional_quoted,
    strict_record_lines, validate_nullable_quoted, validate_quoted, validate_u64,
};
use html::conformance::{ObservationState, ObservedDomAttribute, ObservedTree, ObservedTreeNode};
use std::collections::BTreeSet;
use std::fmt::Write;

const HEADER: &str = "# format: html5-dom-v3";

define_snapshot_types!(ParsedTreeSnapshot, CanonicalTreeSnapshot);

pub(super) fn write(state: &ObservationState<ObservedTree>) -> Result<CanonicalTreeSnapshot, ()> {
    let ObservationState::Captured(tree) = state else {
        return Err(());
    };
    let mut bytes = format!("{HEADER}\n");
    let mut records = Vec::new();
    let mut work = Vec::new();
    for (index, node) in tree.roots.iter().enumerate().rev() {
        work.push(TreeWriteWork::Node {
            node,
            path: format!("/root[{index}]"),
        });
    }
    while let Some(item) = work.pop() {
        match item {
            TreeWriteWork::Node { node, path } => {
                write_node_record(node, &path, &mut bytes, &mut records);
                match node {
                    ObservedTreeNode::Document { children }
                    | ObservedTreeNode::Element { children, .. } => {
                        push_children(&mut work, children, &path);
                    }
                    ObservedTreeNode::HtmlTemplateElement {
                        ordinary_children,
                        contents,
                        ..
                    } => {
                        work.push(TreeWriteWork::TemplateContents {
                            host: path.clone(),
                            children: &contents.children,
                        });
                        push_children(&mut work, ordinary_children, &path);
                    }
                    ObservedTreeNode::DocumentType { .. }
                    | ObservedTreeNode::Comment { .. }
                    | ObservedTreeNode::Text { .. }
                    | ObservedTreeNode::ProcessingInstruction { .. } => {}
                }
            }
            TreeWriteWork::TemplateContents { host, children } => {
                let path = format!("{host}/contents");
                let line = format!("TEMPLATE_CONTENTS path={path} host={host}");
                let _ = writeln!(bytes, "{line}");
                records.push(SnapshotRecord {
                    location: path.clone(),
                    line,
                });
                push_children(&mut work, children, &path);
            }
        }
    }
    Ok(CanonicalTreeSnapshot::new(SnapshotData::new(
        bytes, records,
    )))
}

enum TreeWriteWork<'a> {
    Node {
        node: &'a ObservedTreeNode,
        path: String,
    },
    TemplateContents {
        host: String,
        children: &'a [ObservedTreeNode],
    },
}

fn push_children<'a>(
    work: &mut Vec<TreeWriteWork<'a>>,
    children: &'a [ObservedTreeNode],
    parent: &str,
) {
    for (index, child) in children.iter().enumerate().rev() {
        work.push(TreeWriteWork::Node {
            node: child,
            path: format!("{parent}/child[{index}]"),
        });
    }
}

fn write_node_record(
    node: &ObservedTreeNode,
    path: &str,
    bytes: &mut String,
    records: &mut Vec<SnapshotRecord>,
) {
    let line = match node {
        ObservedTreeNode::Document { .. } => format!("NODE path={path} kind=document"),
        ObservedTreeNode::DocumentType {
            name,
            public_id,
            system_id,
        } => format!(
            "NODE path={path} kind=document-type name={} public-id={} system-id={}",
            optional_quoted(name.as_deref()),
            optional_quoted(public_id.as_deref()),
            optional_quoted(system_id.as_deref())
        ),
        ObservedTreeNode::Comment { data } => {
            format!("NODE path={path} kind=comment data={}", escape_quoted(data))
        }
        ObservedTreeNode::Text { data } => {
            format!("NODE path={path} kind=text data={}", escape_quoted(data))
        }
        ObservedTreeNode::ProcessingInstruction { target, data } => format!(
            "NODE path={path} kind=processing-instruction target={} data={}",
            escape_quoted(target),
            escape_quoted(data)
        ),
        ObservedTreeNode::Element {
            namespace,
            local_name,
            ..
        } => format!(
            "NODE path={path} kind=element namespace={} local-name={}",
            namespace.snapshot_name(),
            escape_quoted(local_name)
        ),
        ObservedTreeNode::HtmlTemplateElement { .. } => {
            format!("NODE path={path} kind=html-template-host")
        }
    };
    let _ = writeln!(bytes, "{line}");
    records.push(SnapshotRecord {
        location: path.to_string(),
        line,
    });
    match node {
        ObservedTreeNode::Element { attributes, .. } => {
            write_attributes(path, attributes, bytes, records);
        }
        ObservedTreeNode::HtmlTemplateElement { attributes, .. } => {
            write_attributes(path, attributes, bytes, records);
        }
        ObservedTreeNode::Document { .. }
        | ObservedTreeNode::DocumentType { .. }
        | ObservedTreeNode::Comment { .. }
        | ObservedTreeNode::Text { .. }
        | ObservedTreeNode::ProcessingInstruction { .. } => {}
    }
}

fn write_attributes(
    path: &str,
    attributes: &[ObservedDomAttribute],
    bytes: &mut String,
    records: &mut Vec<SnapshotRecord>,
) {
    for (index, attribute) in attributes.iter().enumerate() {
        let line = format!(
            "ATTRIBUTE path={path} index={index} namespace={} prefix={} local-name={} value={}",
            attribute.namespace.snapshot_name(),
            optional_quoted(attribute.prefix.as_deref()),
            escape_quoted(&attribute.local_name),
            escape_quoted(&attribute.value)
        );
        let _ = writeln!(bytes, "{line}");
        records.push(SnapshotRecord {
            location: format!("{path} attribute {index}"),
            line,
        });
    }
}

pub(super) fn read(bytes: &[u8]) -> Result<ParsedTreeSnapshot, SnapshotReadError> {
    let lines = strict_record_lines(bytes, HEADER, true)?;
    let mut locations = BTreeSet::new();
    let mut expected_attribute = None::<(String, u64)>;
    let mut framing = TreeFraming::default();
    let mut records = Vec::new();
    for (line_number, line) in lines {
        if line.starts_with("ATTRIBUTE ") {
            let Some((node_path, index)) = expected_attribute.as_mut() else {
                return malformed(
                    line_number,
                    "attribute is not grouped below an element record",
                );
            };
            let Some(fields) = fixed_fields(
                line,
                "ATTRIBUTE",
                &[
                    "path",
                    "index",
                    "namespace",
                    "prefix",
                    "local-name",
                    "value",
                ],
            ) else {
                return malformed(line_number, "invalid attribute shape");
            };
            if fields[0] != node_path
                || !validate_u64(fields[1])
                || fields[1].parse::<u64>().ok() != Some(*index)
                || !matches!(fields[2], "none" | "xml" | "xmlns" | "xlink")
                || !validate_nullable_quoted(fields[3])
                || !validate_quoted(fields[4])
                || !validate_quoted(fields[5])
            {
                return malformed(line_number, "invalid attribute field or local index");
            }
            let location = format!("{} attribute {}", fields[0], fields[1]);
            if !locations.insert(location.clone()) {
                return Err(SnapshotReadError::DuplicateLocation { line: line_number });
            }
            records.push(SnapshotRecord {
                location,
                line: line.to_string(),
            });
            *index = index
                .checked_add(1)
                .ok_or(SnapshotReadError::NonContiguousOrdinal { line: line_number })?;
            continue;
        }
        expected_attribute = None;
        if line.starts_with("TEMPLATE_CONTENTS ") {
            let Some(fields) = fixed_fields(line, "TEMPLATE_CONTENTS", &["path", "host"]) else {
                return malformed(line_number, "invalid template-contents shape");
            };
            if fields[0] != format!("{}/contents", fields[1])
                || !valid_tree_path(fields[0], true)
                || !valid_tree_path(fields[1], false)
                || !framing.accept_template_contents(fields[1])
            {
                return malformed(
                    line_number,
                    "template contents must name its serialized HTML template host",
                );
            }
            if !locations.insert(fields[0].to_string()) {
                return Err(SnapshotReadError::DuplicateLocation { line: line_number });
            }
            records.push(SnapshotRecord {
                location: fields[0].to_string(),
                line: line.to_string(),
            });
            continue;
        }
        let mut prefix = line.splitn(4, ' ');
        let (Some("NODE"), Some(path_field), Some(kind_field)) =
            (prefix.next(), prefix.next(), prefix.next())
        else {
            return malformed(line_number, "invalid node prefix");
        };
        let Some(path) = path_field.strip_prefix("path=") else {
            return malformed(line_number, "node path is missing");
        };
        let Some(kind) = kind_field.strip_prefix("kind=") else {
            return malformed(line_number, "node kind is missing");
        };
        let valid = match kind {
            "document" | "html-template-host" => {
                fixed_fields(line, "NODE", &["path", "kind"]).is_some()
            }
            "document-type" => fixed_fields(
                line,
                "NODE",
                &["path", "kind", "name", "public-id", "system-id"],
            )
            .is_some_and(|f| {
                validate_nullable_quoted(f[2])
                    && validate_nullable_quoted(f[3])
                    && validate_nullable_quoted(f[4])
            }),
            "comment" | "text" => fixed_fields(line, "NODE", &["path", "kind", "data"])
                .is_some_and(|f| validate_quoted(f[2])),
            "processing-instruction" => {
                fixed_fields(line, "NODE", &["path", "kind", "target", "data"])
                    .is_some_and(|f| validate_quoted(f[2]) && validate_quoted(f[3]))
            }
            "element" => fixed_fields(line, "NODE", &["path", "kind", "namespace", "local-name"])
                .is_some_and(|f| {
                    matches!(f[2], "html" | "svg" | "mathml") && validate_quoted(f[3])
                }),
            _ => false,
        };
        if !valid {
            return malformed(line_number, "unknown node kind or malformed node fields");
        }
        let container = match kind {
            "document" | "element" => Some(ContainerKind::Ordinary { next_child: 0 }),
            "html-template-host" => Some(ContainerKind::Template {
                next_ordinary_child: 0,
                next_contents_child: None,
            }),
            "document-type" | "comment" | "text" | "processing-instruction" => None,
            _ => unreachable!("closed node kind validated above"),
        };
        if !valid_tree_path(path, false) || !framing.accept_node(path, container) {
            return malformed(line_number, "invalid or non-preorder tree path");
        }
        if !locations.insert(path.to_string()) {
            return Err(SnapshotReadError::DuplicateLocation { line: line_number });
        }
        records.push(SnapshotRecord {
            location: path.to_string(),
            line: line.to_string(),
        });
        if matches!(kind, "element" | "html-template-host") {
            expected_attribute = Some((path.to_string(), 0));
        }
    }
    if !framing.finish() {
        return malformed(1, "HTML template host is missing its contents boundary");
    }
    Ok(ParsedTreeSnapshot::new(SnapshotData::new(
        std::str::from_utf8(bytes)
            .map_err(|_| SnapshotReadError::InvalidUtf8)?
            .to_string(),
        records,
    )))
}

#[derive(Clone, Debug)]
enum ContainerKind {
    Ordinary {
        next_child: u64,
    },
    Template {
        next_ordinary_child: u64,
        next_contents_child: Option<u64>,
    },
}

#[derive(Clone, Debug)]
struct ContainerFrame {
    path: String,
    kind: ContainerKind,
}

#[derive(Default)]
struct TreeFraming {
    next_root: u64,
    frames: Vec<ContainerFrame>,
}

impl TreeFraming {
    fn accept_node(&mut self, path: &str, container: Option<ContainerKind>) -> bool {
        let accepted = if let Some((parent, index)) = child_location(path) {
            if let Some(host) = parent.strip_suffix("/contents") {
                self.close_to(host)
                    && self.frames.last_mut().is_some_and(|frame| {
                        let ContainerKind::Template {
                            next_contents_child: Some(expected),
                            ..
                        } = &mut frame.kind
                        else {
                            return false;
                        };
                        advance_index(expected, index)
                    })
            } else {
                self.close_to(parent)
                    && self
                        .frames
                        .last_mut()
                        .is_some_and(|frame| match &mut frame.kind {
                            ContainerKind::Ordinary { next_child } => {
                                advance_index(next_child, index)
                            }
                            ContainerKind::Template {
                                next_ordinary_child,
                                next_contents_child: None,
                            } => advance_index(next_ordinary_child, index),
                            ContainerKind::Template {
                                next_contents_child: Some(_),
                                ..
                            } => false,
                        })
            }
        } else if let Some(index) = root_location(path) {
            self.close_all() && advance_index(&mut self.next_root, index)
        } else {
            false
        };
        if accepted && let Some(kind) = container {
            self.frames.push(ContainerFrame {
                path: path.to_string(),
                kind,
            });
        }
        accepted
    }

    fn accept_template_contents(&mut self, host: &str) -> bool {
        self.close_to(host)
            && self.frames.last_mut().is_some_and(|frame| {
                let ContainerKind::Template {
                    next_contents_child,
                    ..
                } = &mut frame.kind
                else {
                    return false;
                };
                if next_contents_child.is_some() {
                    return false;
                }
                *next_contents_child = Some(0);
                true
            })
    }

    fn close_to(&mut self, path: &str) -> bool {
        while self.frames.last().is_some_and(|frame| frame.path != path) {
            if !self.pop_complete() {
                return false;
            }
        }
        self.frames.last().is_some_and(|frame| frame.path == path)
    }

    fn close_all(&mut self) -> bool {
        while !self.frames.is_empty() {
            if !self.pop_complete() {
                return false;
            }
        }
        true
    }

    fn pop_complete(&mut self) -> bool {
        self.frames.pop().is_some_and(|frame| match frame.kind {
            ContainerKind::Ordinary { .. } => true,
            ContainerKind::Template {
                next_contents_child,
                ..
            } => next_contents_child.is_some(),
        })
    }

    fn finish(&mut self) -> bool {
        self.close_all()
    }
}

fn advance_index(expected: &mut u64, actual: u64) -> bool {
    if *expected != actual {
        return false;
    }
    let Some(next) = expected.checked_add(1) else {
        return false;
    };
    *expected = next;
    true
}

fn child_location(path: &str) -> Option<(&str, u64)> {
    let (parent, tail) = path.rsplit_once("/child[")?;
    let index = tail.strip_suffix(']')?;
    validate_u64(index)
        .then(|| index.parse::<u64>().ok())
        .flatten()
        .map(|index| (parent, index))
}

fn root_location(path: &str) -> Option<u64> {
    let index = path.strip_prefix("/root[")?.strip_suffix(']')?;
    validate_u64(index)
        .then(|| index.parse::<u64>().ok())
        .flatten()
}

fn valid_tree_path(path: &str, allow_contents_terminal: bool) -> bool {
    let Some(mut rest) = path.strip_prefix("/root[") else {
        return false;
    };
    let Some(end) = rest.find(']') else {
        return false;
    };
    if !validate_u64(&rest[..end]) {
        return false;
    }
    rest = &rest[end + 1..];
    while !rest.is_empty() {
        if let Some(next) = rest.strip_prefix("/contents") {
            rest = next;
            continue;
        }
        let Some(next) = rest.strip_prefix("/child[") else {
            return false;
        };
        let Some(end) = next.find(']') else {
            return false;
        };
        if !validate_u64(&next[..end]) {
            return false;
        }
        rest = &next[end + 1..];
    }
    allow_contents_terminal || !path.ends_with("/contents")
}

fn malformed<T>(line: usize, reason: &'static str) -> Result<T, SnapshotReadError> {
    Err(SnapshotReadError::MalformedRecord { line, reason })
}
