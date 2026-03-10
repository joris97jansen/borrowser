use super::super::*;
use crate::types::{AtomTable, Token, TokenStream};
use std::sync::Arc;

#[test]
fn debug_arena_stats_track_nodes_edges_and_text_bytes() {
    let mut atoms = AtomTable::new();
    let div = atoms.intern_ascii_lowercase("div");
    let tokens = vec![
        Token::Doctype(crate::TextPayload::Owned("html".to_string())),
        Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        },
        Token::TextOwned { index: 0 },
        Token::Comment(crate::TextPayload::Owned("c".to_string())),
        Token::EndTag(div),
    ];
    let text_pool = vec!["hi".to_string()];
    let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

    let mut builder = TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
    builder.push_stream(&stream).unwrap();
    builder.finish().unwrap();

    let stats = builder.debug_arena_stats();
    assert_eq!(stats.nodes, 4, "expected document, element, text, comment");
    assert_eq!(stats.edges, 3, "expected doc->div and div->(text,comment)");
    assert_eq!(
        stats.text_bytes,
        "html".len() + "hi".len() + "c".len(),
        "expected doctype, text, and comment bytes"
    );

    let atom_count = stream.atoms().debug_atom_count();
    assert_eq!(atom_count, 1, "expected one atom interned");
}
