use crate::{
    model::{ValueComponent, ValueToken},
    properties::PropertyId,
    values::{CssColorValue, CssHexColor},
};

use super::{
    core::{keyword_value, resolve_text, unsupported_component_error},
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    value::{SpecifiedColor, SpecifiedColorKeyword, SpecifiedColorSyntax},
};

pub(super) fn parse_color(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedColor, SpecifiedValueParseError> {
    if let Some(keyword) = keyword_value(property, component)? {
        let Some(keyword_value) = parse_color_keyword(keyword.canonical()) else {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedColorKeyword,
            ));
        };

        return Ok(SpecifiedColor {
            value: CssColorValue::new(keyword.span(), SpecifiedColorSyntax::Keyword(keyword_value)),
        });
    }

    let ValueComponent::Token(token) = component else {
        return Err(unsupported_component_error(property, component));
    };

    let syntax = match token {
        ValueToken::Hash { text, .. } => {
            let digits = resolve_text(property, text)?.to_ascii_lowercase();
            let rgba = parse_hex_color_digits(property, &digits)?;
            SpecifiedColorSyntax::Hex(CssHexColor::new(digits, rgba))
        }
        _ => {
            return Err(unsupported_component_error(property, component));
        }
    };

    Ok(SpecifiedColor {
        value: CssColorValue::new(token.span(), syntax),
    })
}

fn parse_color_keyword(keyword: &str) -> Option<SpecifiedColorKeyword> {
    match keyword {
        "black" => Some(SpecifiedColorKeyword::Black),
        "blue" => Some(SpecifiedColorKeyword::Blue),
        "cyan" => Some(SpecifiedColorKeyword::Cyan),
        "gray" | "grey" => Some(SpecifiedColorKeyword::Gray),
        "green" => Some(SpecifiedColorKeyword::Green),
        "magenta" => Some(SpecifiedColorKeyword::Magenta),
        "maroon" => Some(SpecifiedColorKeyword::Maroon),
        "navy" => Some(SpecifiedColorKeyword::Navy),
        "olive" => Some(SpecifiedColorKeyword::Olive),
        "purple" => Some(SpecifiedColorKeyword::Purple),
        "red" => Some(SpecifiedColorKeyword::Red),
        "silver" => Some(SpecifiedColorKeyword::Silver),
        "teal" => Some(SpecifiedColorKeyword::Teal),
        "transparent" => Some(SpecifiedColorKeyword::Transparent),
        "white" => Some(SpecifiedColorKeyword::White),
        "yellow" => Some(SpecifiedColorKeyword::Yellow),
        _ => None,
    }
}

fn parse_hex_color_digits(
    property: PropertyId,
    digits: &str,
) -> Result<(u8, u8, u8, u8), SpecifiedValueParseError> {
    if !matches!(digits.len(), 3 | 6) || !digits.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::InvalidHexColor,
        ));
    }

    let expanded = match digits.len() {
        3 => {
            let mut expanded = String::with_capacity(6);
            for ch in digits.chars() {
                expanded.push(ch);
                expanded.push(ch);
            }
            expanded
        }
        6 => digits.to_string(),
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::InvariantViolation,
            ));
        }
    };

    let parse_channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&expanded[range], 16)
            .map_err(|_| error(property, SpecifiedValueParseErrorKind::InvalidHexColor))
    };

    Ok((
        parse_channel(0..2)?,
        parse_channel(2..4)?,
        parse_channel(4..6)?,
        255,
    ))
}
