mod adjusted;
mod attributes;
mod dispatch;
mod tables;

pub(in crate::html5::tree_builder) use adjusted::{AdjustedCurrentNode, AdjustedCurrentNodeSource};
pub(in crate::html5::tree_builder) use attributes::*;
pub(in crate::html5::tree_builder) use dispatch::*;
pub(in crate::html5::tree_builder) use tables::*;

#[cfg(test)]
mod tests;
