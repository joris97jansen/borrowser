use super::PatchValidationArena;
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

fn template_seed_batch() -> Vec<DomPatch> {
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
