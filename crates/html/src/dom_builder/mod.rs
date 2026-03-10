mod api;
mod arena;
mod debug;
mod materialize;
mod patches;
mod resolver;
mod text;
mod tree_builder;

pub use api::{TreeBuilderConfig, TreeBuilderError, TreeBuilderResult, build_owned_dom};
pub use resolver::TokenTextResolver;
pub use tree_builder::TreeBuilder;

#[cfg(test)]
mod tests;
