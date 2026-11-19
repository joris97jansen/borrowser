use super::{PatchValidationArena, PatchValidationError};
use crate::dom_patch::PatchKey;
use crate::test_support::{html_attribute, html_name};
use crate::{DomPatch, Node};

#[test]
fn patch_validation_arena_accepts_valid_batches_and_materializes() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: Some("html".to_string()),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: html_name("html"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::CreateText {
                key: PatchKey(3),
                text: "ok".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
        ])
        .expect("valid batch should apply");

    let dom = arena.materialize().expect("valid arena should materialize");
    match dom {
        crate::Node::Document { children, .. } => assert_eq!(children.len(), 1),
        other => panic!("expected document root, got {other:?}"),
    }
}

#[cfg(feature = "parser-conformance")]
#[test]
fn semantic_compare_wide_tree_uses_ancestor_depth_not_sibling_count() {
    const WIDTH: u32 = 2_000;
    let mut patches = Vec::new();
    patches.push(DomPatch::CreateDocument {
        key: PatchKey(1),
        doctype: None,
    });
    patches.push(DomPatch::CreateElement {
        key: PatchKey(2),
        name: html_name("html"),
        attributes: Vec::new(),
    });
    patches.push(DomPatch::AppendChild {
        parent: PatchKey(1),
        child: PatchKey(2),
    });
    for offset in 0..WIDTH {
        let key = PatchKey(3 + offset);
        patches.push(DomPatch::CreateElement {
            key,
            name: html_name("div"),
            attributes: Vec::new(),
        });
        patches.push(DomPatch::AppendChild {
            parent: PatchKey(2),
            child: key,
        });
    }
    let mut arena = PatchValidationArena::default();
    arena.apply_batch(&patches).expect("wide tree batch");
    let document = arena.materialize().expect("wide tree materialization");
    let (matches, maximum_depth) = arena
        .semantic_compare_depth_for_test(&document)
        .expect("wide semantic traversal");
    assert!(matches);
    assert_eq!(maximum_depth, 3);
}

#[cfg(feature = "parser-conformance")]
#[test]
fn semantic_compare_deep_tree_is_iterative_and_tracks_active_ancestors() {
    const DEPTH: u32 = 300;
    let mut patches = Vec::new();
    patches.push(DomPatch::CreateDocument {
        key: PatchKey(1),
        doctype: None,
    });
    let mut parent = PatchKey(1);
    for offset in 0..DEPTH {
        let key = PatchKey(2 + offset);
        patches.push(DomPatch::CreateElement {
            key,
            name: html_name("div"),
            attributes: Vec::new(),
        });
        patches.push(DomPatch::AppendChild { parent, child: key });
        parent = key;
    }
    let mut arena = PatchValidationArena::default();
    arena.apply_batch(&patches).expect("deep tree batch");
    let document = arena.materialize().expect("deep tree materialization");
    let (matches, maximum_depth) = arena
        .semantic_compare_depth_for_test(&document)
        .expect("deep semantic traversal");
    assert!(matches);
    assert_eq!(maximum_depth, DEPTH as usize + 1);
}

