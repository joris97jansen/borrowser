use super::{DomPatchError, DomStore};
use core_types::{DomHandle, DomVersion};
use html::PatchKey;
use html::{DomPatch, Node};

fn handle(id: u64) -> DomHandle {
    DomHandle(id)
}

fn stable_dom_lines(node: &Node) -> Vec<String> {
    fn escape(value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '"' => out.push_str("\\\""),
                '<' => out.push_str("\\u{3C}"),
                '>' => out.push_str("\\u{3E}"),
                c if c.is_ascii_control() => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
                c if c.is_ascii() => out.push(c),
                c => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
            }
        }
        out
    }

    fn push_node(out: &mut Vec<String>, node: &Node, depth: usize) {
        let indent = "  ".repeat(depth);
        match node {
            Node::Document {
                doctype, children, ..
            } => {
                out.push(match doctype {
                    Some(doctype) => {
                        format!("{indent}#document doctype=\"{}\"", escape(doctype))
                    }
                    None => format!("{indent}#document doctype=<none>"),
                });
                for child in children {
                    push_node(out, child, depth + 1);
                }
            }
            Node::Element {
                name,
                attributes,
                children,
                ..
            } => {
                let mut attrs = attributes
                    .iter()
                    .map(|(k, v)| (k.as_ref(), v.as_deref()))
                    .collect::<Vec<_>>();
                attrs.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
                let attrs = attrs
                    .into_iter()
                    .map(|(k, v)| match v {
                        Some(v) => format!("{k}=\"{}\"", escape(v)),
                        None => format!("{k}=<none>"),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(format!("{indent}<{name} attrs=[{attrs}]>"));
                for child in children {
                    push_node(out, child, depth + 1);
                }
            }
            Node::Text { text, .. } => {
                out.push(format!("{indent}text=\"{}\"", escape(text)));
            }
            Node::Comment { text, .. } => {
                out.push(format!("{indent}comment=\"{}\"", escape(text)));
            }
        }
    }

    let mut out = Vec::new();
    push_node(&mut out, node, 0);
    out
}

#[test]
fn create_duplicate_handle_errors() {
    let mut store = DomStore::new();
    let h = handle(1);
    store.create(h).expect("first create should succeed");
    let err = store.create(h).expect_err("duplicate create should error");
    assert!(matches!(err, DomPatchError::DuplicateHandle(v) if v == h));
}

#[test]
fn apply_is_atomic_on_mid_batch_error() {
    let mut store = DomStore::new();
    let h = handle(7);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            }],
        )
        .expect("bootstrap apply");

    let before = store
        .materialize(h)
        .expect("materialize before failed apply");
    let before = stable_dom_lines(&before);

    let err = store
        .apply(
            h,
            v1,
            v2,
            &[
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "div".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendText {
                    key: PatchKey(1),
                    text: "x".to_string(),
                },
            ],
        )
        .expect_err("invalid mid-batch operation should fail");
    assert!(matches!(err, DomPatchError::WrongNodeKind { .. }));

    let after = store
        .materialize(h)
        .expect("materialize after failed apply");
    let after = stable_dom_lines(&after);
    assert_eq!(before, after, "failed batch must not partially commit");

    store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::CreateComment {
                key: PatchKey(3),
                text: "ok".to_string(),
            }],
        )
        .expect("version should remain unchanged after failed batch");
}

#[test]
fn clear_only_batch_is_rejected() {
    let mut store = DomStore::new();
    let h = handle(9);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            }],
        )
        .expect("bootstrap apply");

    let err = store
        .apply(h, v1, v2, &[DomPatch::Clear])
        .expect_err("clear-only batch should be rejected");
    assert!(matches!(err, DomPatchError::Protocol(_)));
}

#[test]
fn empty_patch_batch_is_rejected() {
    let mut store = DomStore::new();
    let h = handle(11);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let err = store
        .apply(h, v0, v1, &[])
        .expect_err("empty patch batch should be rejected");
    assert!(matches!(err, DomPatchError::Protocol(_)));
}

#[test]
fn clear_batch_with_document_is_allowed() {
    let mut store = DomStore::new();
    let h = handle(12);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            }],
        )
        .expect("bootstrap apply");

    store
        .apply(
            h,
            v1,
            v2,
            &[
                DomPatch::Clear,
                DomPatch::CreateDocument {
                    key: PatchKey(10),
                    doctype: None,
                },
            ],
        )
        .expect("clear + CreateDocument should be accepted");

    let dom = store
        .materialize_owned(h)
        .expect("materialize after reset should succeed");
    let lines = stable_dom_lines(&dom);
    assert!(
        lines
            .first()
            .is_some_and(|line| line.starts_with("#document")),
        "reset batch should leave a rooted document"
    );
}

