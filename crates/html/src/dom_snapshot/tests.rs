use super::{DomSnapshot, DomSnapshotOptions, assert_dom_eq, compare_dom};
use crate::Node;
use crate::dom_snapshot::serialize::truncate_line;
use crate::test_support::{html_attribute, html_name};
use crate::types::{DocumentFragmentNode, Id};

fn elem(name: &str, children: Vec<Node>) -> Node {
    crate::Node::from_element_parts(
        Id(0),
        html_name(name),
        vec![html_attribute("class", Some("a b"))],
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
            html_name("div"),
            vec![html_attribute("id", Some("main"))],
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
            html_name("div"),
            vec![html_attribute("id", Some("main"))],
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
fn snapshot_serialization_preserves_parser_attribute_order() {
    let doc = Node::Document {
        id: Id(0),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(0),
            html_name("div"),
            vec![
                html_attribute("zeta", Some("2")),
                html_attribute("alpha", Some("1")),
                html_attribute("hidden", None),
            ],
            Vec::new(),
            None,
            Vec::new(),
        )],
    };
    let lines = DomSnapshot::new(&doc, DomSnapshotOptions::default())
        .as_lines()
        .to_vec();
    assert_eq!(lines[0], "#dom-snapshot-v2");
    let element = &lines[2];
    let zeta = element.find("local=\"zeta\" value=\"2\"").unwrap();
    let alpha = element.find("local=\"alpha\" value=\"1\"").unwrap();
    let hidden = element.find("local=\"hidden\" value=\"\"").unwrap();
    assert!(zeta < alpha && alpha < hidden);
}

#[test]
fn dom_eq_observes_attribute_storage_order() {
    let expected = Node::Document {
        id: Id(0),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(0),
            html_name("div"),
            vec![
                html_attribute("a", Some("1")),
                html_attribute("b", Some("2")),
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
            html_name("div"),
            vec![
                html_attribute("b", Some("2")),
                html_attribute("a", Some("1")),
            ],
            Vec::new(),
            None,
            Vec::new(),
        )],
    };
    assert!(compare_dom(&expected, &actual, DomSnapshotOptions::default()).is_err());
}

#[test]
fn snapshot_element_ids_are_suffix_not_synthetic_attribute() {
    let doc = Node::Document {
        id: Id(7),
        doctype: None,
        children: vec![crate::Node::from_element_parts(
            Id(42),
            html_name("div"),
            vec![html_attribute("class", Some("x"))],
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
    assert_eq!(lines[0], "#dom-snapshot-v2");
    assert_eq!(lines[1], "#document id=7");
    assert_eq!(
        lines[2],
        "  element ns=html local=\"div\" attrs=[{ns=none prefix=- local=\"class\" value=\"x\"}] id=42"
    );
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
            html_name("template"),
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
            "#dom-snapshot-v2",
            "#document id=1",
            "  element ns=html local=\"template\" attrs=[] id=2",
            "    #template-contents id=3",
            "      \"inert\" id=4",
            "    <!-- legacy ordinary child --> id=5",
        ]
    );
}