#[cfg(feature = "parser-conformance")]
#[test]
fn semantic_compare_rejects_complete_payload_and_child_order_mismatches() {
    let patches = vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: Some("legacy-html".to_string()),
        },
        DomPatch::CreateDocumentType {
            key: PatchKey(2),
            name: Some("html".to_string()),
            public_id: Some("public".to_string()),
            system_id: Some("system".to_string()),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: html_name("html"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(3),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: html_name("template"),
            attributes: vec![
                html_attribute("a", Some("1")),
                html_attribute("b", Some("2")),
            ],
        },
        DomPatch::CreateTemplateContents {
            host: PatchKey(4),
            contents: PatchKey(5),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
        DomPatch::CreateText {
            key: PatchKey(6),
            text: "text".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(6),
        },
        DomPatch::CreateComment {
            key: PatchKey(7),
            text: "inside".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(7),
        },
        DomPatch::CreateComment {
            key: PatchKey(8),
            text: "outside".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(8),
        },
        DomPatch::CreateProcessingInstruction {
            key: PatchKey(9),
            target: "target".to_string(),
            data: "data".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(9),
        },
    ];
    let mut arena = PatchValidationArena::default();
    arena.apply_batch(&patches).expect("semantic seed batch");
    let document = arena.materialize().expect("semantic seed materializes");
    let mut reserve = |_| Ok(());
    assert!(
        arena
            .semantic_equals_materialized_dom_for_final_audit(&document, &mut reserve)
            .expect("semantic seed comparison")
    );

    let assert_mismatch = |mutate: &dyn Fn(&mut Node)| {
        let mut changed = arena.materialize().expect("fresh semantic materialization");
        mutate(&mut changed);
        let mut reserve = |_| Ok(());
        assert!(
            !arena
                .semantic_equals_materialized_dom_for_final_audit(&changed, &mut reserve)
                .expect("mismatch comparison")
        );
    };

    assert_mismatch(&|document| {
        let Node::Document { doctype, .. } = document else {
            panic!("document root");
        };
        *doctype = Some("changed".to_string());
    });
    assert_mismatch(&|document| {
        let Node::Document { children, .. } = document else {
            panic!("document root");
        };
        let Node::DocumentType {
            name,
            public_id,
            system_id,
            ..
        } = &mut children[0]
        else {
            panic!("doctype child");
        };
        *name = Some("changed".to_string());
        *public_id = Some("changed".to_string());
        *system_id = Some("changed".to_string());
    });
    assert_mismatch(&|document| {
        let Node::Document { children, .. } = document else {
            panic!("document root");
        };
        let replacement = {
            let Node::Element { element } = &mut children[1] else {
                panic!("html child");
            };
            Node::new_element(
                html_name("body"),
                element.attributes().to_vec(),
                Vec::new(),
                std::mem::take(element.children_mut()),
            )
        };
        children[1] = replacement;
    });
    assert_mismatch(&|document| {
        let template = template_element_mut(document);
        template.attributes_mut().swap(0, 1);
    });
    assert_mismatch(&|document| {
        let template = template_element_mut(document);
        template.attributes_mut()[0] = html_attribute("a", Some("changed"));
    });
    assert_mismatch(&|document| {
        let contents = template_element_mut(document)
            .template_contents_mut()
            .expect("template contents");
        let Node::Text { text, .. } = &mut contents.children_mut()[0] else {
            panic!("text child");
        };
        text.push_str("-changed");
    });
    assert_mismatch(&|document| {
        let contents = template_element_mut(document)
            .template_contents_mut()
            .expect("template contents");
        let Node::Comment { text, .. } = &mut contents.children_mut()[1] else {
            panic!("comment child");
        };
        text.push_str("-changed");
    });
    assert_mismatch(&|document| {
        let Node::Document { children, .. } = document else {
            panic!("document root");
        };
        let Node::Element { element } = &mut children[1] else {
            panic!("html child");
        };
        element.children_mut().swap(1, 2);
    });
    assert_mismatch(&|document| {
        let Node::Document { children, .. } = document else {
            panic!("document root");
        };
        let Node::Element { element } = &mut children[1] else {
            panic!("html child");
        };
        let Node::ProcessingInstruction {
            processing_instruction,
        } = &mut element.children_mut()[2]
        else {
            panic!("processing instruction child");
        };
        *processing_instruction =
            crate::types::ProcessingInstructionNode::try_from_parser_created_parts(
                crate::types::Id::INVALID,
                "changed".to_string(),
                "changed".to_string(),
            )
            .expect("valid changed processing instruction");
    });
    assert_mismatch(&|document| {
        let contents = template_element_mut(document)
            .template_contents_mut()
            .expect("template contents");
        contents.children_mut().swap(0, 1);
    });
    assert_mismatch(&|document| {
        let Node::Document { children, .. } = document else {
            panic!("document root");
        };
        let Node::Element { element } = &mut children[1] else {
            panic!("html child");
        };
        let attributes = match &element.children()[0] {
            Node::Element { element } => element.attributes().to_vec(),
            _ => panic!("template child"),
        };
        element.children_mut()[0] =
            Node::new_element(html_name("template"), attributes, Vec::new(), Vec::new());
    });
}

#[cfg(feature = "parser-conformance")]
fn template_element_mut(document: &mut Node) -> &mut crate::ElementNode {
    let Node::Document { children, .. } = document else {
        panic!("document root");
    };
    let Node::Element { element: html } = &mut children[1] else {
        panic!("html child");
    };
    let Node::Element { element: template } = &mut html.children_mut()[0] else {
        panic!("template child");
    };
    template
}

