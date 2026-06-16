use crate::{
    model::{ValueComponent, ValueToken},
    properties::PropertyId,
    syntax::CssNumericKind,
};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::{ident_keyword, parse_number_text},
    value::{SpecifiedZIndex, SpecifiedZIndexValue},
};

pub(super) fn parse_z_index(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedZIndex, SpecifiedValueParseError> {
    if let Some((keyword, span)) = ident_keyword(property, component)? {
        return if keyword == "auto" {
            Ok(SpecifiedZIndex {
                span,
                value: SpecifiedZIndexValue::Auto,
            })
        } else {
            Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ))
        };
    }

    let ValueComponent::Token(ValueToken::Number { span, kind, text }) = component else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        ));
    };

    if *kind != CssNumericKind::Integer {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::InvalidInteger,
        ));
    }

    let (_number, numeric_value) = parse_number_text(property, text)?;
    if numeric_value < i32::MIN as f64 || numeric_value > i32::MAX as f64 {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::IntegerOutOfRange,
        ));
    }

    Ok(SpecifiedZIndex {
        span: *span,
        value: SpecifiedZIndexValue::Integer(numeric_value as i32),
    })
}
