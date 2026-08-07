use super::*;
use html::conformance::{ObservationState, ObservedToken, ObservedTokenAttribute};

#[test]
fn token_v2_distinguishes_absent_doctype_name_from_literal_null() {
    let state = ObservationState::Captured(vec![
        ObservedToken::Doctype {
            name: None,
            public_id: None,
            system_id: None,
            force_quirks: true,
        },
        ObservedToken::Doctype {
            name: Some("null".to_string()),
            public_id: None,
            system_id: None,
            force_quirks: false,
        },
        ObservedToken::Eof,
    ]);
    let written = token_v2::write(&state).expect("captured tokens serialize");
    assert!(written.data().bytes().contains("name=null public-id=null"));
    assert!(
        written
            .data()
            .bytes()
            .contains("name=\"null\" public-id=null")
    );
    let parsed = token_v2::read(written.data().bytes().as_bytes()).expect("writer output parses");
    assert_eq!(parsed.data().bytes(), written.data().bytes());
}

#[test]
fn strict_v2_framing_rejects_legacy_compatibility_forms() {
    for malformed in [
        "# format: html5-token-v2\r\nTOKEN ordinal=1 kind=eof\r\n",
        "# format: html5-token-v2\n\nTOKEN ordinal=1 kind=eof\n",
        "# format: html5-token-v2\n# comment\nTOKEN ordinal=1 kind=eof\n",
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=eof",
        "\u{feff}# format: html5-token-v2\nTOKEN ordinal=1 kind=eof\n",
    ] {
        assert!(
            token_v2::read(malformed.as_bytes()).is_err(),
            "accepted {malformed:?}"
        );
    }
}

#[test]
fn empty_collection_and_required_singleton_rules_are_surface_specific() {
    assert!(parse_errors::read(b"# format: html5-parse-errors-v1\n").is_ok());
    assert!(tree::read(b"# format: html5-dom-v3\n").is_ok());
    assert!(patches::read(b"# format: html5-dompatch-v3\n").is_ok());
    assert!(document_mode::read(b"# format: html5-document-mode-v1\n").is_err());
    assert!(token_v2::read(b"# format: html5-token-v2\n").is_err());
}

#[test]
fn final_invariants_v1_is_strict_complete_and_canonically_ordered() {
    use html::conformance::{
        DomFinalizationChecks, InputFinalizationChecks, InvariantOutcome, ParserFinalizationReport,
        PatchFinalizationChecks, TokenizerFinalizationChecks, TreeBuilderFinalizationChecks,
    };
    let satisfied = InvariantOutcome::Satisfied;
    let report = ParserFinalizationReport {
        input: InputFinalizationChecks {
            decoder_carry_empty: satisfied.clone(),
            preprocessing_flushed: satisfied.clone(),
        },
        tokenizer: TokenizerFinalizationChecks {
            eof_emitted_once: satisfied.clone(),
            pending_constructs_flushed: satisfied.clone(),
            output_accounted_for: satisfied.clone(),
        },
        tree_builder: TreeBuilderFinalizationChecks {
            pending_table_text_empty: satisfied.clone(),
            insertion_mode_valid: satisfied.clone(),
            open_elements_consistent: satisfied.clone(),
            active_formatting_consistent: satisfied.clone(),
            template_modes_consistent: satisfied.clone(),
            form_pointer_valid: satisfied.clone(),
        },
        dom: DomFinalizationChecks {
            parent_child_links_valid: satisfied.clone(),
            namespaces_valid: satisfied.clone(),
            template_associations_valid: satisfied.clone(),
        },
        patches: PatchFinalizationChecks {
            all_patches_materialized: satisfied.clone(),
            live_tree_matches_materialized_dom: satisfied,
        },
    };
    let written = final_invariants::write(&ObservationState::Captured(report))
        .expect("captured final report");
    let text = written.data().bytes();
    assert_eq!(text.lines().count(), 17);
    assert!(text.lines().nth(1).is_some_and(|line| {
        line == "INVARIANT ordinal=1 field=decoder-carry-empty outcome=satisfied"
    }));
    assert!(text.lines().nth(16).is_some_and(|line| {
        line == "INVARIANT ordinal=16 field=live-tree-matches-materialized-dom outcome=satisfied"
    }));
    final_invariants::read(text.as_bytes()).expect("writer output parses");

    let reordered = text.replace(
        "ordinal=1 field=decoder-carry-empty",
        "ordinal=1 field=preprocessing-flushed",
    );
    assert!(final_invariants::read(reordered.as_bytes()).is_err());
    let truncated = text.lines().take(16).collect::<Vec<_>>().join("\n") + "\n";
    assert!(final_invariants::read(truncated.as_bytes()).is_err());
}