#[test]
fn patch_validation_arena_reports_clear_ordering_actionably() {
    let mut arena = PatchValidationArena::default();
    let err = arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::Clear,
        ])
        .expect_err("Clear after the first patch must fail");

    assert!(
        err.to_string()
            .contains("batch order: Clear may only appear as the first patch in a batch"),
        "unexpected clear-ordering error: {err}"
    );
}

#[test]
fn patch_validation_arena_reports_missing_child_actionably() {
    let mut arena = PatchValidationArena::default();
    let err = arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(9),
            },
        ])
        .expect_err("missing child reference must fail");

    assert!(
        err.to_string()
            .contains("AppendChild child: missing node PatchKey(9)"),
        "unexpected append-child error: {err}"
    );
}

#[test]
fn patch_validation_arena_rejects_detached_non_root_nodes() {
    let mut arena = PatchValidationArena::default();
    let err = arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: html_name("html"),
                attributes: Vec::new(),
            },
        ])
        .expect_err("detached non-root nodes must fail validation");

    assert!(
        err.to_string()
            .contains("post-apply invariants: detached non-root node PatchKey(2)"),
        "unexpected detached-node error: {err}"
    );
}

#[test]
fn patch_validation_arena_preserves_key_freshness_across_clear() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: html_name("html"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ])
        .expect("seed batch should apply");

    let err = arena
        .apply_batch(&[
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
        ])
        .expect_err("Clear must not allow patch-key reuse");

    assert!(
        err.to_string()
            .contains("create: duplicate patch key PatchKey(1)"),
        "unexpected duplicate-key error after Clear: {err}"
    );
}

pub(super) fn template_seed_batch() -> Vec<DomPatch> {
    vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: html_name("html"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: html_name("template"),
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

fn nested_template_seed_batch() -> Vec<DomPatch> {
    let mut patches = template_seed_batch();
    patches.extend([
        DomPatch::CreateElement {
            key: PatchKey(6),
            name: html_name("template"),
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
fn typed_template_contents_materialize_through_the_host_association() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&template_seed_batch())
        .expect("typed template patch batch should apply");

    let dom = arena
        .materialize()
        .expect("template DOM should materialize");
    let crate::Node::Document { children, .. } = dom else {
        panic!("expected document");
    };
    let crate::Node::Element { element: html } = &children[0] else {
        panic!("expected html element");
    };
    let crate::Node::Element { element: template } = &html.children()[0] else {
        panic!("expected associated template contents");
    };
    assert!(
        template.children().is_empty(),
        "template host must have no ordinary children"
    );
    let contents = template
        .template_contents()
        .expect("expected associated template contents");
    assert_eq!(
        contents.kind(),
        crate::types::ParserCreatedFragmentKind::TemplateContents
    );
    assert_eq!(contents.children().len(), 1);
}

#[test]
fn template_association_rejection_rolls_back_the_whole_batch() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&template_seed_batch())
        .expect("seed batch should apply");
    let before = format!(
        "{:?}",
        arena.materialize().expect("seed should materialize")
    );

    let err = arena
        .apply_batch(&[
            DomPatch::CreateText {
                key: PatchKey(6),
                text: "must roll back".to_string(),
            },
            DomPatch::CreateTemplateContents {
                host: PatchKey(3),
                contents: PatchKey(7),
            },
        ])
        .expect_err("duplicate association must fail");
    assert!(err.to_string().contains("already has contents"));
    assert_eq!(
        format!(
            "{:?}",
            arena.materialize().expect("arena should remain valid")
        ),
        before
    );

    arena
        .apply_batch(&[
            DomPatch::CreateText {
                key: PatchKey(6),
                text: "key was not consumed".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(6),
            },
        ])
        .expect("rollback must preserve key freshness state");
}

#[test]
fn hosted_contents_cannot_be_parented_or_removed_directly() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&template_seed_batch())
        .expect("seed batch should apply");

    let append_err = arena
        .apply_batch(&[DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(4),
        }])
        .expect_err("fragment must not acquire an ordinary parent");
    assert!(append_err.to_string().contains("ordinary parents"));

    let remove_err = arena
        .apply_batch(&[DomPatch::RemoveNode { key: PatchKey(4) }])
        .expect_err("hosted fragment must not be removed directly");
    assert!(
        remove_err.to_string().contains("removed directly"),
        "unexpected direct-removal error: {remove_err}"
    );
    arena.materialize().expect("failed batches must roll back");
}

