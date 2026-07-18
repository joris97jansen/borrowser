use super::DomSnapshotOptions;
use crate::Node;
use crate::traverse::{FullModelNodeRef, full_model_preorder};
use std::fmt::Write;
use std::sync::Arc;

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
            let mut label = String::from(name.as_ref());
            // Pick id/class via canonical attribute order so duplicate attributes
            // produce deterministic labels in mismatch paths.
            let canonical_indices = canonical_attribute_order(attributes);
            let id_attr = canonical_indices
                .iter()
                .map(|index| &attributes[*index])
                .find(|(key, _)| key.as_ref() == "id")
                .and_then(|(_, value)| value.as_deref())
                .filter(|value| !value.is_empty());
            let class_attr = canonical_indices
                .iter()
                .map(|index| &attributes[*index])
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
            out.push('<');
            out.push_str(name);
            for index in canonical_attribute_order(attributes) {
                let (attribute, value) = &attributes[index];
                out.push(' ');
                out.push_str(attribute);
                if let Some(value) = value {
                    out.push('=');
                    out.push('"');
                    write_escaped(out, value);
                    out.push('"');
                }
            }
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
            out.push('>');
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

pub(super) fn canonical_attribute_order(attributes: &[(Arc<str>, Option<String>)]) -> Vec<usize> {
    let mut indexed: Vec<_> = attributes
        .iter()
        .enumerate()
        .map(|(index, _)| index)
        .collect();
    indexed.sort_by(|ia, ib| {
        let (name_a, value_a) = (&attributes[*ia].0, &attributes[*ia].1);
        let (name_b, value_b) = (&attributes[*ib].0, &attributes[*ib].1);
        let kind_a = if value_a.is_some() { 1u8 } else { 0u8 };
        let kind_b = if value_b.is_some() { 1u8 } else { 0u8 };
        (
            name_a.as_ref(),
            kind_a,
            value_a.as_deref().unwrap_or(""),
            *ia,
        )
            .cmp(&(
                name_b.as_ref(),
                kind_b,
                value_b.as_deref().unwrap_or(""),
                *ib,
            ))
    });
    indexed
}
