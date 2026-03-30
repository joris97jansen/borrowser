mod checker;
mod errors;
mod model;

pub use checker::{check_dom_invariants, check_patch_invariants};
pub use errors::{DomInvariantError, PatchInvariantError};
pub use model::{DomInvariantNode, DomInvariantNodeKind, DomInvariantState};

#[cfg(test)]
mod tests;