#[test]
fn clear_not_first_is_rejected() {
    let mut store = DomStore::new();
    let h = handle(13);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();

    let err = store
        .apply(
            h,
            v0,
            v1,
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::Clear,
            ],
        )
        .expect_err("Clear not first should be rejected");
    assert!(
        matches!(err, DomPatchError::Protocol(msg) if msg.contains("first patch")),
        "expected protocol error about Clear ordering, got: {err:?}"
    );
}

#[test]
fn duplicate_key_is_rejected_and_atomic() {
    let mut store = DomStore::new();
    let h = handle(14);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            }],
        )
        .expect("bootstrap apply");

    let before = store
        .materialize(h)
        .expect("materialize before failed apply");
    let before = stable_dom_lines(&before);

    let err = store
        .apply(
            h,
            v1,
            v2,
            &[
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "div".into(),
                    attributes: Vec::new(),
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "span".into(),
                    attributes: Vec::new(),
                },
            ],
        )
        .expect_err("duplicate key apply should fail");
    assert!(matches!(err, DomPatchError::DuplicateKey(PatchKey(2))));

    let after = store
        .materialize(h)
        .expect("materialize after failed apply");
    let after = stable_dom_lines(&after);
    assert_eq!(before, after, "failed batch must not partially commit");

    store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::CreateComment {
                key: PatchKey(3),
                text: "ok".to_string(),
            }],
        )
        .expect("version should remain unchanged after failed batch");
}

#[test]
fn invalid_key_is_rejected() {
    let mut store = DomStore::new();
    let h = handle(15);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();

    let err = store
        .apply(
            h,
            v0,
            v1,
            &[DomPatch::CreateDocument {
                key: PatchKey::INVALID,
                doctype: None,
            }],
        )
        .expect_err("invalid key should be rejected");
    assert!(matches!(err, DomPatchError::InvalidKey(PatchKey::INVALID)));
}

#[test]
fn missing_key_is_rejected_and_atomic() {
    let mut store = DomStore::new();
    let h = handle(16);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "div".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
            ],
        )
        .expect("bootstrap apply");

    let before = store
        .materialize(h)
        .expect("materialize before failed apply");
    let before = stable_dom_lines(&before);

    let err = store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::AppendChild {
                parent: PatchKey(999),
                child: PatchKey(2),
            }],
        )
        .expect_err("missing parent key should be rejected");
    assert!(matches!(err, DomPatchError::MissingKey(PatchKey(999))));

    let after = store
        .materialize(h)
        .expect("materialize after failed apply");
    let after = stable_dom_lines(&after);
    assert_eq!(before, after, "failed batch must not partially commit");

    store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::CreateComment {
                key: PatchKey(3),
                text: "ok".to_string(),
            }],
        )
        .expect("version should remain unchanged after failed batch");
}

#[test]
fn cycle_detection_rejects_back_edge_and_is_atomic() {
    let mut store = DomStore::new();
    let h = handle(17);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();
    let v3 = v2.next();

    store
        .apply(
            h,
            v0,
            v1,
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
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(3),
                },
            ],
        )
        .expect("bootstrap apply");

    let before = store
        .materialize(h)
        .expect("materialize before failed apply");
    let before = stable_dom_lines(&before);

    let err = store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(2),
            }],
        )
        .expect_err("back-edge append should be rejected");
    assert!(matches!(
        err,
        DomPatchError::CycleDetected {
            parent: PatchKey(3),
            child: PatchKey(2)
        }
    ));

    let after = store
        .materialize(h)
        .expect("materialize after failed apply");
    let after = stable_dom_lines(&after);
    assert_eq!(before, after, "cycle failure must not partially commit");

    let err = store
        .apply(
            h,
            v2,
            v3,
            &[DomPatch::CreateComment {
                key: PatchKey(999),
                text: "late".to_string(),
            }],
        )
        .expect_err("advanced from-version should mismatch");
    assert!(matches!(err, DomPatchError::VersionMismatch { .. }));

    store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::CreateComment {
                key: PatchKey(4),
                text: "ok".to_string(),
            }],
        )
        .expect("version should remain unchanged after failed batch");
}