#[test]
fn host_and_ancestor_removal_cascade_through_template_contents() {
    for removal_key in [PatchKey(3), PatchKey(2)] {
        let mut arena = PatchValidationArena::default();
        arena
            .apply_batch(&nested_template_seed_batch())
            .expect("seed batch should apply");
        arena
            .apply_batch(&[DomPatch::RemoveNode { key: removal_key }])
            .expect("owner removal must remove its associated fragment graph");
        for key in (3..=8).map(PatchKey) {
            assert!(!arena.nodes.contains_key(&key), "stale nested node {key:?}");
        }
    }
}

#[test]
fn generic_validation_allows_unassociated_legacy_template_elements() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: html_name("template"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ])
        .expect("generic validation must not infer parser provenance from the name");
}

#[test]
fn clear_removes_template_associations_and_nested_fragment_subgraphs() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&nested_template_seed_batch())
        .expect("template seed should apply");
    arena
        .apply_batch(&[DomPatch::Clear])
        .expect("generic validator permits a cleared, rootless arena");

    assert!(arena.nodes.is_empty());
    assert!(arena.root.is_none());
    assert!(
        (3..=8).all(|key| arena.allocated.contains(&PatchKey(key))),
        "Clear removes live association state without weakening session key freshness"
    );
}

#[test]
fn materialization_rejects_an_associated_fragment_kind_mismatch() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&template_seed_batch())
        .expect("template seed should apply");
    let fragment = arena.nodes.get_mut(&PatchKey(4)).unwrap();
    fragment.kind = super::model::PatchKind::DocumentFragment {
        kind: crate::types::ParserCreatedFragmentKind::TestOnlyUnsupported,
        host: PatchKey(3),
    };

    let error = arena
        .materialize()
        .expect_err("materialization must independently validate fragment kind");
    assert!(error.to_string().contains("not template contents"));
}

#[test]
fn processing_instruction_patch_is_typed_materialized_and_a_leaf() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateProcessingInstruction {
                key: PatchKey(2),
                target: "Exact-Target".to_string(),
                data: "alpha ? beta".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ])
        .expect("valid parser-created PI batch");

    let dom = arena.materialize().expect("PI arena materializes");
    let Node::Document { children, .. } = dom else {
        panic!("expected document")
    };
    let Node::ProcessingInstruction {
        processing_instruction,
    } = &children[0]
    else {
        panic!("expected typed PI child")
    };
    assert_eq!(processing_instruction.target(), "Exact-Target");
    assert_eq!(processing_instruction.data(), "alpha ? beta");
    assert!(children[0].children().is_none(), "PI must be a leaf");

    let before = crate::dom_snapshot::DomSnapshot::new(
        &Node::Document {
            id: crate::types::Id::INVALID,
            doctype: None,
            children,
        },
        crate::dom_snapshot::DomSnapshotOptions::default(),
    )
    .render();
    let err = arena
        .apply_batch(&[
            DomPatch::CreateText {
                key: PatchKey(3),
                text: "child".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
        ])
        .expect_err("PI cannot accept children");
    assert!(err.to_string().contains("target must be a container node"));
    let after = crate::dom_snapshot::DomSnapshot::new(
        &arena.materialize().expect("failed leaf batch is atomic"),
        crate::dom_snapshot::DomSnapshotOptions::default(),
    )
    .render();
    assert_eq!(before, after, "failed PI child batch must be atomic");
}

#[test]
fn processing_instruction_patch_rejects_non_parser_producible_payloads_atomically() {
    let invalid = [
        ("", "data"),
        ("1pi", "data"),
        ("pi.name", "data"),
        ("xMl", "data"),
        ("XML-Stylesheet", "data"),
        ("pi", "contains > terminator"),
    ];

    for (target, data) in invalid {
        let mut arena = PatchValidationArena::default();
        arena
            .apply_batch(&[DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            }])
            .expect("seed document");
        let before = crate::dom_snapshot::DomSnapshot::new(
            &arena.materialize().expect("seed materializes"),
            crate::dom_snapshot::DomSnapshotOptions::default(),
        )
        .render();
        let err = arena
            .apply_batch(&[DomPatch::CreateProcessingInstruction {
                key: PatchKey(2),
                target: target.to_string(),
                data: data.to_string(),
            }])
            .expect_err("invalid PI payload must be rejected");
        assert!(
            err.to_string()
                .contains("invalid parser-created PI payload"),
            "unexpected error for target={target:?} data={data:?}: {err}"
        );
        let after = crate::dom_snapshot::DomSnapshot::new(
            &arena.materialize().expect("failed payload is atomic"),
            crate::dom_snapshot::DomSnapshotOptions::default(),
        )
        .render();
        assert_eq!(before, after);
    }
}

