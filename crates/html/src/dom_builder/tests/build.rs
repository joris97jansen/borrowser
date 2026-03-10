use super::super::*;
use crate::types::{AtomTable, Node, Token, TokenStream};
use std::sync::Arc;

#[test]
fn build_dom_stress_deep_nesting() {
    let depth: usize = 10_000;
    let mut tokens = Vec::with_capacity(depth * 2);
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");

    for _ in 0..depth {
        tokens.push(Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        });
    }
    for _ in 0..depth {
        tokens.push(Token::EndTag(div));
    }

    let stream = TokenStream::new(tokens, atoms, Arc::from(""), Vec::new());
    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    builder.push_stream(&stream).unwrap();
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let mut current = &dom;
    let mut seen = 0usize;
    loop {
        match current {
            Node::Document { children, .. } => {
                assert_eq!(children.len(), 1);
                current = &children[0];
            }
            Node::Element { name, children, .. } => {
                assert_eq!(name.as_ref(), "div");
                seen += 1;
                if seen == depth {
                    assert!(children.is_empty());
                    break;
                }
                assert_eq!(children.len(), 1);
                current = &children[0];
            }
            Node::Text { .. } | Node::Comment { .. } => {
                panic!("unexpected leaf node before reaching depth");
            }
        }
    }
}

#[test]
fn tree_builder_incremental_basic() {
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
        Token::StartTag {
            name: span,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 1 },
        Token::EndTag(span),
        Token::EndTag(div),
    ];
    let text_pool = vec!["hi".to_string(), "bye".to_string()];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let Node::Document { children, .. } = dom else {
        panic!("expected document node");
    };
    assert_eq!(children.len(), 1);
    let Node::Element { name, children, .. } = &children[0] else {
        panic!("expected div element");
    };
    assert_eq!(name.as_ref(), "div");
    assert_eq!(children.len(), 2);
    let Node::Text { text, .. } = &children[0] else {
        panic!("expected leading text node");
    };
    assert_eq!(text, "hi");
    let Node::Element { name, children, .. } = &children[1] else {
        panic!("expected span element");
    };
    assert_eq!(name.as_ref(), "span");
    assert_eq!(children.len(), 1);
    let Node::Text { text, .. } = &children[0] else {
        panic!("expected nested text node");
    };
    assert_eq!(text, "bye");
}

#[test]
fn tree_builder_self_closing_does_not_capture_following_text() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");

    let tokens = vec![
        Token::TextOwned { index: 0 },
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: true,
        },
        Token::TextOwned { index: 1 },
    ];
    let text_pool = vec!["a".to_string(), "b".to_string()];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity_and_config(
        stream.tokens().len().saturating_add(1),
        TreeBuilderConfig {
            coalesce_text: true,
        },
    );
    builder.push_stream(&stream).unwrap();
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let Node::Document { children, .. } = dom else {
        panic!("expected document node");
    };
    assert_eq!(children.len(), 3);
    let Node::Text { text, .. } = &children[0] else {
        panic!("expected leading text node");
    };
    assert_eq!(text, "a");
    let Node::Element { name, .. } = &children[1] else {
        panic!("expected element node");
    };
    assert_eq!(name.as_ref(), "div");
    let Node::Text { text, .. } = &children[2] else {
        panic!("expected trailing text node");
    };
    assert_eq!(text, "b");
}

#[test]
fn tree_builder_flushes_pending_text_on_comment_and_starttag() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::Comment(crate::TextPayload::Owned("x".to_string())),
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
    builder.push_stream(&stream).unwrap();
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let Node::Document { children, .. } = dom else {
        panic!("expected document node");
    };
    let Node::Element { children, .. } = &children[0] else {
        panic!("expected element node");
    };
    assert_eq!(children.len(), 3);
    let Node::Text { text, .. } = &children[0] else {
        panic!("expected leading text node");
    };
    assert_eq!(text, "a");
    let Node::Comment { text, .. } = &children[1] else {
        panic!("expected comment node");
    };
    assert_eq!(text, "x");
    let Node::Text { text, .. } = &children[2] else {
        panic!("expected trailing text node");
    };
    assert_eq!(text, "b");
}
