use super::DomSnapshotOptions;
use super::mismatch::DomMismatch;
use super::serialize::{canonical_attribute_order, format_node_line, node_label, truncate_line};
use crate::Node;
use std::sync::OnceLock;

pub(super) fn compare_nodes<'a>(
    expected: &'a Node,
    actual: &'a Node,
    options: &DomSnapshotOptions,
    path: &mut Vec<String>,
) -> Result<(), Box<DomMismatch<'a>>> {
    match (expected, actual) {
        (Node::Document { .. }, Node::Document { .. }) => {
            compare_document(expected, actual, options, path)
        }
        (Node::Element { .. }, Node::Element { .. }) => {
            compare_element(expected, actual, options, path)
        }
        (Node::Text { .. }, Node::Text { .. }) => compare_text(expected, actual, options, path),
        (Node::Comment { .. }, Node::Comment { .. }) => {
            compare_comment(expected, actual, options, path)
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

fn compare_document<'a>(
    expected_node: &'a Node,
    actual_node: &'a Node,
    options: &DomSnapshotOptions,
    path: &mut Vec<String>,
) -> Result<(), Box<DomMismatch<'a>>> {
    let (
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
    ) = (expected_node, actual_node)
    else {
        unreachable!("compare_document called with non-document nodes");
    };
    if !options.ignore_ids && *expected_id != *actual_id {
        return Err(Box::new(mismatch(
            path,
            "document id",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected_doctype != actual_doctype {
        return Err(Box::new(mismatch(
            path,
            "doctype",
            expected_node,
            actual_node,
            options,
        )));
    }
    compare_children(
        expected_node,
        actual_node,
        expected_children,
        actual_children,
        options,
        path,
    )
}

fn compare_element<'a>(
    expected_node: &'a Node,
    actual_node: &'a Node,
    options: &DomSnapshotOptions,
    path: &mut Vec<String>,
) -> Result<(), Box<DomMismatch<'a>>> {
    let (
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
    ) = (expected_node, actual_node)
    else {
        unreachable!("compare_element called with non-element nodes");
    };
    if !options.ignore_ids && *expected_id != *actual_id {
        return Err(Box::new(mismatch(
            path,
            "element id",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected_name != actual_name {
        return Err(Box::new(mismatch(
            path,
            "element name",
            expected_node,
            actual_node,
            options,
        )));
    }

    let expected_attr_order = canonical_attribute_order(expected_attrs);
    let actual_attr_order = canonical_attribute_order(actual_attrs);
    if expected_attr_order.len() != actual_attr_order.len() {
        return Err(Box::new(mismatch(
            path,
            "attribute count",
            expected_node,
            actual_node,
            options,
        )));
    }
    for (index, (expected_index, actual_index)) in expected_attr_order
        .iter()
        .zip(actual_attr_order.iter())
        .enumerate()
    {
        let expected_attr = &expected_attrs[*expected_index];
        let actual_attr = &actual_attrs[*actual_index];
        if expected_attr.0 != actual_attr.0 {
            return Err(Box::new(mismatch(
                path,
                &format!("canonical attribute mismatch at index {index} (name)"),
                expected_node,
                actual_node,
                options,
            )));
        }
        if expected_attr.1 != actual_attr.1 {
            return Err(Box::new(mismatch(
                path,
                &format!("canonical attribute mismatch at index {index} (value)"),
                expected_node,
                actual_node,
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
                expected_node,
                actual_node,
                options,
            )));
        }
        for (index, (expected_style_entry, actual_style_entry)) in
            expected_style.iter().zip(actual_style.iter()).enumerate()
        {
            if expected_style_entry != actual_style_entry {
                return Err(Box::new(mismatch(
                    path,
                    &format!("style entry at index {index}"),
                    expected_node,
                    actual_node,
                    options,
                )));
            }
        }
    }

    compare_children(
        expected_node,
        actual_node,
        expected_children,
        actual_children,
        options,
        path,
    )
}

fn compare_text<'a>(
    expected_node: &'a Node,
    actual_node: &'a Node,
    options: &DomSnapshotOptions,
    path: &[String],
) -> Result<(), Box<DomMismatch<'a>>> {
    let (
        Node::Text {
            id: expected_id,
            text: expected_text,
        },
        Node::Text {
            id: actual_id,
            text: actual_text,
        },
    ) = (expected_node, actual_node)
    else {
        unreachable!("compare_text called with non-text nodes");
    };
    if !options.ignore_ids && *expected_id != *actual_id {
        return Err(Box::new(mismatch(
            path,
            "text id",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected_text != actual_text {
        return Err(Box::new(mismatch(
            path,
            "text",
            expected_node,
            actual_node,
            options,
        )));
    }
    Ok(())
}

fn compare_comment<'a>(
    expected_node: &'a Node,
    actual_node: &'a Node,
    options: &DomSnapshotOptions,
    path: &[String],
) -> Result<(), Box<DomMismatch<'a>>> {
    let (
        Node::Comment {
            id: expected_id,
            text: expected_text,
        },
        Node::Comment {
            id: actual_id,
            text: actual_text,
        },
    ) = (expected_node, actual_node)
    else {
        unreachable!("compare_comment called with non-comment nodes");
    };
    if !options.ignore_ids && *expected_id != *actual_id {
        return Err(Box::new(mismatch(
            path,
            "comment id",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected_text != actual_text {
        return Err(Box::new(mismatch(
            path,
            "comment",
            expected_node,
            actual_node,
            options,
        )));
    }
    Ok(())
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
    for (index, (expected_child, actual_child)) in expected.iter().zip(actual.iter()).enumerate() {
        path.push(format!("{}[{}]", node_label(expected_child), index));
        let result = compare_nodes(expected_child, actual_child, options, path);
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