#[test]
fn readers_validate_framing_without_reimplementing_parser_semantics() {
    let implausible_tree = b"# format: html5-dom-v3\nNODE path=/root[0] kind=element namespace=svg local-name=\"html\"\n";
    assert!(tree::read(implausible_tree).is_ok());
    let implausible_patch =
        b"# format: html5-dompatch-v3\nPATCH operation=1 kind=remove-node node=\"node-99\"\n";
    assert!(patches::read(implausible_patch).is_ok());
}

#[test]
fn token_v2_reader_rejects_unknown_spellings_duplicate_fields_bad_escapes_and_bad_locations() {
    for malformed in [
        "# format: html5-token-v3\nTOKEN ordinal=1 kind=eof\n",
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=unknown\n",
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=character data=\"\\x\"\nTOKEN ordinal=2 kind=eof\n",
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=character data=\"x\" data=\"y\"\nTOKEN ordinal=2 kind=eof\n",
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=start-tag name=\"x\" self-closing=false\nTOKEN_ATTRIBUTE token=1 index=1 name=\"a\" value=\"b\"\nTOKEN ordinal=2 kind=eof\n",
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=eof\nTOKEN ordinal=2 kind=comment data=\"after\"\n",
    ] {
        assert!(
            token_v2::read(malformed.as_bytes()).is_err(),
            "accepted {malformed:?}"
        );
    }
}

#[test]
fn canonical_token_writer_preserves_vector_order_and_is_byte_identical() {
    let state = ObservationState::Captured(vec![
        ObservedToken::StartTag {
            name: "x".to_string(),
            attributes: vec![
                ObservedTokenAttribute {
                    name: "z".to_string(),
                    value: "1".to_string(),
                },
                ObservedTokenAttribute {
                    name: "a".to_string(),
                    value: "2".to_string(),
                },
            ],
            self_closing: false,
        },
        ObservedToken::Eof,
    ]);
    let first = token_v2::write(&state).unwrap();
    let second = token_v2::write(&state).unwrap();
    assert_eq!(first, second);
    let z = first.data().bytes().find("index=0 name=\"z\"").unwrap();
    let a = first.data().bytes().find("index=1 name=\"a\"").unwrap();
    assert!(z < a);
}

#[test]
fn every_requested_canonical_writer_is_repeatable_and_strictly_readable() {
    use html::conformance::{
        ObservationRequest, ParserObservationInput, ParserObservationRequest,
        ParserObservationTarget, ScalarObservationRequest, execute_parser_observation,
    };

    let capture = ObservationRequest::Capture { capacity: 1_024 };
    let result = execute_parser_observation(ParserObservationRequest {
        target: ParserObservationTarget::DocumentParser,
        input: ParserObservationInput::Utf8("<!doctype html><body class=a><body class=b>"),
        tokens: capture,
        parse_errors: capture,
        implementation_diagnostics: capture,
        transitions: capture,
        unsupported_features: capture,
        document_mode: ScalarObservationRequest::Capture,
        tree: capture,
        patches: capture,
        final_invariants: html::conformance::FinalInvariantRequest::NotRequested,
    })
    .expect("production canonical observation");

    for surface in [
        ExpectationSurface::Tokens,
        ExpectationSurface::ParseErrors,
        ExpectationSurface::ImplementationDiagnostics,
        ExpectationSurface::DocumentMode,
        ExpectationSurface::Tree,
        ExpectationSurface::Patches,
        ExpectationSurface::Transitions,
        ExpectationSurface::UnsupportedFeatures,
    ] {
        let first = serialize_snapshot(surface, &result).expect("requested surface serializes");
        let second = serialize_snapshot(surface, &result).expect("repeat serialization");
        assert_eq!(first, second, "{} serialization changed", surface.name());
        let parsed = read_snapshot(surface, first.snapshot().bytes().as_bytes())
            .expect("canonical writer output passes its strict reader");
        assert_eq!(parsed.surface(), surface);
        assert_eq!(parsed.format(), first.format());
    }

    assert!(
        !serialize_snapshot(ExpectationSurface::Transitions, &result)
            .unwrap()
            .snapshot()
            .is_empty()
    );
    assert!(
        !serialize_snapshot(ExpectationSurface::UnsupportedFeatures, &result)
            .unwrap()
            .snapshot()
            .is_empty()
    );
}

