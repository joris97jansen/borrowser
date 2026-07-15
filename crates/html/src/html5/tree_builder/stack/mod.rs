//! Stack of open elements helpers.

mod end_tag;
mod foster;
mod open_elements;
mod scope;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use open_elements::OpenElementsStack;
pub(crate) use types::{
    ExactOpenElementRemoval, InBodyEndTagScan, OpenElement, ScopeKeyMatch, ScopeKind, ScopeTagSet,
};

#[cfg(test)]
pub(crate) use types::FosterParentingAnchorIndices;
