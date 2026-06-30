//! Stable snapshot serializer for the engine-facing CSS model.
//!
//! The snapshot surface is intentionally aligned with the model layer rather
//! than authored CSS text. It is deterministic, versioned, and suitable for
//! regression fixtures.

mod labels;
mod parse;
mod stylesheet;
mod syntax;
mod values;

pub(crate) use self::parse::serialize_declaration_list_parse_for_snapshot;
pub use self::parse::serialize_stylesheet_parse_for_snapshot;
pub use self::stylesheet::{
    serialize_declaration_for_snapshot, serialize_rule_for_snapshot,
    serialize_stylesheet_for_snapshot, serialize_value_for_snapshot,
};
