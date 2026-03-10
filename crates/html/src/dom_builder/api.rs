use crate::types::{Node, TokenStream};

use super::TreeBuilder;

/// Build a fully owned DOM tree from a token stream.
///
/// This materializes the internal arena into recursive `Node` storage and can
/// be expensive; avoid calling it on hot preview paths where patches suffice.
pub fn build_owned_dom(stream: &TokenStream) -> Node {
    #[cfg(feature = "parse-guards")]
    crate::parse_guards::record_full_build_dom();
    let tokens = stream.tokens();
    let mut builder = TreeBuilder::with_capacity(tokens.len().saturating_add(1));
    builder
        .push_stream(stream)
        .expect("dom builder token push should be infallible");

    builder
        .finish()
        .expect("dom builder finish should be infallible");

    builder
        .materialize()
        .expect("dom builder materialize should be infallible")
}

#[derive(Debug)]
pub enum TreeBuilderError {
    Finished,
    InvariantViolation(&'static str),
    Protocol(&'static str),
    #[allow(
        dead_code,
        reason = "reserved for upcoming insertion mode / spec handling"
    )]
    Unsupported(&'static str),
}

pub type TreeBuilderResult<T> = Result<T, TreeBuilderError>;

impl core::fmt::Display for TreeBuilderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TreeBuilderError::Finished => write!(f, "tree builder is already finished"),
            TreeBuilderError::InvariantViolation(msg) => {
                write!(f, "invariant violation: {msg}")
            }
            TreeBuilderError::Protocol(msg) => {
                write!(f, "protocol violation: {msg}")
            }
            TreeBuilderError::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
    }
}

impl std::error::Error for TreeBuilderError {}

#[derive(Clone, Copy, Debug)]
pub struct TreeBuilderConfig {
    pub coalesce_text: bool,
}

impl Default for TreeBuilderConfig {
    fn default() -> Self {
        Self {
            coalesce_text: true,
        }
    }
}