fn document(key: u32) -> DomPatch {
    DomPatch::CreateDocument {
        key: PatchKey(key),
        doctype: None,
    }
}

fn doctype(key: u32) -> DomPatch {
    DomPatch::CreateDocumentType {
        key: PatchKey(key),
        name: Some("html".into()),
        public_id: None,
        system_id: None,
    }
}

fn element(key: u32) -> DomPatch {
    DomPatch::CreateElement {
        key: PatchKey(key),
        name: html_name("div"),
        attributes: Vec::new(),
    }
}

fn append(parent: u32, child: u32) -> DomPatch {
    DomPatch::AppendChild {
        parent: PatchKey(parent),
        child: PatchKey(child),
    }
}

fn before(parent: u32, child: u32, anchor: u32) -> DomPatch {
    DomPatch::InsertBefore {
        parent: PatchKey(parent),
        child: PatchKey(child),
        before: PatchKey(anchor),
    }
}

fn assert_keyed_state_eq(actual: &PatchValidationArena, expected: &PatchValidationArena) {
    assert_eq!(actual.root, expected.root);
    assert_eq!(actual.nodes, expected.nodes);
    assert_eq!(actual.allocated, expected.allocated);
}

fn check_rejected_batch(
    start: &PatchValidationArena,
    patches: &[DomPatch],
) -> (
    PatchValidationArena,
    PatchValidationError,
    PatchValidationError,
) {
    // The failed trusted clone drops at the end of this statement.
    let trusted_error = start.clone().apply_batch_trusted(patches).unwrap_err();
    let mut reference = start.clone();
    let public_error = reference.apply_batch(patches).unwrap_err();
    assert_keyed_state_eq(&reference, start);
    (reference, trusted_error, public_error)
}

fn check_completion(
    start: &PatchValidationArena,
    patches: &[DomPatch],
    valid: bool,
) -> PatchValidationArena {
    if !valid {
        // These cases have a single violation or a defined order of errors.
        let (reference, trusted_error, public_error) = check_rejected_batch(start, patches);
        assert_eq!(trusted_error, public_error, "{patches:?}");
        return reference;
    }
    let mut trusted = start.clone();
    let mut reference = start.clone();
    trusted.apply_batch_trusted(patches).unwrap();
    reference.apply_batch(patches).unwrap();
    trusted.assert_invariants(&mut Default::default()).unwrap();
    assert_keyed_state_eq(&trusted, &reference);
    if trusted.root.is_some() {
        crate::dom_snapshot::assert_dom_eq(
            &trusted.materialize().unwrap(),
            &reference.materialize().unwrap(),
            Default::default(),
        );
    } else {
        assert!(trusted.nodes.is_empty() && reference.nodes.is_empty());
    }
    reference
}

#[test]
fn completion_preserves_equal_siblings_by_key_and_rolls_back_rejected_edits() {
    use super::model::PatchKind;

    let text = |id| DomPatch::CreateText {
        key: PatchKey(id),
        text: "same".into(),
    };
    let siblings = check_completion(
        &PatchValidationArena::default(),
        &[
            document(1),
            element(2),
            append(1, 2),
            text(3),
            append(2, 3),
            text(4),
            append(2, 4),
            before(2, 4, 3),
        ],
        true,
    );
    assert_eq!(
        siblings.nodes[&PatchKey(2)].children,
        [PatchKey(4), PatchKey(3)]
    );
    let update = DomPatch::SetText {
        key: PatchKey(4),
        text: "edited".into(),
    };
    // Stage a reorder and a payload change before creating two detached nodes.
    let (rolled_back, trusted_error, public_error) =
        check_rejected_batch(&siblings, &[append(2, 4), update.clone(), text(5), text(6)]);
    let detached = [PatchKey(5), PatchKey(6)].map(|key| {
        PatchValidationError::new(
            "post-apply invariants",
            format!("detached non-root node {key:?}"),
        )
    });
    for error in [trusted_error, public_error] {
        assert!(detached.contains(&error), "{error}");
    }
    let accepted = check_completion(&rolled_back, &[update, text(5), append(2, 5)], true);
    assert_eq!(
        accepted.nodes[&PatchKey(2)].children,
        [PatchKey(4), PatchKey(3), PatchKey(5)]
    );
    for (id, expected) in [(4, "edited"), (3, "same"), (5, "same")] {
        assert!(
            matches!(&accepted.nodes[&PatchKey(id)].kind, PatchKind::Text { text } if text == expected)
        );
    }
}

