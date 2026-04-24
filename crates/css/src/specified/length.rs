use crate::{
    model::{ValueComponent, ValueToken},
    properties::{PropertyId, PropertyLengthSignPolicy},
};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::{ident_keyword, parse_number_text, resolve_text},
    value::{
        SpecifiedLength, SpecifiedLengthNumber, SpecifiedLengthOrAuto, SpecifiedLengthOrNone,
        SpecifiedLengthUnit,
    },
};

pub(super) fn parse_length_or_auto(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLengthOrAuto, SpecifiedValueParseError> {
    if let Some((keyword, span)) = ident_keyword(property, component)? {
        return if keyword == "auto" {
            Ok(SpecifiedLengthOrAuto::Auto { span })
        } else {
            Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ))
        };
    }

    parse_length(property, component).map(SpecifiedLengthOrAuto::Length)
}

pub(super) fn parse_length_or_none(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedLengthOrNone, SpecifiedValueParseError> {
    if let Some((keyword, span)) = ident_keyword(property, component)? {
        return if keyword == "none" {
            Ok(SpecifiedLengthOrNone::None { span })
        } else {
            Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ))
        };
    }

    parse_length(property, component).map(SpecifiedLengthOrNone::Length)
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
