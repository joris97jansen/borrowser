use crate::{
    model::{ValueComponent, ValueToken},
    properties::{PropertyId, PropertyLengthSignPolicy},
};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::{ident_keyword, parse_number_text, resolve_text},
    value::{
        SpecifiedLength, SpecifiedLengthNumber, SpecifiedLengthPercentage,
        SpecifiedLengthPercentageOrAuto, SpecifiedLengthPercentageOrNone, SpecifiedLengthUnit,
        SpecifiedPercentage, SpecifiedPercentageNumber,
    },
};

pub(super) fn parse_length_percentage_or_auto(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLengthPercentageOrAuto, SpecifiedValueParseError> {
    if let Some((keyword, span)) = ident_keyword(property, component)? {
        return if keyword == "auto" {
            Ok(SpecifiedLengthPercentageOrAuto::Auto { span })
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
    if let Some((keyword, span)) = ident_keyword(property, component)? {
        return if keyword == "none" {
            Ok(SpecifiedLengthPercentageOrNone::None { span })
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
        ValueComponent::Token(ValueToken::Percentage { span, text, .. }) => {
            let (number, numeric_value) = parse_number_text(property, text)?;
            reject_negative_if_needed(property, numeric_value)?;

            Ok(SpecifiedLengthPercentage::Percentage(SpecifiedPercentage {
                span: *span,
                number,
                numeric_value: SpecifiedPercentageNumber::new(numeric_value),
            }))
        }
        _ => parse_length(property, component).map(SpecifiedLengthPercentage::Length),
    }
}

pub(super) fn parse_length(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLength, SpecifiedValueParseError> {
    let ValueComponent::Token(token) = component else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        ));
    };

    match token {
        ValueToken::Dimension {
            span, number, unit, ..
        } => {
            let (number, numeric_value) = parse_number_text(property, number)?;
            reject_negative_if_needed(property, numeric_value)?;

            let unit = resolve_text(property, unit)?.to_ascii_lowercase();
            if unit != "px" {
                return Err(error(
                    property,
                    SpecifiedValueParseErrorKind::UnsupportedLengthUnit,
                ));
            }

            Ok(SpecifiedLength {
                span: *span,
                number,
                numeric_value: SpecifiedLengthNumber::new(numeric_value),
                unit: SpecifiedLengthUnit::Px,
            })
        }
        ValueToken::Number { span, text, .. } => {
            let (number, numeric_value) = parse_number_text(property, text)?;
            reject_negative_if_needed(property, numeric_value)?;
            if numeric_value != 0.0 {
                return Err(error(
                    property,
                    SpecifiedValueParseErrorKind::NonZeroUnitlessLength,
                ));
            }

            Ok(SpecifiedLength {
                span: *span,
                number,
                numeric_value: SpecifiedLengthNumber::new(numeric_value),
                unit: SpecifiedLengthUnit::UnitlessZero,
            })
        }
        ValueToken::Ident { .. } => Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedKeyword,
        )),
        _ => Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        )),
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
