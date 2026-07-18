use super::{DomSnapshot, DomSnapshotOptions, assert_dom_eq, compare_dom};
use crate::Node;
use crate::dom_snapshot::serialize::truncate_line;
use crate::types::{DocumentFragmentNode, Id};
use std::sync::Arc;

fn elem(name: &str, children: Vec<Node>) -> Node {
    crate::Node::from_element_parts(
        Id(0),
        Arc::from(name),
        vec![(Arc::from("class"), Some("a b".to_string()))],
        Vec::new(),
        None,
        children,
    )
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
        children: vec![crate::Node::from_element_parts(
            Id(0),
            Arc::from("div"),
            vec![(Arc::from("id"), Some("main".to_string()))],
            Vec::new(),
            None,
            vec![Node::Text {
                id: Id(0),
                text: "a".to_string(),
            }],
        )],
    };
    let actual = Node::Document {
        id: Id(0),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(0),
            Arc::from("div"),
            vec![(Arc::from("id"), Some("main".to_string()))],
            Vec::new(),
            None,
            vec![Node::Text {
                id: Id(0),
                text: "b".to_string(),
            }],
        )],
    };
    let err = compare_dom(&expected, &actual, DomSnapshotOptions::default())
        .expect_err("expected mismatch");
    assert!(err.to_string().contains("div#main[0]"));
}

#[test]
fn snapshot_serialization_sorts_attributes_lexically() {
    let doc = Node::Document {
        id: Id(0),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(0),
            Arc::from("div"),
            vec![
                (Arc::from("zeta"), Some("2".to_string())),
                (Arc::from("alpha"), Some("1".to_string())),
                (Arc::from("hidden"), None),
            ],
            Vec::new(),
            None,
            Vec::new(),
        )],
    };
    let lines = DomSnapshot::new(&doc, DomSnapshotOptions::default())
        .as_lines()
        .to_vec();
    assert_eq!(lines[1], "  <div alpha=\"1\" hidden zeta=\"2\">");
}

#[test]
fn dom_eq_ignores_attribute_storage_order() {
    let expected = Node::Document {
        id: Id(0),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(0),
            Arc::from("div"),
            vec![
                (Arc::from("a"), Some("1".to_string())),
                (Arc::from("b"), Some("2".to_string())),
            ],
            Vec::new(),
            None,
            Vec::new(),
        )],
    };
    let actual = Node::Document {
        id: Id(0),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(0),
            Arc::from("div"),
            vec![
                (Arc::from("b"), Some("2".to_string())),
                (Arc::from("a"), Some("1".to_string())),
            ],
            Vec::new(),
            None,
            Vec::new(),
        )],
    };
    assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
}

#[test]
fn snapshot_element_ids_are_suffix_not_synthetic_attribute() {
    let doc = Node::Document {
        id: Id(7),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(42),
            Arc::from("div"),
            vec![(Arc::from("class"), Some("x".to_string()))],
            Vec::new(),
            None,
            Vec::new(),
        )],
    };
    let lines = DomSnapshot::new(
        &doc,
        DomSnapshotOptions {
            ignore_ids: false,
            ignore_empty_style: true,
        },
    )
    .as_lines()
    .to_vec();
    assert_eq!(lines[0], "#document id=7");
    assert_eq!(lines[1], "  <div class=\"x\"> id=42");
}

#[test]
fn truncate_line_handles_multibyte_characters() {
    let line = "abc🙂def".to_string();
    assert_eq!(truncate_line(line, 6), "abc...");
}

#[test]
fn snapshot_exposes_template_association_before_ordinary_host_children() {
    let doc = Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(2),
            Arc::from("template"),
            Vec::new(),
            Vec::new(),
            Some(Box::new(DocumentFragmentNode::new_template_contents(
                Id(3),
                vec![Node::Text {
                    id: Id(4),
                    text: "inert".to_string(),
                }],
            ))),
            vec![Node::Comment {
                id: Id(5),
                text: "legacy ordinary child".to_string(),
            }],
        )],
    };
    let lines = DomSnapshot::new(
        &doc,
        DomSnapshotOptions {
            ignore_ids: false,
            ignore_empty_style: true,
        },
    )
    .as_lines()
    .to_vec();
    assert_eq!(
        lines,
        vec![
            "#document id=1",
            "  <template> id=2",
            "    #template-contents id=3",
            "      \"inert\" id=4",
            "    <!-- legacy ordinary child --> id=5",
        ]
    );
}
