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
//! - CSS-owned invalidation impact classification for supported longhands
//!
//! `PropertyId` is the stable identity for one supported property.
//! `PropertyId::metadata()` is the normative source for inheritance,
//! initial/default, specified-value-shape, computed-value-shape, and
//! invalid-value, value-range, and invalidation-impact facts. Downstream code
//! must not re-encode those facts in separate match tables.
//!
//! AD5 exposes a derived value-boundary inventory for docs and tests. That
//! inventory is not a second property registry; it reports the
//! specified/computed value facts already owned by `PropertyMetadata`.
//!
//! This module deliberately does not own cascade precedence, selector
//! matching, property-specific parsers, or layout-facing interpretation.

mod boundary;
mod data;
mod registry;
mod shorthand;
mod types;

pub use boundary::{
    PropertyValueBoundary, SpecifiedToComputedConversionRule, property_value_boundaries,
    property_value_boundary, property_value_boundary_debug_snapshot,
};
pub use registry::{
    PropertyRegistration, PropertyRegistry, property_coverage_debug_snapshot, property_registry,
    property_registry_metadata_debug_snapshot,
};
pub use shorthand::{
    ShorthandId, ShorthandRegistration, ShorthandRegistry, shorthand_registry,
    shorthand_registry_debug_snapshot,
};
pub use types::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
    PropertyInvalidValuePolicy, PropertyInvalidationImpact, PropertyLengthSignPolicy,
    PropertyMetadata, PropertySpecifiedValueKind,
};

#[cfg(test)]
mod tests;
