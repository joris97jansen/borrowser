//! Cascade-local names for the shared CSS property system contract.
//!
//! The property universe itself is owned by `crate::properties`. Cascade keeps
//! these aliases so the Milestone R contract remains stable while both cascade
//! and computed-style work consume the same underlying property table.

pub use crate::properties::{
    InitialStyleValue, PropertyId as CascadePropertyId, PropertyInheritance as CascadeInheritance,
    PropertyLengthSignPolicy as CascadePropertyLengthSignPolicy,
    PropertyMetadata as CascadePropertyMetadata,
    PropertyRegistration as CascadePropertyRegistration,
    PropertyRegistry as CascadePropertyRegistry, property_registry as cascade_property_registry,
};
