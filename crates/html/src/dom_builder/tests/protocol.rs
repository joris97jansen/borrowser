use super::super::*;
use crate::dom_patch::DomPatch;
use crate::types::{AtomTable, Token, TokenStream};
use std::sync::Arc;

#[test]
fn tree_builder_rejects_push_after_finish() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");
    let stream = TokenStream::new(
        vec![Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: true,
        }],
        atoms,
        Arc::from(""),
        Vec::new(),
    );

    let mut builder = TreeBuilder::with_capacity(4);
    builder.finish().unwrap();
    let err = builder
        .push_token(&stream.tokens()[0], stream.atoms(), &stream)
        .unwrap_err();
    assert!(matches!(err, TreeBuilderError::Finished));
}

#[test]
fn tree_builder_rejects_duplicate_doctype() {
    let tokens = vec![
        Token::Doctype(crate::TextPayload::Owned("html".to_string())),
        Token::Doctype(crate::TextPayload::Owned("html".to_string())),
    ];
    let stream = TokenStream::new(tokens, AtomTable::new(), Arc::from(""), Vec::new());
    let atoms = stream.atoms();
    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    builder
        .push_token(&stream.tokens()[0], atoms, &stream)
        .unwrap();
    let err = builder
        .push_token(&stream.tokens()[1], atoms, &stream)
        .unwrap_err();
    assert!(matches!(
        err,
        TreeBuilderError::Protocol("duplicate doctype")
    ));
}

#[test]
fn doctype_after_document_emission_is_rejected() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");
    let stream = TokenStream::new(
        vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: true,
            },
            Token::Doctype(crate::TextPayload::Owned("html".to_string())),
        ],
        atoms,
        Arc::from(""),
        Vec::new(),
    );

    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    builder
        .push_token(&stream.tokens()[0], stream.atoms(), &stream)
        .unwrap();

    let err = builder
        .push_token(&stream.tokens()[1], stream.atoms(), &stream)
        .unwrap_err();
    assert!(matches!(
        err,
        TreeBuilderError::Protocol("doctype after document emission")
    ));
}

#[test]
fn create_document_is_first_patch() {
    let input = "<div>hi</div>";
    let stream = crate::tokenize(input);

    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    builder.push_stream(&stream).unwrap();
    builder.finish().unwrap();

    let patches = builder.take_patches();
    assert!(matches!(
        patches.first(),
        Some(DomPatch::CreateDocument { .. })
    ));
}

#[test]
fn create_document_is_first_patch_without_doctype() {
    let input = "hi<div>ok</div>";
    let stream = crate::tokenize(input);

    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    builder.push_stream(&stream).unwrap();
    builder.finish().unwrap();

    let patches = builder.take_patches();
    assert!(matches!(
        patches.first(),
        Some(DomPatch::CreateDocument { .. })
    ));
}
