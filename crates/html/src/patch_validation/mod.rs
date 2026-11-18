mod apply;
mod error;
mod materialize;
mod model;
#[cfg(feature = "parser-conformance")]
mod semantic_compare;
mod validate;

pub use error::PatchValidationError;
pub use model::PatchValidationArena;
pub(crate) use validate::PatchValidationScratch;

#[cfg(test)]
mod tests;