#[test]
fn remove_root_without_clear_is_rejected_and_atomic() {
    let mut store = DomStore::new();
    let h = handle(18);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            }],
        )
        .expect("bootstrap apply");

    let before = store
        .materialize(h)
        .expect("materialize before failed apply");
    let before = stable_dom_lines(&before);

    let err = store
        .apply(h, v1, v2, &[DomPatch::RemoveNode { key: PatchKey(1) }])
        .expect_err("root removal without Clear should be rejected");
    assert!(matches!(
        err,
        DomPatchError::Protocol(msg) if msg.contains("rootless")
    ));

    let after = store
        .materialize(h)
        .expect("materialize after failed apply");
    let after = stable_dom_lines(&after);
    assert_eq!(before, after, "failed batch must not partially commit");

    store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::CreateComment {
                key: PatchKey(2),
                text: "ok".to_string(),
            }],
        )
        .expect("version should remain unchanged after failed batch");
}

#[test]
fn key_reuse_is_rejected_until_clear_then_allowed() {
    let mut store = DomStore::new();
    let h = handle(19);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();
    let v3 = v2.next();
    let v4 = v3.next();
    let v5 = v4.next();
    let v6 = v5.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "div".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
            ],
        )
        .expect("bootstrap apply");

    store
        .apply(h, v1, v2, &[DomPatch::RemoveNode { key: PatchKey(2) }])
        .expect("remove node");

    let err = store
        .apply(
            h,
            v2,
            v3,
            &[DomPatch::CreateElement {
                key: PatchKey(2),
                name: "span".into(),
                attributes: Vec::new(),
            }],
        )
        .expect_err("key reuse without Clear should be rejected");
    assert!(matches!(err, DomPatchError::DuplicateKey(PatchKey(2))));

    let err = store
        .apply(
            h,
            v3,
            v4,
            &[DomPatch::CreateComment {
                key: PatchKey(99),
                text: "nope".to_string(),
            }],
        )
        .expect_err("version must not have advanced after failed duplicate-key batch");
    assert!(matches!(err, DomPatchError::VersionMismatch { .. }));

    store
        .apply(
            h,
            v2,
            v3,
            &[DomPatch::CreateComment {
                key: PatchKey(99),
                text: "still v2".to_string(),
            }],
        )
        .expect("failed batch must not advance version; v2->v3 should still succeed");

    store
        .apply(
            h,
            v3,
            v4,
            &[
                DomPatch::Clear,
                DomPatch::CreateDocument {
                    key: PatchKey(10),
                    doctype: None,
                },
            ],
        )
        .expect("Clear should reset allocation domain");

    store
        .apply(
            h,
            v4,
            v5,
            &[DomPatch::CreateElement {
                key: PatchKey(2),
                name: "span".into(),
                attributes: Vec::new(),
            }],
        )
        .expect("key reuse should be allowed after Clear");

    store
        .apply(
            h,
            v5,
            v6,
            &[DomPatch::AppendChild {
                parent: PatchKey(10),
                child: PatchKey(2),
            }],
        )
        .expect("reused key should be attachable after Clear");
}

#[test]
fn reattaching_parented_node_returns_move_not_supported() {
    let mut store = DomStore::new();
    let h = handle(20);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
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
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(3),
                },
            ],
        )
        .expect("bootstrap apply");

    let err = store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(2),
            }],
        )
        .expect_err("reattaching a parented node should fail");
    assert!(matches!(
        err,
        DomPatchError::MoveNotSupported { key: PatchKey(2) }
    ));
}

#[test]
fn insert_before_with_parented_node_returns_move_not_supported() {
    let mut store = DomStore::new();
    let h = handle(21);
    store.create(h).expect("create handle");
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let v2 = v1.next();

    store
        .apply(
            h,
            v0,
            v1,
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::CreateElement {
                    key: PatchKey(2),
                    name: "child".into(),
                    attributes: Vec::new(),
                },
                DomPatch::CreateElement {
                    key: PatchKey(4),
                    name: "anchor".into(),
                    attributes: Vec::new(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(2),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(1),
                    child: PatchKey(4),
                },
            ],
        )
        .expect("bootstrap apply");

    let err = store
        .apply(
            h,
            v1,
            v2,
            &[DomPatch::InsertBefore {
                parent: PatchKey(1),
                child: PatchKey(2),
                before: PatchKey(4),
            }],
        )
        .expect_err("insert_before with already-parented child should fail");
    assert!(matches!(
        err,
        DomPatchError::MoveNotSupported { key: PatchKey(2) }
    ));
}
