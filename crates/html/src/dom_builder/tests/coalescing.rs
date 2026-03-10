use super::super::*;
use super::helpers::PatchArena;
use crate::dom_patch::DomPatch;
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
use crate::types::{AtomTable, Node, Token, TokenStream};
use std::sync::Arc;

#[test]
fn tree_builder_coalesces_text_per_parent() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");
    let span = atoms.intern_ascii_lowercase("span");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::TextOwned { index: 1 },
        Token::StartTag {
            name: span,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 2 },
        Token::TextOwned { index: 3 },
        Token::EndTag(span),
        Token::TextOwned { index: 4 },
        Token::EndTag(div),
    ];
    let text_pool = vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);
    let mut builder = TreeBuilder::with_capacity_and_config(
        stream.tokens().len().saturating_add(1),
        TreeBuilderConfig {
            coalesce_text: true,
        },
    );
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let Node::Document { children, .. } = dom else {
        panic!("expected document node");
    };
    let Node::Element {
        name,
        children: div_children,
        ..
    } = &children[0]
    else {
        panic!("expected div element");
    };
    assert_eq!(name.as_ref(), "div");
    assert_eq!(div_children.len(), 3);

    let Node::Text { text, .. } = &div_children[0] else {
        panic!("expected coalesced div text");
    };
    assert_eq!(text, "ab");

    let Node::Element {
        name,
        children: span_children,
        ..
    } = &div_children[1]
    else {
        panic!("expected span element");
    };
    assert_eq!(name.as_ref(), "span");
    assert_eq!(span_children.len(), 1);
    let Node::Text { text, .. } = &span_children[0] else {
        panic!("expected coalesced span text");
    };
    assert_eq!(text, "cd");

    let Node::Text { text, .. } = &div_children[2] else {
        panic!("expected trailing div text");
    };
    assert_eq!(text, "e");
}

#[test]
fn tree_builder_toggle_coalesce_flushes_pending_text() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::TextOwned { index: 1 },
        Token::EndTag(div),
    ];
    let text_pool = vec!["a".to_string(), "b".to_string()];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity_and_config(
        stream.tokens().len().saturating_add(1),
        TreeBuilderConfig {
            coalesce_text: true,
        },
    );
    let atoms = stream.atoms();
    builder
        .push_token(&stream.tokens()[0], atoms, &stream)
        .unwrap();
    builder
        .push_token(&stream.tokens()[1], atoms, &stream)
        .unwrap();
    builder.set_coalesce_text(false).unwrap();
    builder
        .push_token(&stream.tokens()[2], atoms, &stream)
        .unwrap();
    builder
        .push_token(&stream.tokens()[3], atoms, &stream)
        .unwrap();
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let Node::Document { children, .. } = dom else {
        panic!("expected document node");
    };
    let Node::Element {
        name,
        children: div_children,
        ..
    } = &children[0]
    else {
        panic!("expected div element");
    };
    assert_eq!(name.as_ref(), "div");
    assert_eq!(div_children.len(), 2);

    let Node::Text { text, .. } = &div_children[0] else {
        panic!("expected flushed text node");
    };
    assert_eq!(text, "a");
    let Node::Text { text, .. } = &div_children[1] else {
        panic!("expected following text node");
    };
    assert_eq!(text, "b");
}

