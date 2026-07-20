use super::helpers::{EmptyResolver, run_tree_builder_chunks};
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{DocumentParseContext, TextValue, Token};
use crate::html5::tree_builder::document::QuirksMode;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{
    DomInvariantNodeKind, Html5TreeBuilder, TreeBuilderConfig, check_dom_invariants,
};

#[test]
fn accepted_doctype_creates_document_type_node_before_document_element() {
    let patches = run_tree_builder_chunks(&["<!doctype html><p>ok"]);

    assert!(matches!(
        patches.as_slice(),
        [
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None
            },
            DomPatch::CreateDocumentType {
                key: PatchKey(2),
                name: Some(name),
                public_id: None,
                system_id: None
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2)
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: html,
                ..
            },
            ..
        ] if name == "html" && html.is_html("html")
    ));

    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize DOM");
    let crate::Node::Document { children, .. } = dom else {
        panic!("expected materialized document root");
    };
    assert!(matches!(
        children.as_slice(),
        [
            crate::Node::DocumentType {
                name: Some(name),
                public_id: None,
                system_id: None,
                ..
            },
            crate::Node::Element { element: html },
            ..
        ] if name == "html" && html.expanded_name().is_html("html")
    ));
}

#[test]
fn initial_comment_before_doctype_preserves_comment_doctype_html_order() {
    let patches = run_tree_builder_chunks(&["<!--pre--><!doctype html><p>ok"]);

    assert!(matches!(
        patches.as_slice(),
        [
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None
            },
            DomPatch::CreateComment {
                key: PatchKey(2),
                text
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2)
            },
            DomPatch::CreateDocumentType {
                key: PatchKey(3),
                name: Some(name),
                public_id: None,
                system_id: None
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(3)
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: html,
                ..
            },
            ..
        ] if text == "pre" && name == "html" && html.is_html("html")
    ));

    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize DOM");
    let crate::Node::Document { children, .. } = dom else {
        panic!("expected materialized document root");
    };
    assert!(matches!(
        children.as_slice(),
        [
            crate::Node::Comment { text, .. },
            crate::Node::DocumentType {
                name: Some(name),
                public_id: None,
                system_id: None,
                ..
            },
            crate::Node::Element { element: html },
            ..
        ] if text == "pre" && name == "html" && html.expanded_name().is_html("html")
    ));
}

#[test]
fn doctype_node_participates_in_parent_child_invariant_state() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");
    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");

    for token in [
        Token::Doctype {
            name: Some(html),
            public_id: None,
            system_id: None,
            force_quirks: false,
        },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("tree builder should process doctype/eof");
    }

    let state = builder.dom_invariant_state();
    check_dom_invariants(&state).expect("doctype DOM state must satisfy invariants");
    let root = state.root().expect("document root");
    let root_node = state.node(root).expect("root node exists");
    assert_eq!(
        root_node.children().first().copied(),
        Some(PatchKey(2)),
        "doctype should be the first document child for doctype+EOF input"
    );
    let doctype = state.node(PatchKey(2)).expect("doctype node exists");
    assert_eq!(doctype.kind(), DomInvariantNodeKind::DocumentType);
    assert_eq!(doctype.parent(), Some(root));
    assert!(doctype.children().is_empty());
}

#[test]
fn document_mode_is_parser_metadata_not_doctype_node_identity() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");
    let foo = ctx
        .atoms
        .intern_ascii_folded("foo")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::Doctype {
                name: Some(foo),
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("doctype should process");
    assert_eq!(
        builder.state_snapshot().quirks_mode,
        QuirksMode::Quirks,
        "non-html doctype should select parser-owned quirks metadata"
    );

    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("EOF should create document and doctype node");
    let patches = builder.drain_patches();
    assert!(patches.iter().any(|patch| matches!(
        patch,
        DomPatch::CreateDocumentType {
            key: PatchKey(2),
            name: Some(name),
            public_id: None,
            system_id: None,
        } if name == "foo"
    )));
    assert!(
        patches.iter().all(|patch| {
            !matches!(
                patch,
                DomPatch::CreateDocumentType {
                    name: Some(name), ..
                } if name.contains("quirks")
            )
        }),
        "document mode must not be encoded in doctype node payload"
    );
}

#[test]
fn initial_comment_before_quirks_doctype_keeps_document_mode_as_metadata() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");
    let foo = ctx
        .atoms
        .intern_ascii_folded("foo")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::Comment {
                text: TextValue::Owned("pre".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("initial comment should process");
    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::Initial,
        "Initial comments may create the document but must not hand off insertion mode"
    );

    let _ = builder
        .process(
            &Token::Doctype {
                name: Some(foo),
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("doctype after initial comment should process");

    assert_eq!(
        builder.state_snapshot().quirks_mode,
        QuirksMode::Quirks,
        "doctype after an Initial comment should still derive document mode"
    );
    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::BeforeHtml,
        "accepted doctype should hand off to BeforeHtml after the Initial comment path"
    );

    let state = builder.dom_invariant_state();
    check_dom_invariants(&state).expect("comment plus doctype state must satisfy invariants");
    let root = state.root().expect("document root");
    let root_node = state.node(root).expect("root node exists");
    assert_eq!(
        root_node.children(),
        &[PatchKey(2), PatchKey(3)],
        "document children should remain comment, then doctype before html insertion"
    );

    let patches = builder.drain_patches();
    assert!(patches.iter().any(|patch| matches!(
        patch,
        DomPatch::CreateDocumentType {
            key: PatchKey(3),
            name: Some(name),
            public_id: None,
            system_id: None,
        } if name == "foo"
    )));
    assert!(
        patches.iter().all(|patch| {
            !matches!(
                patch,
                DomPatch::CreateDocumentType {
                    name: Some(name), ..
                } if name.contains("quirks")
            )
        }),
        "document mode must not be encoded in doctype node payload"
    );
}
