use crate::{
    model::{DeclarationValue, ValueComponent, ValueText, ValueToken},
    properties::{PropertyId, PropertySpecifiedValueKind},
    syntax::CssSpan,
};

use super::{
    color::parse_color,
    display::parse_display,
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    length::{parse_length, parse_length_or_auto, parse_length_or_none},
    value::{SpecifiedPropertyValue, SpecifiedValue},
};

/// Parses one model-layer declaration value into a property-aware specified
/// value.
pub fn parse_specified_value(
    property: PropertyId,
    value: &DeclarationValue,
) -> Result<SpecifiedPropertyValue, SpecifiedValueParseError> {
    parse_specified_value_with_limits(property, value, &SpecifiedValueLimits::default())
}

pub fn parse_specified_value_with_limits(
    property: PropertyId,
    value: &DeclarationValue,
    limits: &SpecifiedValueLimits,
) -> Result<SpecifiedPropertyValue, SpecifiedValueParseError> {
    let component = sole_non_trivia_component(property, value, limits)?;
    let specified = match property.metadata().specified_value {
        PropertySpecifiedValueKind::Color => {
            SpecifiedValue::Color(parse_color(property, component)?)
        }
        PropertySpecifiedValueKind::DisplayKeyword => {
            SpecifiedValue::Display(parse_display(property, component)?)
        }
        PropertySpecifiedValueKind::AbsoluteLength => {
            SpecifiedValue::Length(parse_length(property, component)?)
        }
        PropertySpecifiedValueKind::AbsoluteLengthOrAuto => {
            SpecifiedValue::LengthOrAuto(parse_length_or_auto(property, component)?)
        }
        PropertySpecifiedValueKind::AbsoluteLengthOrNone => {
            SpecifiedValue::LengthOrNone(parse_length_or_none(property, component)?)
        }
    };

    debug_assert_eq!(
        specified.kind(),
        property.metadata().specified_value,
        "specified parser emitted a value kind that does not match property metadata"
    );

    Ok(SpecifiedPropertyValue {
        property,
        value: specified,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecifiedValueLimits {
    pub max_components_per_value: usize,
}

impl Default for SpecifiedValueLimits {
    fn default() -> Self {
        Self {
            max_components_per_value: 4_096,
        }
    }
}

fn sole_non_trivia_component<'a>(
    property: PropertyId,
    value: &'a DeclarationValue,
    limits: &SpecifiedValueLimits,
) -> Result<&'a ValueComponent, SpecifiedValueParseError> {
    // Current S3-supported properties all use one non-trivia component.
    // Multi-value shorthands, functions, and property-specific component
    // grammars should replace this gate when those value families are added.
    if value.components.len() > limits.max_components_per_value {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::ResourceLimitExceeded,
        ));
    }

    let mut components = value
        .components
        .iter()
        .filter(|component| !is_trivia(component));
    let Some(component) = components.next() else {
        return Err(error(property, SpecifiedValueParseErrorKind::EmptyValue));
    };
    if components.next().is_some() {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnexpectedComponentCount,
        ));
    }
    Ok(component)
}

fn is_trivia(component: &ValueComponent) -> bool {
    matches!(
        component,
        ValueComponent::Token(ValueToken::Whitespace { .. } | ValueToken::Comment { .. })
    )
}

pub(super) fn ident_keyword(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<Option<(String, CssSpan)>, SpecifiedValueParseError> {
    let ValueComponent::Token(ValueToken::Ident { span, text }) = component else {
        return Ok(None);
    };

    Ok(Some((
        resolve_text(property, text)?.to_ascii_lowercase(),
        *span,
    )))
}

pub(super) fn parse_number_text(
    property: PropertyId,
    text: &ValueText,
) -> Result<(String, f64), SpecifiedValueParseError> {
    let number = resolve_text(property, text)?.to_string();
    let numeric_value = number
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| error(property, SpecifiedValueParseErrorKind::InvalidLengthNumber))?;

    Ok((number, numeric_value))
}

pub(super) fn resolve_text(
    property: PropertyId,
    text: &ValueText,
) -> Result<&str, SpecifiedValueParseError> {
    text.text
        .as_deref()
        .ok_or_else(|| error(property, SpecifiedValueParseErrorKind::UnresolvedTokenText))
}
