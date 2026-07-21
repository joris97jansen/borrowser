use super::DomSnapshotOptions;
use crate::Node;
use crate::attributes::AttributeNamespace;
use crate::traverse::{FullModelNodeRef, full_model_preorder};
use std::fmt::Write;

pub(super) fn walk_snapshot(
    node: &Node,
    options: &DomSnapshotOptions,
    indent_level: &mut usize,
    out: &mut Vec<String>,
) {
    let base_indent = *indent_level;
    for visit in full_model_preorder(node) {
        let mut line = "  ".repeat(base_indent.saturating_add(visit.depth));
        match visit.entry {
            FullModelNodeRef::Node(node) => write_node_line(&mut line, node, options),
            FullModelNodeRef::DocumentFragment(fragment) => {
                line.push_str("#template-contents");
                if !options.ignore_ids {
                    let _ = write!(&mut line, " id={}", fragment.id().0);
                }
            }
        }
        out.push(line);
    }
}

pub(super) fn format_node_line(node: &Node, options: &DomSnapshotOptions) -> String {
    let mut line = String::new();
    write_node_line(&mut line, node, options);
    line
}

pub(super) fn node_label(node: &Node) -> String {
    match node {
        Node::Document { .. } => "#document".to_string(),
        Node::DocumentType { name, .. } => {
            let mut label = String::from("#doctype");
            if let Some(name) = name {
                label.push(':');
                write_escaped(&mut label, name);
            }
            label
        }
        Node::Element { element } => {
            let name = element.name();
            let attributes = element.attributes();
            let mut label = format!("{}:{}", element.namespace().snapshot_name(), name);
            let id_attr = attributes
                .iter()
                .find(|attribute| {
                    attribute.namespace() == AttributeNamespace::None
                        && attribute.local_name() == "id"
                })
                .map(|attribute| attribute.value())
                .filter(|value| !value.is_empty());
            let class_attr = attributes
                .iter()
                .find(|attribute| {
                    attribute.namespace() == AttributeNamespace::None
                        && attribute.local_name() == "class"
                })
                .map(|attribute| attribute.value())
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
        Node::ProcessingInstruction {
            processing_instruction,
        } => format!(
            "#processing-instruction:{}",
            processing_instruction.target()
        ),
    }
}

pub(super) fn truncate_line(mut line: String, max_len: usize) -> String {
    let char_len = line.chars().count();
    if char_len > max_len {
        if max_len == 0 {
            return String::new();
        }
        if max_len <= 3 {
            return ".".repeat(max_len);
        }
        let keep_chars = max_len - 3;
        let truncate_at = line
            .char_indices()
            .nth(keep_chars)
            .map(|(idx, _)| idx)
            .unwrap_or(line.len());
        line.truncate(truncate_at);
        line.push_str("...");
    }
    line
}

pub(super) fn write_node_line(out: &mut String, node: &Node, options: &DomSnapshotOptions) {
    match node {
        Node::Document { doctype, id, .. } => {
            out.push_str("#document");
            if let Some(doctype) = doctype {
                out.push_str(" doctype=\"");
                write_escaped(out, doctype);
                out.push('"');
            }
            if !options.ignore_ids {
                out.push_str(" id=");
                let _ = write!(out, "{}", id.0);
            }
        }
        Node::DocumentType {
            id,
            name,
            public_id,
            system_id,
        } => {
            out.push_str("<!doctype");
            if let Some(name) = name {
                out.push(' ');
                write_escaped(out, name);
            }
            if let Some(public_id) = public_id {
                out.push_str(" public-id=\"");
                write_escaped(out, public_id);
                out.push('"');
            }
            if let Some(system_id) = system_id {
                out.push_str(" system-id=\"");
                write_escaped(out, system_id);
                out.push('"');
            }
            out.push('>');
            if !options.ignore_ids {
                out.push_str(" id=");
                write!(out, "{}", id.0).ok();
            }
        }
        Node::Element { element } => {
            let id = element.id();
            let name = element.name();
            let attributes = element.attributes();
            let style = element.style();
            out.push_str("element ns=");
            out.push_str(element.namespace().snapshot_name());
            out.push_str(" local=\"");
            write_escaped(out, name);
            out.push_str("\" attrs=[");
            for (index, attribute) in attributes.iter().enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                out.push_str("{ns=");
                out.push_str(attribute.namespace().snapshot_name());
                out.push_str(" prefix=");
                if let Some(prefix) = attribute.prefix() {
                    out.push('"');
                    out.push_str(prefix);
                    out.push('"');
                } else {
                    out.push('-');
                }
                out.push_str(" local=\"");
                write_escaped(out, attribute.local_name());
                out.push_str("\" value=\"");
                write_escaped(out, attribute.value());
                out.push_str("\"}");
            }
            out.push(']');
            let include_style = !(options.ignore_empty_style && style.is_empty());
            if include_style {
                out.push_str(" style=[");
                for (index, (key, value)) in style.iter().enumerate() {
                    if index != 0 {
                        out.push_str("; ");
                    }
                    out.push_str(key);
                    out.push_str(": ");
                    write_escaped(out, value);
                }
                out.push(']');
            }
            if !options.ignore_ids {
                out.push_str(" id=");
                write!(out, "{}", id.0).ok();
            }
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
        Node::ProcessingInstruction {
            processing_instruction,
        } => {
            out.push_str("processing-instruction target=\"");
            write_escaped(out, processing_instruction.target());
            out.push_str("\" data=\"");
            write_escaped(out, processing_instruction.data());
            out.push('"');
            if !options.ignore_ids {
                out.push_str(" id=");
                write!(out, "{}", processing_instruction.id().0).ok();
            }
        }
    }
}

pub(super) fn write_escaped(out: &mut String, value: &str) {
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
