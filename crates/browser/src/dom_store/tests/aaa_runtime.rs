use super::support::{VersionSteps, apply_ok, new_store_with_handle};
use html::{DomPatch, Node, PatchKey};

#[test]
fn runtime_applier_supports_aaa_furthest_block_reparenting_preserving_identity() {
    let (mut store, h) = new_store_with_handle(26);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: Some("html".to_string()),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "html".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "head".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "body".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(4),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(6),
                name: "p".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(6),
            },
            DomPatch::CreateText {
                key: PatchKey(7),
                text: "one".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(7),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(6),
            },
            DomPatch::CreateElement {
                key: PatchKey(8),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(8),
                child: PatchKey(7),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(8),
            },
        ],
        "runtime must materialize AAA furthest-block reparenting",
    );

    let dom = store
        .materialize_owned(h)
        .expect("materializing AAA furthest-block runtime DOM");
    let Node::Document { id, children, .. } = dom else {
        panic!("expected document root");
    };
    assert_eq!(id.0, 1);
    let Node::Element {
        id: html_id,
        children: html_children,
        ..
    } = &children[0]
    else {
        panic!("expected html element");
    };
    assert_eq!(html_id.0, 2);
    let Node::Element {
        id: body_id,
        children: body_children,
        ..
    } = &html_children[1]
    else {
        panic!("expected body element");
    };
    assert_eq!(body_id.0, 4);
    let Node::Element {
        id: outer_anchor_id,
        children: outer_anchor_children,
        ..
    } = &body_children[0]
    else {
        panic!("expected original anchor");
    };
    assert_eq!(outer_anchor_id.0, 5);
    assert!(
        outer_anchor_children.is_empty(),
        "original anchor should stay live but empty after the reparenting move"
    );
    let Node::Element {
        id: paragraph_id,
        children: paragraph_children,
        ..
    } = &body_children[1]
    else {
        panic!("expected moved paragraph");
    };
    assert_eq!(paragraph_id.0, 6);
    let Node::Element {
        id: recreated_anchor_id,
        children: recreated_anchor_children,
        ..
    } = &paragraph_children[0]
    else {
        panic!("expected recreated anchor");
    };
    assert_eq!(recreated_anchor_id.0, 8);
    let Node::Text { id: text_id, text } = &recreated_anchor_children[0] else {
        panic!("expected moved text child");
    };
    assert_eq!(text_id.0, 7);
    assert_eq!(text, "one");
}

#[test]
fn runtime_applier_supports_aaa_foster_parent_insert_before_preserving_identity() {
    let (mut store, h) = new_store_with_handle(27);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: Some("html".to_string()),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "html".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "head".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "body".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(4),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "table".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(6),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(6),
            },
            DomPatch::CreateElement {
                key: PatchKey(7),
                name: "tr".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(7),
            },
            DomPatch::CreateText {
                key: PatchKey(8),
                text: "x".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(7),
                child: PatchKey(8),
            },
            DomPatch::InsertBefore {
                parent: PatchKey(4),
                child: PatchKey(7),
                before: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(9),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(9),
                child: PatchKey(8),
            },
            DomPatch::AppendChild {
                parent: PatchKey(7),
                child: PatchKey(9),
            },
        ],
        "runtime must materialize AAA foster-parent InsertBefore reparenting",
    );

    let dom = store
        .materialize_owned(h)
        .expect("materializing AAA foster-parent runtime DOM");
    let Node::Document { id, children, .. } = dom else {
        panic!("expected document root");
    };
    assert_eq!(id.0, 1);
    let Node::Element {
        id: html_id,
        children: html_children,
        ..
    } = &children[0]
    else {
        panic!("expected html element");
    };
    assert_eq!(html_id.0, 2);
    let Node::Element {
        id: body_id,
        children: body_children,
        ..
    } = &html_children[1]
    else {
        panic!("expected body element");
    };
    assert_eq!(body_id.0, 4);
    let Node::Element {
        id: tr_id,
        children: tr_children,
        ..
    } = &body_children[0]
    else {
        panic!("expected foster-parented tr element");
    };
    assert_eq!(tr_id.0, 7);
    let Node::Element {
        id: recreated_anchor_id,
        children: recreated_anchor_children,
        ..
    } = &tr_children[0]
    else {
        panic!("expected recreated anchor under foster-parented tr");
    };
    assert_eq!(recreated_anchor_id.0, 9);
    let Node::Text { id: text_id, text } = &recreated_anchor_children[0] else {
        panic!("expected moved text node");
    };
    assert_eq!(text_id.0, 8);
    assert_eq!(text, "x");

    let Node::Element {
        id: table_id,
        children: table_children,
        ..
    } = &body_children[1]
    else {
        panic!("expected table sibling");
    };
    assert_eq!(table_id.0, 5);
    let Node::Element {
        id: old_anchor_id, ..
    } = &table_children[0]
    else {
        panic!("expected original anchor under table");
    };
    assert_eq!(old_anchor_id.0, 6);
}
