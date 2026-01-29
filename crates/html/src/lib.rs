pub mod collect;
pub mod debug;
pub mod dom_diff;
#[cfg(any(test, feature = "dom-snapshot"))]
pub mod dom_snapshot;
pub mod dom_utils;
pub mod golden_corpus;
pub mod head;
#[cfg(feature = "parse-guards")]
pub mod parse_guards;
#[cfg(any(test, feature = "test-harness"))]
pub mod perf_fixtures;
#[cfg(all(test, feature = "perf-tests"))]
mod perf_guards_heavy;
#[cfg(test)]
mod perf_guards_smoke;
#[cfg(test)]
mod streaming_parity;
#[cfg(any(test, feature = "test-harness"))]
pub mod test_harness;
#[cfg(test)]
mod test_utils;
pub mod traverse;

#[cfg(feature = "html5-parse")]
pub mod html5;

mod dom_builder;
mod dom_patch;
mod entities;
mod tokenizer;
mod types;

use memchr::{memchr, memchr2};

pub fn is_html(ct: &Option<String>) -> bool {
    let Some(value) = ct.as_deref() else {
        return false;
    };
    contains_ignore_ascii_case(value, b"text/html")
        || contains_ignore_ascii_case(value, b"application/xhtml")
}

fn contains_ignore_ascii_case(haystack: &str, needle: &[u8]) -> bool {
    let hay = haystack.as_bytes();
    let n = needle.len();
    if n == 0 {
        return true;
    }
    let hay_len = hay.len();
    if hay_len < n {
        return false;
    }
    let first = needle[0];
    let (a, b) = if first.is_ascii_alphabetic() {
        (first.to_ascii_lowercase(), first.to_ascii_uppercase())
    } else {
        (first, first)
    };
    if n == 1 {
        if a == b {
            return memchr(a, hay).is_some();
        }
        return memchr2(a, b, hay).is_some();
    }
    let mut i = 0;
    while i + n <= hay_len {
        let rel = if a == b {
            memchr(a, &hay[i..])
        } else {
            memchr2(a, b, &hay[i..])
        };
        let Some(rel) = rel else {
            return false;
        };
        let pos = i + rel;
        if pos + n <= hay_len && hay[pos..pos + n].eq_ignore_ascii_case(needle) {
            return true;
        }
        i = pos + 1;
    }
    false
}

pub use crate::dom_builder::build_owned_dom;
pub use crate::dom_builder::{
    TokenTextResolver, TreeBuilder, TreeBuilderConfig, TreeBuilderError, TreeBuilderResult,
};
pub use crate::dom_diff::{
    DomDiffState, diff_dom, diff_dom_stateless, diff_dom_with_state, diff_from_empty,
};
pub use crate::dom_patch::{DomPatch, PatchKey};
pub use crate::tokenizer::Tokenizer;
pub use crate::tokenizer::tokenize;
pub use crate::types::{AtomId, AtomTable, AttributeValue, Node, TextPayload, Token, TokenStream};

#[cfg(feature = "internal-api")]
pub mod internal {
    pub use super::types::{Id, NodeId, NodeKey};
}