#[test]
fn document_order_marker_survives_unrelated_operations_and_noops() {
    let batch = [
        document(1),
        element(2),
        append(1, 2),
        doctype(3),
        append(1, 3),
        element(4),
        append(2, 4),
        DomPatch::SetAttributes {
            key: PatchKey(4),
            attributes: vec![],
        },
        append(2, 4),
        append(1, 3),
    ];
    check_completion(&Default::default(), &batch, false);
    let mut repaired = batch.to_vec();
    repaired.push(before(1, 3, 2));
    check_completion(&Default::default(), &repaired, true);
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
#[test]
fn obsolete_document_order_marker_skips_replacement_root() {
    let batch = [
        document(1),
        doctype(2),
        append(1, 2),
        DomPatch::RemoveNode { key: PatchKey(1) },
        document(3),
        element(4),
        append(3, 4),
    ];
    let mut arena = PatchValidationArena::default();
    let (result, work) = super::work::measure(|| arena.apply_batch_trusted(&batch));
    result.unwrap();
    assert_eq!(work.document_children, 0);
    assert_eq!(work.completion_patches, batch.len());
    check_completion(&Default::default(), &batch, true);
}

#[test]
fn replacement_document_order_marker_rejects_invalid_doctype() {
    let batch = [
        document(1),
        doctype(2),
        append(1, 2),
        DomPatch::RemoveNode { key: PatchKey(1) },
        document(3),
        element(4),
        append(3, 4),
        doctype(5),
        append(3, 5),
    ];
    check_completion(&Default::default(), &batch, false);
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
fn append_document_comment_and_pi(comment: u32, pi: u32) -> [DomPatch; 4] {
    [
        DomPatch::CreateComment {
            key: PatchKey(comment),
            text: "comment".into(),
        },
        append(1, comment),
        DomPatch::CreateProcessingInstruction {
            key: PatchKey(pi),
            target: "pi".into(),
            data: "data".into(),
        },
        append(1, pi),
    ]
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
fn wide_document_with_trailing_doctype() -> PatchValidationArena {
    let mut patches = vec![document(1)];
    for comment in (10..138).step_by(2) {
        patches.extend(append_document_comment_and_pi(comment, comment + 1));
    }
    patches.extend([doctype(2), append(1, 2)]);
    let mut arena = PatchValidationArena::default();
    arena.apply_batch(&patches).unwrap();
    arena
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
fn measure_document_child_visits(arena: &mut PatchValidationArena, patches: &[DomPatch]) -> usize {
    let mut reference = arena.clone();
    let (result, work) = super::work::measure(|| arena.apply_batch_trusted(patches));
    result.unwrap();
    assert_eq!(work.completion_batches, 1);
    assert_eq!(work.full_checks, 0);
    reference.apply_batch(patches).unwrap();
    assert_keyed_state_eq(arena, &reference);
    work.document_children
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
#[test]
fn wide_document_skips_child_scan_for_noops_and_unrelated_edits() {
    let mut arena = wide_document_with_trailing_doctype();
    assert_eq!(
        measure_document_child_visits(&mut arena, &[append(1, 2)]),
        0
    );
    assert_eq!(
        measure_document_child_visits(&mut arena, &append_document_comment_and_pi(3, 4)),
        0
    );
    arena.apply_batch(&[element(5), append(1, 5)]).unwrap();
    assert_eq!(
        measure_document_child_visits(
            &mut arena,
            &[
                DomPatch::CreateText {
                    key: PatchKey(6),
                    text: "first".into()
                },
                append(5, 6),
                DomPatch::AppendText {
                    key: PatchKey(6),
                    text: " second".into()
                },
            ]
        ),
        0
    );
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
#[test]
fn wide_document_scans_children_once_after_repaired_order_edits() {
    let mut arena = wide_document_with_trailing_doctype();
    // Insert an element before the doctype, move the doctype ahead of it,
    // then append the doctype and repair its order again in the same batch.
    let visits = measure_document_child_visits(
        &mut arena,
        &[
            element(3),
            before(1, 3, 2),
            before(1, 2, 3),
            append(1, 2),
            before(1, 2, 3),
        ],
    );
    assert_eq!(visits, arena.nodes[&PatchKey(1)].children.len());
}

#[test]
fn trusted_completion_matches_reference_through_moves_and_repaired_forests() {
    let mut start = PatchValidationArena::default();
    start.apply_batch(&nested_template_seed_batch()).unwrap();
    for (patches, valid) in [
        (
            vec![
                element(20),
                element(21),
                append(20, 21),
                append(4, 20),
                before(4, 21, 20),
                append(7, 20),
                DomPatch::RemoveNode { key: PatchKey(6) },
            ],
            true,
        ),
        (vec![element(20), append(20, 3), append(4, 20)], false),
        (vec![element(20), element(21), append(20, 21)], false),
        (vec![doctype(20), append(2, 20), before(1, 20, 2)], true),
        (vec![doctype(20), before(1, 20, 2), append(2, 20)], false),
        (
            vec![
                doctype(20),
                before(1, 20, 2),
                doctype(21),
                before(1, 21, 20),
            ],
            false,
        ),
        (
            vec![
                doctype(20),
                before(1, 20, 2),
                element(21),
                before(1, 21, 20),
            ],
            false,
        ),
        (vec![element(20), before(2, 20, 5)], false),
        (vec![element(20), append(5, 20)], false),
        (vec![element(0)], false),
        (vec![element(2)], false),
        (vec![append(1, 2)], false),
        (vec![DomPatch::Clear, document(1)], false),
        (vec![element(20), DomPatch::Clear], false),
        (vec![DomPatch::Clear], true),
    ] {
        check_completion(&start, &patches, valid);
    }
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
#[test]
fn full_checker_work_is_counted_separately() {
    use super::work::{Work, measure};
    let patches = nested_template_seed_batch();
    let mut arena = PatchValidationArena::default();
    let ((), work) = measure(|| arena.apply_batch_trusted(&patches).unwrap());
    assert_eq!(work.completion_patches, patches.len());
    assert_eq!(work.full_checks, 0);
    assert_eq!(work.audit_nodes, 0);
    let ((), reference) = measure(|| arena.assert_invariants(&mut Default::default()).unwrap());
    assert_eq!(reference.full_checks, 1);
    assert_eq!(reference.audit_nodes, arena.nodes.len());
    assert!(reference.audit_edges > 0 && reference.reverse_comparisons > 0);
    assert_eq!(reference.completion_patches, 0);
    assert!(std::panic::catch_unwind(|| measure(|| measure(|| ()))).is_err());
    assert_eq!(measure(|| ()).1, Work::default());
}

#[cfg(all(test, not(feature = "perf-tests"), not(feature = "count-alloc")))]
#[test]
fn trusted_leaf_appends_and_text_updates_do_no_sibling_or_full_tree_work() {
    use super::work::measure;
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch_trusted(&[document(1), element(2), append(1, 2)])
        .unwrap();
    let count = 256;
    let ((), work) = measure(|| {
        for id in 3..3 + count {
            arena
                .apply_batch_trusted(&[
                    DomPatch::CreateText {
                        key: PatchKey(id),
                        text: String::new(),
                    },
                    append(2, id),
                ])
                .unwrap();
        }
    });
    assert_eq!(work.completion_batches, count as usize);
    assert_eq!(work.completion_patches, 2 * count as usize);
    assert_eq!(work.cycle_pops, count as usize);
    assert_eq!(
        (
            work.full_checks,
            work.document_children,
            work.mutation_sibling_tests
        ),
        (0, 0, 0)
    );
    let ((), updates) = measure(|| {
        for id in 3..3 + count {
            arena
                .apply_batch_trusted(&[DomPatch::AppendText {
                    key: PatchKey(id),
                    text: "x".into(),
                }])
                .unwrap();
        }
    });
    assert_eq!(updates.completion_patches, count as usize);
    assert_eq!(
        (
            updates.cycle_pops,
            updates.mutation_sibling_tests,
            updates.document_children,
            updates.full_checks
        ),
        (0, 0, 0, 0)
    );
    arena.assert_invariants(&mut Default::default()).unwrap();
}
