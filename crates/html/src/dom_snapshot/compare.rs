use super::DomSnapshotOptions;
use super::mismatch::DomMismatch;
use super::serialize::{format_node_line, node_label, truncate_line};
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
        (Node::DocumentType { .. }, Node::DocumentType { .. }) => {
            compare_document_type(expected, actual, options, path)
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

fn compare_document_type<'a>(
    expected_node: &'a Node,
    actual_node: &'a Node,
    options: &DomSnapshotOptions,
    path: &[String],
) -> Result<(), Box<DomMismatch<'a>>> {
    let (
        Node::DocumentType {
            id: expected_id,
            name: expected_name,
            public_id: expected_public_id,
            system_id: expected_system_id,
        },
        Node::DocumentType {
            id: actual_id,
            name: actual_name,
            public_id: actual_public_id,
            system_id: actual_system_id,
        },
    ) = (expected_node, actual_node)
    else {
        unreachable!("compare_document_type called with non-doctype nodes");
    };
    if !options.ignore_ids && *expected_id != *actual_id {
        return Err(Box::new(mismatch(
            path,
            "doctype id",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected_name != actual_name {
        return Err(Box::new(mismatch(
            path,
            "doctype name",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected_public_id != actual_public_id {
        return Err(Box::new(mismatch(
            path,
            "doctype public id",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected_system_id != actual_system_id {
        return Err(Box::new(mismatch(
            path,
            "doctype system id",
            expected_node,
            actual_node,
            options,
        )));
    }
    Ok(())
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
    let (Node::Element { element: expected }, Node::Element { element: actual }) =
        (expected_node, actual_node)
    else {
        unreachable!("compare_element called with non-element nodes");
    };
    if !options.ignore_ids && expected.id() != actual.id() {
        return Err(Box::new(mismatch(
            path,
            "element id",
            expected_node,
            actual_node,
            options,
        )));
    }
    if expected.expanded_name() != actual.expanded_name() {
        return Err(Box::new(mismatch(
            path,
            "element name",
            expected_node,
            actual_node,
            options,
        )));
    }

    if expected.attributes() != actual.attributes() {
        return Err(Box::new(mismatch(
            path,
            "ordered attribute list",
            expected_node,
            actual_node,
            options,
        )));
    }

    let expected_style = expected.style();
    let actual_style = actual.style();
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

    match (expected.template_contents(), actual.template_contents()) {
        (None, None) => {}
        (Some(expected), Some(actual)) => {
            if expected.kind() != actual.kind() {
                return Err(Box::new(mismatch(
                    path,
                    "template contents kind",
                    expected_node,
                    actual_node,
                    options,
                )));
            }
            if !options.ignore_ids && expected.id() != actual.id() {
                return Err(Box::new(mismatch(
                    path,
                    "template contents id",
                    expected_node,
                    actual_node,
                    options,
                )));
            }
            path.push("#template-contents".to_string());
            compare_children(
                expected_node,
                actual_node,
                expected.children(),
                actual.children(),
                options,
                path,
            )?;
            path.pop();
        }
        _ => {
            return Err(Box::new(mismatch(
                path,
                "template contents association",
                expected_node,
                actual_node,
                options,
            )));
        }
    }

    compare_children(
        expected_node,
        actual_node,
        expected.children(),
        actual.children(),
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
