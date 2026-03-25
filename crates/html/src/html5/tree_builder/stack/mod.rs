//! Stack of open elements helpers.

mod foster;
mod scope;
mod stack;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use stack::OpenElementsStack;
pub(crate) use types::{OpenElement, ScopeKeyMatch, ScopeKind, ScopeTagSet};

#[cfg(test)]
pub(crate) use types::FosterParentingAnchorIndices;
