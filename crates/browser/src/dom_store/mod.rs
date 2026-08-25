mod arena;
mod document;
mod error;
mod materialize;
mod mutation;
mod store;

pub(crate) use error::DomIdentityResolutionError;
pub use error::DomPatchError;
pub(crate) use mutation::{
    DomMutationPrecisionFailure, DomMutationSnapshotInvariantError, DomMutationSnapshotLimits,
    ExactDomMutationDetails, ExactStoreAttributeMutation, ExactStoreTextMutation,
};
pub use store::DomStore;
pub(crate) use store::ResolvedMutationNodeIds;

#[cfg(test)]
mod tests;
