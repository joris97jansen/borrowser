use super::support::{VersionSteps, apply_ok, materialized_dom_lines, new_store_with_handle};
use html::{DomPatch, PatchKey};

#[test]
fn reattaching_parented_node_reparents_preserving_identity() {
    let (mut store, h) = new_store_with_handle(20);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "b".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "root".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(4),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(3),
            },
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(2),
            },
        ],
        "bootstrap apply",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(2),
        }],
        "reattaching a parented node should perform a move",
    );
    assert_eq!(
        materialized_dom_lines(&store, h),
        vec![
            "#document doctype=<none>".to_string(),
            "  <root attrs=[]>".to_string(),
            "    <b attrs=[]>".to_string(),
            "    <a attrs=[]>".to_string(),
        ]
    );
}

#[test]
fn insert_before_with_parented_node_reorders_and_noops() {
    let (mut store, h) = new_store_with_handle(21);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "root".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "b".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "c".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(5),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(2),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(3),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(4),
            },
        ],
        "bootstrap apply",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::InsertBefore {
            parent: PatchKey(5),
            child: PatchKey(4),
            before: PatchKey(2),
        }],
        "insert_before with already-parented child should reorder",
    );
    assert_eq!(
        materialized_dom_lines(&store, h),
        vec![
            "#document doctype=<none>".to_string(),
            "  <root attrs=[]>".to_string(),
            "    <c attrs=[]>".to_string(),
            "    <a attrs=[]>".to_string(),
            "    <b attrs=[]>".to_string(),
        ]
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::InsertBefore {
            parent: PatchKey(5),
            child: PatchKey(2),
            before: PatchKey(3),
        }],
        "insert_before in the existing position should be a no-op",
    );
    assert_eq!(
        materialized_dom_lines(&store, h),
        vec![
            "#document doctype=<none>".to_string(),
            "  <root attrs=[]>".to_string(),
            "    <c attrs=[]>".to_string(),
            "    <a attrs=[]>".to_string(),
            "    <b attrs=[]>".to_string(),
        ]
    );
}

#[test]
fn append_child_with_same_parent_moves_node_to_end() {
    let (mut store, h) = new_store_with_handle(23);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "root".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "b".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "c".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(5),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(2),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(3),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(4),
            },
        ],
        "bootstrap apply",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(2),
        }],
        "append_child on same parent should move the node to the end",
    );
    assert_eq!(
        materialized_dom_lines(&store, h),
        vec![
            "#document doctype=<none>".to_string(),
            "  <root attrs=[]>".to_string(),
            "    <b attrs=[]>".to_string(),
            "    <c attrs=[]>".to_string(),
            "    <a attrs=[]>".to_string(),
        ]
    );
}

#[test]
fn insert_before_supports_cross_parent_reparenting() {
    let (mut store, h) = new_store_with_handle(24);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "left".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "right".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "child".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "anchor".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(3),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(4),
            },
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(5),
            },
        ],
        "bootstrap apply",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(4),
            before: PatchKey(5),
        }],
        "insert_before should support cross-parent reparenting",
    );
    assert_eq!(
        materialized_dom_lines(&store, h),
        vec![
            "#document doctype=<none>".to_string(),
            "  <left attrs=[]>".to_string(),
            "  <right attrs=[]>".to_string(),
            "    <child attrs=[]>".to_string(),
            "    <anchor attrs=[]>".to_string(),
        ]
    );
}
