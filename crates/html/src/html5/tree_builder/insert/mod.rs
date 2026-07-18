mod create;
pub(in crate::html5::tree_builder) mod location;
pub(in crate::html5::tree_builder) use location::InsertionLocation;
mod scope;
mod text;

#[cfg(test)]
mod tests;
