//! Property-aware parsed specified values for supported CSS properties.
//!
//! This module sits between the model-layer `DeclarationValue` syntax tree and
//! computed values. It validates authored values against the supported
//! property registry and produces typed specified values without performing
//! inheritance, initial/default fallback, layout-dependent resolution, or
//! computed-value normalization.

mod color;
mod display;
mod error;
mod length;
mod parse;
mod value;

pub use error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind};
pub use parse::parse_specified_value;
pub use value::{
    SpecifiedColor, SpecifiedColorKeyword, SpecifiedColorSyntax, SpecifiedDisplay,
    SpecifiedDisplayKeyword, SpecifiedHexColor, SpecifiedLength, SpecifiedLengthOrAuto,
    SpecifiedLengthOrNone, SpecifiedLengthUnit, SpecifiedPropertyValue, SpecifiedValue,
};

#[cfg(test)]
mod tests;
