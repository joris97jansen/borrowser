pub mod collect;
pub mod debug;
pub mod dom_utils;
pub mod head;
pub mod traverse;

mod dom_builder;
mod entities;
mod tokenizer;
mod types;

pub fn is_html(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase())
        .map(|s| s.contains("text/html") || s.contains("application/xhtml"))
        .unwrap_or(false)
}

pub use crate::dom_builder::build_dom;
pub use crate::tokenizer::tokenize;
pub use crate::types::{AtomId, AtomTable, Id, Node, NodeId, Token, TokenStream};