#[test]
fn canonical_tree_and_patch_writers_preserve_typed_vector_order_without_relabeling() {
    use html::conformance::{
        CanonicalParserResult, ObservedDomAttribute, ObservedPatchOperation, ObservedPatchStream,
        ObservedTree, ObservedTreeNode, PatchNodeLabel,
    };
    use html::{AttributeNamespace, ElementNamespace};

    let mut result = CanonicalParserResult {
        tokens: ObservationState::NotRequested,
        parse_errors: ObservationState::NotRequested,
        implementation_diagnostics: ObservationState::NotRequested,
        document_mode: ObservationState::NotRequested,
        tree: ObservationState::Captured(ObservedTree {
            roots: vec![ObservedTreeNode::Document {
                children: vec![ObservedTreeNode::Element {
                    namespace: ElementNamespace::Html,
                    local_name: "x".to_string(),
                    attributes: vec![
                        ObservedDomAttribute {
                            namespace: AttributeNamespace::None,
                            prefix: None,
                            local_name: "z".to_string(),
                            value: "1".to_string(),
                        },
                        ObservedDomAttribute {
                            namespace: AttributeNamespace::None,
                            prefix: None,
                            local_name: "a".to_string(),
                            value: "2".to_string(),
                        },
                    ],
                    children: Vec::new(),
                }],
            }],
        }),
        patches: ObservationState::NotRequested,
        transitions: ObservationState::NotRequested,
        unsupported_features: ObservationState::NotRequested,
        final_invariants: ObservationState::NotRequested,
    };
    let tree = serialize_snapshot(ExpectationSurface::Tree, &result).unwrap();
    let tree_bytes = tree.snapshot().bytes();
    assert!(
        tree_bytes.find("index=0 namespace=none").unwrap()
            < tree_bytes.find("index=1 namespace=none").unwrap()
    );
    assert!(
        tree_bytes.find("local-name=\"z\"").unwrap() < tree_bytes.find("local-name=\"a\"").unwrap()
    );

    result.tree = ObservationState::NotRequested;
    result.patches = ObservationState::Captured(ObservedPatchStream {
        operations: vec![
            ObservedPatchOperation::RemoveNode {
                node: PatchNodeLabel("node-2".to_string()),
            },
            ObservedPatchOperation::RemoveNode {
                node: PatchNodeLabel("node-1".to_string()),
            },
        ],
    });
    let patches = serialize_snapshot(ExpectationSurface::Patches, &result).unwrap();
    let patch_bytes = patches.snapshot().bytes();
    assert!(patch_bytes.find("operation=1").unwrap() < patch_bytes.find("operation=2").unwrap());
    assert!(patch_bytes.find("node-2").unwrap() < patch_bytes.find("node-1").unwrap());
}

#[test]
fn token_writer_rejects_missing_or_nonfinal_eof() {
    assert!(
        token_v2::write(&ObservationState::Captured(vec![
            ObservedToken::Character {
                data: "x".to_string(),
            },
        ]))
        .is_err()
    );
    assert!(
        token_v2::write(&ObservationState::Captured(vec![
            ObservedToken::Eof,
            ObservedToken::Comment {
                data: "after".to_string(),
            },
        ]))
        .is_err()
    );
}

#[test]
fn surface_readers_reject_unknown_closed_spellings() {
    assert!(parse_errors::read(b"# format: html5-parse-errors-v1\nPARSE_ERROR occurrence=1 stage=tokenizer code=standard:not-real recovery=null position=unavailable:parser-did-not-provide-position context=absent context-token=null context-mode=null context-namespace=null\n").is_err());
    assert!(implementation_diagnostics::read(b"# format: html5-implementation-diagnostics-v1\nIMPLEMENTATION_DIAGNOSTIC occurrence=1 stage=tokenizer code=parser-guardrail:not-real payload=consecutive-stall-steps:1 position=unavailable:parser-did-not-provide-position context=absent context-token=null context-mode=null context-namespace=null\n").is_err());
    assert!(unsupported_features::read(b"# format: html5-unsupported-features-v1\nUNSUPPORTED_FEATURE occurrence=1 subsystem=tree-construction feature=not-real context-token=null context-mode=null context-namespace=null\n").is_err());
}

