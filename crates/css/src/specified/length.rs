use crate::{
    model::{ValueComponent, ValueToken},
    properties::{PropertyId, PropertyLengthSignPolicy},
    values::{CssLengthPercentageValue, CssLengthUnit, CssLengthValue, CssPercentageValue},
};

use super::{
    core::{keyword_value, number_value_from_parts, resolve_text, unsupported_component_error},
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    value::{
        SpecifiedLength, SpecifiedLengthPercentage, SpecifiedLengthPercentageOrAuto,
        SpecifiedLengthPercentageOrNone,
    },
};

pub(super) fn parse_length_percentage_or_auto(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLengthPercentageOrAuto, SpecifiedValueParseError> {
    if let Some(keyword) = keyword_value(property, component)? {
        return if keyword.canonical() == "auto" {
            Ok(SpecifiedLengthPercentageOrAuto::Auto {
                span: keyword.span(),
            })
        } else {
            Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ))
        };
    }

    parse_length_percentage(property, component)
        .map(SpecifiedLengthPercentageOrAuto::LengthPercentage)
}

pub(super) fn parse_length_percentage_or_none(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLengthPercentageOrNone, SpecifiedValueParseError> {
    if let Some(keyword) = keyword_value(property, component)? {
        return if keyword.canonical() == "none" {
            Ok(SpecifiedLengthPercentageOrNone::None {
                span: keyword.span(),
            })
        } else {
            Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ))
        };
    }

    parse_length_percentage(property, component)
        .map(SpecifiedLengthPercentageOrNone::LengthPercentage)
}

pub(super) fn parse_length_percentage(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLengthPercentage, SpecifiedValueParseError> {
    match component {
        ValueComponent::Token(ValueToken::Percentage { span, kind, text }) => {
            let number = number_value_from_parts(property, *span, *kind, text)?;
            let numeric_value = number.numeric_value();
            reject_negative_if_needed(property, numeric_value)?;

            Ok(SpecifiedLengthPercentage {
                value: CssLengthPercentageValue::Percentage(CssPercentageValue::new(number)),
            })
        }
        _ => parse_length(property, component).map(|length| SpecifiedLengthPercentage {
            value: CssLengthPercentageValue::Length(length.value),
        }),
    }
}

pub(super) fn parse_length(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLength, SpecifiedValueParseError> {
    let ValueComponent::Token(token) = component else {
        return Err(unsupported_component_error(property, component));
    };

    match token {
        ValueToken::Dimension {
            span,
            kind,
            number,
            unit,
        } => {
            let number = number_value_from_parts(property, *span, *kind, number)?;
            let numeric_value = number.numeric_value();
            reject_negative_if_needed(property, numeric_value)?;

            let unit = resolve_text(property, unit)?.to_ascii_lowercase();
            if unit != "px" {
                return Err(error(
                    property,
                    SpecifiedValueParseErrorKind::UnsupportedLengthUnit,
                ));
            }

            Ok(SpecifiedLength {
                value: CssLengthValue::new(number, CssLengthUnit::Px),
            })
        }
        ValueToken::Number { span, kind, text } => {
            let number = number_value_from_parts(property, *span, *kind, text)?;
            let numeric_value = number.numeric_value();
            reject_negative_if_needed(property, numeric_value)?;
            if numeric_value != 0.0 {
                return Err(error(
                    property,
                    SpecifiedValueParseErrorKind::NonZeroUnitlessLength,
                ));
            }

            Ok(SpecifiedLength {
                value: CssLengthValue::new(number, CssLengthUnit::UnitlessZero),
            })
        }
        ValueToken::Ident { .. } => Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedKeyword,
        )),
        _ => Err(unsupported_component_error(property, component)),
    }
}

fn reject_negative_if_needed(
    property: PropertyId,
    numeric_value: f64,
) -> Result<(), SpecifiedValueParseError> {
    match property.metadata().length_sign {
        PropertyLengthSignPolicy::NonNegative if numeric_value < 0.0 => Err(error(
            property,
            SpecifiedValueParseErrorKind::NegativeLengthNotAllowed,
        )),
        PropertyLengthSignPolicy::NonNegative | PropertyLengthSignPolicy::AllowNegative => Ok(()),
        PropertyLengthSignPolicy::NotLength => Err(error(
            property,
            SpecifiedValueParseErrorKind::InvariantViolation,
        )),
    }
}
