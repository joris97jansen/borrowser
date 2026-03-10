use super::super::arena::NodeArena;
use super::super::*;
use crate::types::{AtomTable, Id, Node, NodeKey, Token, TokenStream};
use std::collections::HashSet;
use std::sync::Arc;

#[test]
fn build_dom_assigns_unique_ids() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");
    let p = atoms.intern_ascii_lowercase("p");
    let span = atoms.intern_ascii_lowercase("span");
    let ul = atoms.intern_ascii_lowercase("ul");
    let li = atoms.intern_ascii_lowercase("li");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: p,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::EndTag(p),
        Token::StartTag {
            name: span,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::Comment(crate::TextPayload::Owned("note".to_string())),
        Token::EndTag(span),
        Token::StartTag {
            name: ul,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: li,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::EndTag(li),
        Token::StartTag {
            name: li,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 1 },
        Token::EndTag(li),
        Token::EndTag(ul),
        Token::EndTag(div),
    ];
    let text_pool = vec!["hello".to_string(), "item".to_string()];
    let dom = build_owned_dom(&TokenStream::new(tokens, atoms, Arc::from(""), text_pool));

    let mut ids = HashSet::new();
    let mut count = 0usize;
    let mut stack = vec![&dom];

    while let Some(node) = stack.pop() {
        let id = node.id();
        assert_ne!(id, Id(0));
        assert!(ids.insert(id), "duplicate id {:?} in dom", id);
        count += 1;

        if let Node::Document { children, .. } | Node::Element { children, .. } = node {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }

    assert_eq!(ids.len(), count);
}

#[test]
fn tree_builder_ids_are_monotonic_and_start_at_root() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");
    let span = atoms.intern_ascii_lowercase("span");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: span,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::EndTag(span),
        Token::EndTag(div),
    ];
    let text_pool = vec!["hi".to_string()];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    assert_eq!(
        builder.debug_root_key(),
        NodeKey(1),
        "root key should be the first allocated"
    );
    let node_count = builder.debug_node_count();
    assert_eq!(
        builder.debug_next_key(),
        NodeKey(node_count + 1),
        "next key should be one past the last allocated node key"
    );
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let mut ids: Vec<Id> = Vec::new();
    let mut stack = vec![&dom];
    while let Some(node) = stack.pop() {
        ids.push(node.id());
        if let Node::Document { children, .. } | Node::Element { children, .. } = node {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }

    assert!(!ids.is_empty(), "expected at least the document node id");

    let root_id = dom.id();
    assert_eq!(root_id, Id(1), "root id should be the first allocated");

    let mut unique = HashSet::new();
    for id in &ids {
        assert!(unique.insert(*id), "duplicate id detected: {id:?}");
    }
}

#[test]
fn tree_builder_ids_are_never_reused() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");
    let span = atoms.intern_ascii_lowercase("span");

    let tokens = vec![
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: span,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::EndTag(span),
        Token::StartTag {
            name: span,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::EndTag(span),
        Token::EndTag(div),
    ];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), Vec::new());

    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    let atoms = stream.atoms();
    for token in stream.tokens() {
        builder.push_token(token, atoms, &stream).unwrap();
    }
    builder.finish().unwrap();
    let dom = builder.materialize().unwrap();

    let mut ids = HashSet::new();
    let mut stack = vec![&dom];
    while let Some(node) = stack.pop() {
        let id = node.id();
        assert!(ids.insert(id), "id reuse detected: {id:?}");
        if let Node::Document { children, .. } | Node::Element { children, .. } = node {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }
}

#[test]
fn arena_key_mapping_roundtrips() {
    let input = "<div>hi<span>yo</span><!--c--></div>";
    let stream = crate::tokenize(input);
    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    builder.push_stream(&stream).unwrap();

    fn index_of(arena: &NodeArena, key: NodeKey) -> Option<usize> {
        if key == NodeKey::INVALID {
            return None;
        }
        let k = key.0 as usize;
        let idx = *arena.key_to_index.get(k)?;
        if idx == NodeArena::MISSING {
            return None;
        }
        Some(idx)
    }

    for idx in 0..builder.arena.nodes.len() {
        let key = builder.arena.node_key(idx);
        let idx2 = index_of(&builder.arena, key).expect("key must resolve");
        assert_eq!(idx, idx2);
    }

    assert!(index_of(&builder.arena, NodeKey::INVALID).is_none());
    assert!(index_of(&builder.arena, builder.debug_next_key()).is_none());
}
