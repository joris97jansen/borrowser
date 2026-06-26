use crate::{
    model::{DeclarationValue, ValueComponent, ValueToken},
    properties::{PropertyId, PropertySpecifiedValueKind},
};

use super::{
    border::parse_border_style,
    color::parse_color,
    css_wide::parse_supported_css_wide_keyword,
    display::parse_display,
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    length::{parse_length, parse_length_percentage_or_auto, parse_length_percentage_or_none},
    outline::parse_outline_style,
    overflow::parse_overflow,
    position::parse_position,
    text_decoration::parse_text_decoration_line,
    value::{SpecifiedDeclarationValue, SpecifiedPropertyValue, SpecifiedValue},
    z_index::parse_z_index,
};

/// Parses one model-layer declaration value into a declaration-level specified
/// value, including CSS-wide keywords.
pub fn parse_specified_declaration_value(
    property: PropertyId,
    value: &DeclarationValue,
) -> Result<SpecifiedDeclarationValue, SpecifiedValueParseError> {
    parse_specified_declaration_value_with_limits(property, value, &SpecifiedValueLimits::default())
}

pub fn parse_specified_declaration_value_with_limits(
    property: PropertyId,
    value: &DeclarationValue,
    limits: &SpecifiedValueLimits,
) -> Result<SpecifiedDeclarationValue, SpecifiedValueParseError> {
    let component = sole_non_trivia_component(property, value, limits)?;
    if let Some(value) = parse_supported_css_wide_keyword(property, component)? {
        return Ok(SpecifiedDeclarationValue::CssWideKeyword { property, value });
    }

    parse_specified_value_component(property, component).map(|value| {
        SpecifiedDeclarationValue::Property(SpecifiedPropertyValue { property, value })
    })
}

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
    let specified = parse_specified_value_component(property, component)?;

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

fn parse_specified_value_component(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedValue, SpecifiedValueParseError> {
    let specified = match property.metadata().specified_value {
        PropertySpecifiedValueKind::BorderStyleKeyword => {
            SpecifiedValue::BorderStyle(parse_border_style(property, component)?)
        }
        PropertySpecifiedValueKind::OutlineStyleKeyword => {
            SpecifiedValue::OutlineStyle(parse_outline_style(property, component)?)
        }
        PropertySpecifiedValueKind::TextDecorationLineKeyword => {
            SpecifiedValue::TextDecorationLine(parse_text_decoration_line(property, component)?)
        }
        PropertySpecifiedValueKind::Color => {
            SpecifiedValue::Color(parse_color(property, component)?)
        }
        PropertySpecifiedValueKind::DisplayKeyword => {
            SpecifiedValue::Display(parse_display(property, component)?)
        }
        PropertySpecifiedValueKind::OverflowKeyword => {
            SpecifiedValue::Overflow(parse_overflow(property, component)?)
        }
        PropertySpecifiedValueKind::PositionKeyword => {
            SpecifiedValue::Position(parse_position(property, component)?)
        }
        PropertySpecifiedValueKind::ZIndex => {
            SpecifiedValue::ZIndex(parse_z_index(property, component)?)
        }
        PropertySpecifiedValueKind::AbsoluteLength => {
            SpecifiedValue::Length(parse_length(property, component)?)
        }
        PropertySpecifiedValueKind::LengthPercentageOrAuto => {
            SpecifiedValue::LengthPercentageOrAuto(parse_length_percentage_or_auto(
                property, component,
            )?)
        }
        PropertySpecifiedValueKind::LengthPercentageOrNone => {
            SpecifiedValue::LengthPercentageOrNone(parse_length_percentage_or_none(
                property, component,
            )?)
        }
    };

    debug_assert_eq!(
        specified.kind(),
        property.metadata().specified_value,
        "specified parser emitted a value kind that does not match property metadata"
    );

    Ok(specified)
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
