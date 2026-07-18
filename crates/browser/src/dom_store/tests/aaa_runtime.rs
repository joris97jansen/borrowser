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
    let html = children[0].element().expect("expected html element");
    assert_eq!(html.id().0, 2);
    let body = html.children()[1].element().expect("expected body element");
    assert_eq!(body.id().0, 4);
    let outer_anchor = body.children()[0]
        .element()
        .expect("expected original anchor");
    assert_eq!(outer_anchor.id().0, 5);
    assert!(
        outer_anchor.children().is_empty(),
        "original anchor should stay live but empty after the reparenting move"
    );
    let paragraph = body.children()[1]
        .element()
        .expect("expected moved paragraph");
    assert_eq!(paragraph.id().0, 6);
    let recreated_anchor = paragraph.children()[0]
        .element()
        .expect("expected recreated anchor");
    assert_eq!(recreated_anchor.id().0, 8);
    let Node::Text { id: text_id, text } = &recreated_anchor.children()[0] else {
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
    let html = children[0].element().expect("expected html element");
    assert_eq!(html.id().0, 2);
    let body = html.children()[1].element().expect("expected body element");
    assert_eq!(body.id().0, 4);
    let tr = body.children()[0]
        .element()
        .expect("expected foster-parented tr element");
    assert_eq!(tr.id().0, 7);
    let recreated_anchor = tr.children()[0]
        .element()
        .expect("expected recreated anchor under foster-parented tr");
    assert_eq!(recreated_anchor.id().0, 9);
    let Node::Text { id: text_id, text } = &recreated_anchor.children()[0] else {
        panic!("expected moved text node");
    };
    assert_eq!(text_id.0, 8);
    assert_eq!(text, "x");

    let table = body.children()[1]
        .element()
        .expect("expected table sibling");
    assert_eq!(table.id().0, 5);
    let old_anchor = table.children()[0]
        .element()
        .expect("expected original anchor under table");
    assert_eq!(old_anchor.id().0, 6);
}
