use super::super::DomPatchError;
use super::support::{
    VersionSteps, apply_ok, assert_failed_apply_is_atomic, bootstrap_document,
    materialized_dom_lines, new_store_with_handle,
};
use html::{DomPatch, Node, PatchKey};

#[test]
fn parser_processing_instruction_patches_materialize_with_exact_payload_and_identity() {
    let parsed = html::parse_document(
        "<?Before?><!DOCTYPE html><?Middle value?><html><body><?Exact-Target alpha ? beta?><p>x</p></body></html><?After?>",
        html::HtmlParseOptions::default(),
    )
    .expect("PI document parse");
    let (mut store, handle) = new_store_with_handle(51);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &parsed.patches,
        "parser PI patch stream applies",
    );

    let dom = store.materialize(handle).expect("PI DOM materializes");
    let Node::Document { children, .. } = *dom else {
        panic!("expected document")
    };
    let processing_instructions = children
        .iter()
        .filter_map(|node| match node {
            Node::ProcessingInstruction {
                processing_instruction,
            } => Some(processing_instruction),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(processing_instructions.len(), 3);
    assert_eq!(processing_instructions[0].target(), "Before");
    assert_eq!(processing_instructions[1].data(), "value");
    assert_eq!(processing_instructions[2].target(), "After");
    assert!(processing_instructions.iter().all(|pi| pi.id().0 != 0));

    let lines = materialized_dom_lines(&store, handle);
    assert!(lines.iter().any(|line| {
        line.contains("processing-instruction target=\"Exact-Target\" data=\"alpha ? beta\"")
    }));
}

#[test]
fn runtime_rejects_invalid_processing_instruction_payloads_atomically() {
    let invalid = [
        ("", "data"),
        ("1pi", "data"),
        ("pi.name", "data"),
        ("XmL", "data"),
        ("xml-STYLESHEET", "data"),
        ("pi", "contains > terminator"),
    ];
    for (index, (target, data)) in invalid.into_iter().enumerate() {
        let (mut store, handle) = new_store_with_handle(60 + index as u64);
        let mut versions = VersionSteps::new();
        bootstrap_document(&mut store, handle, &mut versions, PatchKey(1));
        let (from, to) = versions.next_pair();
        let error = assert_failed_apply_is_atomic(
            &mut store,
            handle,
            from,
            to,
            &[DomPatch::CreateProcessingInstruction {
                key: PatchKey(2),
                target: target.to_string(),
                data: data.to_string(),
            }],
        );
        assert!(matches!(error, DomPatchError::Protocol(_)));
    }
}

#[test]
fn runtime_processing_instruction_is_a_leaf_and_failed_child_batch_is_atomic() {
    let (mut store, handle) = new_store_with_handle(70);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateProcessingInstruction {
                key: PatchKey(2),
                target: "pi".to_string(),
                data: String::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ],
        "seed PI document",
    );

    let (from, to) = versions.next_pair();
    let error = assert_failed_apply_is_atomic(
        &mut store,
        handle,
        from,
        to,
        &[
            DomPatch::CreateText {
                key: PatchKey(3),
                text: "child".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
        ],
    );
    assert!(matches!(error, DomPatchError::InvalidParent(PatchKey(2))));
}