#[test]
fn canonical_tree_writer_is_iterative_for_deep_trees() {
    use html::ElementNamespace;
    use html::conformance::{ObservedTree, ObservedTreeNode};

    const DEPTH: usize = 2_048;
    let mut node = ObservedTreeNode::Text {
        data: "leaf".to_string(),
    };
    for _ in 0..DEPTH {
        node = ObservedTreeNode::Element {
            namespace: ElementNamespace::Html,
            local_name: "x".to_string(),
            attributes: Vec::new(),
            children: vec![node],
        };
    }
    let tree = ObservedTree {
        roots: vec![ObservedTreeNode::Document {
            children: vec![node],
        }],
    };
    std::thread::Builder::new()
        .name("iterative-canonical-tree-writer".to_string())
        .stack_size(64 * 1024)
        .spawn(move || {
            let state = ObservationState::Captured(tree);
            let written = tree::write(&state).expect("deep canonical tree serializes iteratively");
            assert_eq!(written.data().record_count(), DEPTH + 2);
            tree::read(written.data().bytes().as_bytes())
                .expect("deep writer output remains strict");
            let ObservationState::Captured(tree) = state else {
                unreachable!()
            };
            drop_tree_iteratively(tree);
        })
        .expect("deep-tree test thread starts")
        .join()
        .expect("deep-tree test thread remains stack-safe");
}

#[test]
fn nested_template_contents_writer_output_round_trips_through_tree_framing() {
    use html::conformance::{ObservedTemplateContents, ObservedTree, ObservedTreeNode};

    let state = ObservationState::Captured(ObservedTree {
        roots: vec![ObservedTreeNode::HtmlTemplateElement {
            attributes: Vec::new(),
            ordinary_children: Vec::new(),
            contents: ObservedTemplateContents {
                children: vec![ObservedTreeNode::HtmlTemplateElement {
                    attributes: Vec::new(),
                    ordinary_children: Vec::new(),
                    contents: ObservedTemplateContents {
                        children: vec![ObservedTreeNode::Text {
                            data: "nested".to_string(),
                        }],
                    },
                }],
            },
        }],
    });

    let written = tree::write(&state).expect("nested templates serialize");
    let expected = concat!(
        "# format: html5-dom-v3\n",
        "NODE path=/root[0] kind=html-template-host\n",
        "TEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\n",
        "NODE path=/root[0]/contents/child[0] kind=html-template-host\n",
        "TEMPLATE_CONTENTS path=/root[0]/contents/child[0]/contents host=/root[0]/contents/child[0]\n",
        "NODE path=/root[0]/contents/child[0]/contents/child[0] kind=text data=\"nested\"\n",
    );
    assert_eq!(written.data().bytes(), expected);
    assert_eq!(written.data().bytes().matches("/contents").count(), 7);
    tree::read(written.data().bytes().as_bytes())
        .expect("strict reader accepts nested template contents paths");
}

fn drop_tree_iteratively(mut tree: html::conformance::ObservedTree) {
    use html::conformance::ObservedTreeNode;

    let mut work = std::mem::take(&mut tree.roots);
    while let Some(node) = work.pop() {
        match node {
            ObservedTreeNode::Document { mut children }
            | ObservedTreeNode::Element { mut children, .. } => work.append(&mut children),
            ObservedTreeNode::HtmlTemplateElement {
                mut ordinary_children,
                mut contents,
                ..
            } => {
                work.append(&mut ordinary_children);
                work.append(&mut contents.children);
            }
            ObservedTreeNode::DocumentType { .. }
            | ObservedTreeNode::Comment { .. }
            | ObservedTreeNode::Text { .. }
            | ObservedTreeNode::ProcessingInstruction { .. } => {}
        }
    }
}

#[test]
fn tree_reader_enforces_canonical_preorder_and_template_framing() {
    let malformed = [
        "# format: html5-dom-v3\nNODE path=/root[0]/child[0] kind=text data=\"x\"\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=element namespace=html local-name=\"x\"\nNODE path=/root[0]/child[0] kind=text data=\"x\"\nATTRIBUTE path=/root[0] index=0 namespace=none prefix=null local-name=\"a\" value=\"b\"\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nNODE path=/root[0]/contents/child[0] kind=text data=\"x\"\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nNODE path=/root[0]/child[0] kind=text data=\"late\"\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=document\nNODE path=/root[0]/child[0] kind=element namespace=html local-name=\"x\"\nNODE path=/root[0]/child[0]/child[0] kind=text data=\"x\"\nNODE path=/root[0]/child[1] kind=text data=\"y\"\nNODE path=/root[0]/child[0]/child[1] kind=text data=\"late\"\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=document\nNODE path=/root[0]/child[1] kind=text data=\"skipped\"\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\n",
    ];
    for snapshot in malformed {
        assert!(
            tree::read(snapshot.as_bytes()).is_err(),
            "accepted:\n{snapshot}"
        );
    }

    let implausible_but_framed = "# format: html5-dom-v3\nNODE path=/root[0] kind=document\nNODE path=/root[0]/child[0] kind=element namespace=svg local-name=\"html\"\nNODE path=/root[0]/child[0]/child[0] kind=element namespace=mathml local-name=\"body\"\n";
    assert!(tree::read(implausible_but_framed.as_bytes()).is_ok());
}

