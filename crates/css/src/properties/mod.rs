//! Engine-owned CSS property system contract for the current supported subset.
//!
//! This module is the shared property table for the cascade and computed-style
//! layers. It owns:
//! - the supported property identifier universe
//! - the registry of supported properties and their canonical CSS names
//! - inheritance and initial/default metadata
//! - the boundary between typed specified-value parsing and typed computed
//!   values
//! - property-owned value-range metadata for specified-value validation
//! - the current-scope invalid-value handling rule
//!
//! `PropertyId` is the stable identity for one supported property.
//! `PropertyId::metadata()` is the normative source for inheritance,
//! initial/default, specified-value-shape, computed-value-shape, and
//! invalid-value and value-range facts. Downstream code must not re-encode
//! those facts in separate match tables.
//!
//! This module deliberately does not own cascade precedence, selector
//! matching, property-specific parsers, or layout-facing interpretation.

mod data;
mod registry;
mod types;

pub use registry::{PropertyRegistration, PropertyRegistry, property_registry};
pub use types::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
    PropertyInvalidValuePolicy, PropertyLengthSignPolicy, PropertyMetadata,
    PropertySpecifiedValueKind,
};

#[cfg(test)]
mod tests;
