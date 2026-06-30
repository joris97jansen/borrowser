//! Property-aware parsed specified values for supported CSS properties.
//!
//! This module sits between the model-layer `DeclarationValue` syntax tree and
//! computed values. It validates authored values against the supported
//! property registry and produces typed specified values without performing
//! inheritance, initial/default fallback, layout-dependent resolution, or
//! computed-value normalization.

mod border;
mod color;
mod core;
mod css_wide;
mod display;
mod error;
mod length;
mod outline;
mod overflow;
mod parse;
mod position;
mod shorthand;
mod text_decoration;
mod value;
mod z_index;

pub use error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind};
pub use parse::{
    SpecifiedValueLimits, parse_specified_declaration_value,
    parse_specified_declaration_value_with_limits, parse_specified_value,
    parse_specified_value_with_limits,
};
pub use shorthand::{
    ExpandedLonghandDeclaration, ShorthandExpansion, ShorthandExpansionError,
    ShorthandExpansionErrorKind, expand_shorthand_declaration, shorthand_expansion_debug_snapshot,
};
pub use value::{
    SpecifiedBorderStyle, SpecifiedBorderStyleKeyword, SpecifiedColor, SpecifiedColorKeyword,
    SpecifiedColorSyntax, SpecifiedDeclarationValue, SpecifiedDisplay, SpecifiedDisplayKeyword,
    SpecifiedHexColor, SpecifiedLength, SpecifiedLengthPercentage, SpecifiedLengthPercentageOrAuto,
    SpecifiedLengthPercentageOrNone, SpecifiedLengthUnit, SpecifiedOutlineStyle,
    SpecifiedOutlineStyleKeyword, SpecifiedOverflow, SpecifiedOverflowKeyword, SpecifiedPercentage,
    SpecifiedPosition, SpecifiedPositionKeyword, SpecifiedPropertyValue,
    SpecifiedTextDecorationLine, SpecifiedTextDecorationLineKeyword, SpecifiedValue,
    SpecifiedZIndex, SpecifiedZIndexValue,
};

#[cfg(test)]
mod tests;
