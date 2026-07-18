use super::super::DomPatchError;
use super::support::{
    VersionSteps, apply_ok, assert_failed_apply_is_atomic, materialized_dom_lines,
    new_store_with_handle,
};
use html::internal::ParserCreatedFragmentKind;
use html::{DomPatch, PatchKey};

fn template_batch() -> Vec<DomPatch> {
    vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
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
            name: "template".into(),
            attributes: Vec::new(),
        },
        DomPatch::CreateTemplateContents {
            host: PatchKey(3),
            contents: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::CreateText {
            key: PatchKey(5),
            text: "inert".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(5),
        },
    ]
}

fn nested_template_batch() -> Vec<DomPatch> {
    let mut patches = template_batch();
    patches.extend([
        DomPatch::CreateElement {
            key: PatchKey(6),
            name: "template".into(),
            attributes: Vec::new(),
        },
        DomPatch::CreateTemplateContents {
            host: PatchKey(6),
            contents: PatchKey(7),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(6),
        },
        DomPatch::CreateText {
            key: PatchKey(8),
            text: "nested inert".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(7),
            child: PatchKey(8),
        },
    ]);
    patches
}

#[test]
fn runtime_arena_materializes_typed_template_contents_in_full_model_order() {
    let (mut store, handle) = new_store_with_handle(40);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &template_batch(),
        "template batch should apply",
    );
    assert_eq!(
        materialized_dom_lines(&store, handle),
        vec![
            "#document doctype=<none>",
            "  <html attrs=[]>",
            "    <template attrs=[]>",
            "      #template-contents id=4",
            "        text=\"inert\"",
        ]
    );
}

#[test]
fn runtime_template_association_rejections_are_atomic() {
    let (mut store, handle) = new_store_with_handle(41);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &template_batch(),
        "template batch should apply",
    );

    let (from, to) = versions.next_pair();
    let duplicate = assert_failed_apply_is_atomic(
        &mut store,
        handle,
        from,
        to,
        &[DomPatch::CreateTemplateContents {
            host: PatchKey(3),
            contents: PatchKey(6),
        }],
    );
    assert!(matches!(duplicate, DomPatchError::Protocol(_)));

    let (from, to) = versions.next_pair();
    let parent = assert_failed_apply_is_atomic(
        &mut store,
        handle,
        from,
        to,
        &[DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        }],
    );
    assert!(matches!(parent, DomPatchError::IllegalMove { .. }));

    let (from, to) = versions.next_pair();
    let removal = assert_failed_apply_is_atomic(
        &mut store,
        handle,
        from,
        to,
        &[DomPatch::RemoveNode { key: PatchKey(4) }],
    );
    assert!(matches!(removal, DomPatchError::IllegalMove { .. }));
}

#[test]
fn removing_template_host_or_ancestor_removes_associated_fragment_subgraph() {
    for (handle_id, removal) in [(42, PatchKey(3)), (43, PatchKey(2))] {
        let (mut store, handle) = new_store_with_handle(handle_id);
        let mut versions = VersionSteps::new();
        apply_ok(
            &mut store,
            handle,
            &mut versions,
            &nested_template_batch(),
            "template batch should apply",
        );
        apply_ok(
            &mut store,
            handle,
            &mut versions,
            &[DomPatch::RemoveNode { key: removal }],
            "owner removal should cascade through template contents",
        );
        for removed in (3..=8).map(PatchKey) {
            let err = store
                .resolve_live_node_ids(handle, &[removed])
                .expect_err("associated subtree key must no longer be live");
            assert!(matches!(err, DomPatchError::MissingKey(key) if key == removed));
        }
    }
}

#[test]
fn clear_removes_runtime_template_associations_and_nested_fragment_subgraphs() {
    let (mut store, handle) = new_store_with_handle(44);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &nested_template_batch(),
        "template batch should apply",
    );
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &[
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(10),
                doctype: None,
            },
        ],
        "runtime Clear must replace the complete association graph",
    );

    for removed in (3..=8).map(PatchKey) {
        let err = store
            .resolve_live_node_ids(handle, &[removed])
            .expect_err("cleared association key must not remain live");
        assert!(matches!(err, DomPatchError::MissingKey(key) if key == removed));
    }
    assert_eq!(
        materialized_dom_lines(&store, handle),
        vec!["#document doctype=<none>"]
    );
}

#[test]
fn runtime_materialization_rejects_fragment_kind_mismatch() {
    let (mut store, handle) = new_store_with_handle(45);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &template_batch(),
        "template batch should apply",
    );
    store.set_fragment_kind_for_test(
        handle,
        PatchKey(4),
        ParserCreatedFragmentKind::TestOnlyUnsupported,
    );

    let error = store
        .materialize(handle)
        .expect_err("runtime materialization must validate the association fragment kind");
    assert!(
        matches!(error, DomPatchError::Protocol(message) if message.contains("wrong fragment kind"))
    );
}