#[test]
fn tree_reader_rejects_malformed_nested_template_boundaries() {
    let malformed = [
        // Inner boundary before the inner template host.
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nTEMPLATE_CONTENTS path=/root[0]/contents/child[0]/contents host=/root[0]/contents/child[0]\n",
        // Inner contents child before the inner boundary.
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nNODE path=/root[0]/contents/child[0] kind=html-template-host\nNODE path=/root[0]/contents/child[0]/contents/child[0] kind=text data=\"early\"\n",
        // Duplicate boundary for the same inner host.
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nNODE path=/root[0]/contents/child[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents/child[0]/contents host=/root[0]/contents/child[0]\nTEMPLATE_CONTENTS path=/root[0]/contents/child[0]/contents host=/root[0]/contents/child[0]\n",
        // Ordinary child after the inner host entered its contents phase.
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nNODE path=/root[0]/contents/child[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents/child[0]/contents host=/root[0]/contents/child[0]\nNODE path=/root[0]/contents/child[0]/child[0] kind=text data=\"late\"\n",
        // Complete but consecutive contents segments have no intervening host.
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nTEMPLATE_CONTENTS path=/root[0]/contents/contents host=/root[0]/contents\n",
        // Noncanonical and malformed child indices remain lexically invalid.
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nNODE path=/root[0]/contents/child[01] kind=html-template-host\n",
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nNODE path=/root[0]/contents/child[-1] kind=html-template-host\n",
        // The inner template host must receive its own boundary at EOF.
        "# format: html5-dom-v3\nNODE path=/root[0] kind=html-template-host\nTEMPLATE_CONTENTS path=/root[0]/contents host=/root[0]\nNODE path=/root[0]/contents/child[0] kind=html-template-host\n",
    ];
    for snapshot in malformed {
        assert!(
            tree::read(snapshot.as_bytes()).is_err(),
            "accepted malformed nested template snapshot:\n{snapshot}"
        );
    }
}

#[test]
fn patch_codec_requires_canonical_labels_and_decimals() {
    let malformed = [
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=remove-node node=\"node-0\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=remove-node node=\"node-01\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=remove-node node=\"other-1\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=remove-node node=\"\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=remove-node node=\"node-x\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=remove-node node=node-1\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=create-comment node=\"arbitrary\" data=\"x\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=01 kind=remove-node node=\"node-1\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=+1 kind=remove-node node=\"node-1\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=create-element node=\"node-1\" namespace=html local-name=\"x\"\nPATCH_ATTRIBUTE operation=1 index=00 namespace=none prefix=null local-name=\"a\" value=\"b\"\n",
        "# format: html5-dompatch-v3\nPATCH operation=1 kind=create-element node=\"node-1\" namespace=html local-name=\"x\"\nPATCH_ATTRIBUTE operation=01 index=0 namespace=none prefix=null local-name=\"a\" value=\"b\"\n",
    ];
    for snapshot in malformed {
        assert!(
            patches::read(snapshot.as_bytes()).is_err(),
            "accepted:\n{snapshot}"
        );
    }

    use html::conformance::{ObservedPatchOperation, ObservedPatchStream, PatchNodeLabel};
    assert!(
        patches::write(&ObservationState::Captured(ObservedPatchStream {
            operations: vec![ObservedPatchOperation::RemoveNode {
                node: PatchNodeLabel("arbitrary".to_string()),
            }],
        }))
        .is_err()
    );
}

#[test]
fn snapshot_surface_variants_are_distinct_and_compile_time_specific() {
    use std::any::TypeId;

    let _: fn(token_v2::ParsedTokenSnapshot) -> ParsedSnapshot = ParsedSnapshot::Tokens;
    let _: fn(patches::ParsedPatchesSnapshot) -> ParsedSnapshot = ParsedSnapshot::Patches;
    let _: fn(token_v2::CanonicalTokenSnapshot) -> CanonicalSnapshot = CanonicalSnapshot::Tokens;
    let _: fn(patches::CanonicalPatchesSnapshot) -> CanonicalSnapshot = CanonicalSnapshot::Patches;

    assert_ne!(
        TypeId::of::<token_v2::ParsedTokenSnapshot>(),
        TypeId::of::<patches::ParsedPatchesSnapshot>()
    );
    assert_ne!(
        TypeId::of::<token_v2::CanonicalTokenSnapshot>(),
        TypeId::of::<patches::CanonicalPatchesSnapshot>()
    );
}
