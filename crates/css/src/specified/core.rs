use crate::{
    model::{ValueComponent, ValueText, ValueToken},
    properties::PropertyId,
    syntax::{CssNumericKind, CssSpan},
    values::{CssIntegerValue, CssKeywordValue, CssNumberScalar, CssNumberValue},
};

use super::error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error};

pub(super) fn keyword_value(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<Option<CssKeywordValue>, SpecifiedValueParseError> {
    let ValueComponent::Token(ValueToken::Ident { span, text }) = component else {
        return Ok(None);
    };

    Ok(Some(CssKeywordValue::new(
        *span,
        resolve_text(property, text)?.to_ascii_lowercase(),
    )))
}

pub(super) fn number_value(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<Option<CssNumberValue>, SpecifiedValueParseError> {
    let ValueComponent::Token(ValueToken::Number { span, kind, text }) = component else {
        return Ok(None);
    };

    number_value_from_parts(property, *span, *kind, text).map(Some)
}

pub(super) fn integer_value(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<Option<CssIntegerValue>, SpecifiedValueParseError> {
    let Some(number) = number_value(property, component)? else {
        return Ok(None);
    };

    if number.kind() != CssNumericKind::Integer {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::InvalidInteger,
        ));
    }

    let numeric_value = number.numeric_value();
    if numeric_value < i32::MIN as f64 || numeric_value > i32::MAX as f64 {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::IntegerOutOfRange,
        ));
    }

    Ok(Some(CssIntegerValue::new(number, numeric_value as i32)))
}

pub(super) fn number_value_from_parts(
    property: PropertyId,
    span: CssSpan,
    kind: CssNumericKind,
    text: &ValueText,
) -> Result<CssNumberValue, SpecifiedValueParseError> {
    let repr = resolve_text(property, text)?.to_string();
    let value = repr
        .parse::<f64>()
        .ok()
        .and_then(CssNumberScalar::new)
        .ok_or_else(|| error(property, SpecifiedValueParseErrorKind::InvalidLengthNumber))?;

    Ok(CssNumberValue::new(span, repr, value, kind))
}

pub(super) fn resolve_text(
    property: PropertyId,
    text: &ValueText,
) -> Result<&str, SpecifiedValueParseError> {
    text.text
        .as_deref()
        .ok_or_else(|| error(property, SpecifiedValueParseErrorKind::UnresolvedTokenText))
}

pub(super) fn unsupported_component_error(
    property: PropertyId,
    component: &ValueComponent,
) -> SpecifiedValueParseError {
    let kind = match component {
        ValueComponent::Function(_) => SpecifiedValueParseErrorKind::UnsupportedFunction,
        ValueComponent::Token(ValueToken::Url { .. } | ValueToken::BadUrl { .. }) => {
            SpecifiedValueParseErrorKind::UnsupportedUrl
        }
        ValueComponent::Token(ValueToken::String { .. } | ValueToken::BadString { .. }) => {
            SpecifiedValueParseErrorKind::UnsupportedString
        }
        _ => SpecifiedValueParseErrorKind::UnsupportedComponent,
    };

    error(property, kind)
}