#[test]
fn tree_builder_coalesces_text_with_settext_patches() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::TextOwned { index: 1 },
        Token::EndTag(div),
    ];
    let text_pool = vec!["a".to_string(), "b".to_string()];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity_and_config(
        stream.tokens().len().saturating_add(1),
        TreeBuilderConfig {
            coalesce_text: true,
        },
    );
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    builder.finish().unwrap();
    let patches = builder.take_patches();
    let expected = builder.materialize().unwrap();

    let create_text_count = patches
        .iter()
        .filter(|p| matches!(p, DomPatch::CreateText { .. }))
        .count();
    let text_key = patches.iter().find_map(|p| match p {
        DomPatch::CreateText { key, .. } => Some(*key),
        _ => None,
    });
    let set_text_count = patches
        .iter()
        .filter(|p| matches!(p, DomPatch::SetText { .. }))
        .count();
    let text_append_count = patches
        .iter()
        .filter(|p| match (p, text_key) {
            (DomPatch::AppendChild { child, .. }, Some(key)) => key == *child,
            _ => false,
        })
        .count();

    assert_eq!(create_text_count, 1, "expected one text node creation");
    assert!(text_key.is_some(), "expected a text node key");
    assert_eq!(set_text_count, 1, "expected one text update");
    assert_eq!(text_append_count, 1, "expected text appended once");

    let mut arena = PatchArena::default();
    arena.apply(&patches);
    let actual = arena.materialize();
    assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
}

#[test]
fn tree_builder_single_text_chunk_emits_no_settext() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::EndTag(div),
    ];
    let text_pool = vec!["a".to_string()];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity_and_config(
        stream.tokens().len().saturating_add(1),
        TreeBuilderConfig {
            coalesce_text: true,
        },
    );
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    builder.finish().unwrap();
    let patches = builder.take_patches();
    let expected = builder.materialize().unwrap();

    let create_text_count = patches
        .iter()
        .filter(|p| matches!(p, DomPatch::CreateText { .. }))
        .count();
    let text_key = patches.iter().find_map(|p| match p {
        DomPatch::CreateText { key, .. } => Some(*key),
        _ => None,
    });
    let set_text_count = patches
        .iter()
        .filter(|p| matches!(p, DomPatch::SetText { .. }))
        .count();
    let text_append_count = patches
        .iter()
        .filter(|p| match (p, text_key) {
            (DomPatch::AppendChild { child, .. }, Some(key)) => key == *child,
            _ => false,
        })
        .count();

    assert_eq!(create_text_count, 1, "expected one text node creation");
    assert!(text_key.is_some(), "expected a text node key");
    assert_eq!(set_text_count, 0, "expected no text update");
    assert_eq!(text_append_count, 1, "expected text appended once");

    let mut arena = PatchArena::default();
    arena.apply(&patches);
    let actual = arena.materialize();
    assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
}

#[test]
fn tree_builder_many_text_chunks_is_bounded() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");

    let chunk_count = 10_000usize;
    let mut tokens = Vec::with_capacity(chunk_count + 2);
    tokens.push(Token::StartTag {
        name: div,
        attributes: Vec::new(),
        self_closing: false,
    });
    for i in 0..chunk_count {
        tokens.push(Token::TextOwned { index: i });
    }
    tokens.push(Token::EndTag(div));

    let text_pool = vec!["x".to_string(); chunk_count];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity_and_config(
        stream.tokens().len().saturating_add(1),
        TreeBuilderConfig {
            coalesce_text: true,
        },
    );
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    builder.finish().unwrap();
    let patches = builder.take_patches();
    let expected = builder.materialize().unwrap();

    let create_text_count = patches
        .iter()
        .filter(|p| matches!(p, DomPatch::CreateText { .. }))
        .count();
    let text_key = patches.iter().find_map(|p| match p {
        DomPatch::CreateText { key, .. } => Some(*key),
        _ => None,
    });
    let set_text_count = patches
        .iter()
        .filter(|p| matches!(p, DomPatch::SetText { .. }))
        .count();
    let text_append_count = patches
        .iter()
        .filter(|p| match (p, text_key) {
            (DomPatch::AppendChild { child, .. }, Some(key)) => key == *child,
            _ => false,
        })
        .count();

    assert_eq!(create_text_count, 1, "expected one text node creation");
    assert!(text_key.is_some(), "expected a text node key");
    assert_eq!(set_text_count, 1, "expected one text update");
    assert_eq!(text_append_count, 1, "expected text appended once");

    let mut arena = PatchArena::default();
    arena.apply(&patches);
    let actual = arena.materialize();
    assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
}
