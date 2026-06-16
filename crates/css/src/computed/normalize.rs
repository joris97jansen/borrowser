use crate::{
    PropertyId,
    specified::{
        SpecifiedColor, SpecifiedColorKeyword, SpecifiedColorSyntax, SpecifiedDisplayKeyword,
        SpecifiedLength, SpecifiedLengthPercentage, SpecifiedLengthPercentageOrAuto,
        SpecifiedLengthPercentageOrNone, SpecifiedPercentage, SpecifiedZIndexValue,
    },
    values::{
        BorderStyle, Display, Length, LengthPercentage, OutlineStyle, Percentage,
        TextDecorationLine, ZIndex,
    },
};

use super::value::{ComputedValueNormalizationError, ComputedValueNormalizationErrorKind};

pub(super) fn normalize_color(color: &SpecifiedColor) -> (u8, u8, u8, u8) {
    match color.syntax() {
        SpecifiedColorSyntax::Keyword(keyword) => normalize_color_keyword(*keyword),
        SpecifiedColorSyntax::Hex(hex) => hex.rgba(),
    }
}

pub(super) fn normalize_color_keyword(keyword: SpecifiedColorKeyword) -> (u8, u8, u8, u8) {
    match keyword {
        SpecifiedColorKeyword::Black => (0, 0, 0, 255),
        SpecifiedColorKeyword::Blue => (0, 0, 255, 255),
        SpecifiedColorKeyword::Cyan => (0, 255, 255, 255),
        SpecifiedColorKeyword::Gray => (128, 128, 128, 255),
        SpecifiedColorKeyword::Green => (0, 128, 0, 255),
        SpecifiedColorKeyword::Magenta => (255, 0, 255, 255),
        SpecifiedColorKeyword::Maroon => (128, 0, 0, 255),
        SpecifiedColorKeyword::Navy => (0, 0, 128, 255),
        SpecifiedColorKeyword::Olive => (128, 128, 0, 255),
        SpecifiedColorKeyword::Purple => (128, 0, 128, 255),
        SpecifiedColorKeyword::Red => (255, 0, 0, 255),
        SpecifiedColorKeyword::Silver => (192, 192, 192, 255),
        SpecifiedColorKeyword::Teal => (0, 128, 128, 255),
        SpecifiedColorKeyword::Transparent => (0, 0, 0, 0),
        SpecifiedColorKeyword::White => (255, 255, 255, 255),
        SpecifiedColorKeyword::Yellow => (255, 255, 0, 255),
    }
}

pub(super) fn normalize_display(display: SpecifiedDisplayKeyword) -> Display {
    match display {
        SpecifiedDisplayKeyword::Block => Display::Block,
        SpecifiedDisplayKeyword::Inline => Display::Inline,
        SpecifiedDisplayKeyword::InlineBlock => Display::InlineBlock,
        SpecifiedDisplayKeyword::ListItem => Display::ListItem,
        SpecifiedDisplayKeyword::Flex => Display::Flex,
        SpecifiedDisplayKeyword::None => Display::None,
    }
}

pub(super) fn normalize_border_style(style: crate::SpecifiedBorderStyleKeyword) -> BorderStyle {
    match style {
        crate::SpecifiedBorderStyleKeyword::None => BorderStyle::None,
        crate::SpecifiedBorderStyleKeyword::Solid => BorderStyle::Solid,
    }
}

pub(super) fn normalize_outline_style(style: crate::SpecifiedOutlineStyleKeyword) -> OutlineStyle {
    match style {
        crate::SpecifiedOutlineStyleKeyword::None => OutlineStyle::None,
        crate::SpecifiedOutlineStyleKeyword::Solid => OutlineStyle::Solid,
    }
}

pub(super) fn normalize_text_decoration_line(
    line: crate::SpecifiedTextDecorationLineKeyword,
) -> TextDecorationLine {
    match line {
        crate::SpecifiedTextDecorationLineKeyword::None => TextDecorationLine::None,
        crate::SpecifiedTextDecorationLineKeyword::Underline => TextDecorationLine::Underline,
    }
}

pub(super) fn normalize_z_index(value: SpecifiedZIndexValue) -> ZIndex {
    match value {
        SpecifiedZIndexValue::Auto => ZIndex::Auto,
        SpecifiedZIndexValue::Integer(value) => ZIndex::Integer(value),
    }
}

pub(super) fn normalize_length(
    property: PropertyId,
    length: &SpecifiedLength,
) -> Result<Length, ComputedValueNormalizationError> {
    let value = normalize_px_scalar(property, length.numeric_value())?;

    Ok(Length::Px(value))
}

pub(super) fn normalize_length_percentage_or_auto(
    property: PropertyId,
    value: &SpecifiedLengthPercentageOrAuto,
) -> Result<Option<LengthPercentage>, ComputedValueNormalizationError> {
    match value {
        SpecifiedLengthPercentageOrAuto::LengthPercentage(value) => {
            normalize_length_percentage(property, value).map(Some)
        }
        SpecifiedLengthPercentageOrAuto::Auto { .. } => Ok(None),
    }
}

pub(super) fn normalize_length_percentage_or_none(
    property: PropertyId,
    value: &SpecifiedLengthPercentageOrNone,
) -> Result<Option<LengthPercentage>, ComputedValueNormalizationError> {
    match value {
        SpecifiedLengthPercentageOrNone::LengthPercentage(value) => {
            normalize_length_percentage(property, value).map(Some)
        }
        SpecifiedLengthPercentageOrNone::None { .. } => Ok(None),
    }
}

pub(super) fn normalize_length_percentage(
    property: PropertyId,
    value: &SpecifiedLengthPercentage,
) -> Result<LengthPercentage, ComputedValueNormalizationError> {
    match value {
        SpecifiedLengthPercentage::Length(length) => {
            normalize_length(property, length).map(LengthPercentage::Length)
        }
        SpecifiedLengthPercentage::Percentage(percentage) => {
            normalize_percentage(property, percentage).map(LengthPercentage::Percentage)
        }
    }
}

pub(super) fn normalize_percentage(
    property: PropertyId,
    percentage: &SpecifiedPercentage,
) -> Result<Percentage, ComputedValueNormalizationError> {
    let percent = normalize_px_scalar(property, percentage.numeric_value())?;
    Percentage::from_percent(percent).ok_or_else(|| {
        ComputedValueNormalizationError::new(
            property,
            ComputedValueNormalizationErrorKind::LengthOutOfRange,
        )
    })
}

fn normalize_px_scalar(
    property: PropertyId,
    value: f64,
) -> Result<f32, ComputedValueNormalizationError> {
    debug_assert!(
        value.is_finite(),
        "specified length values must carry a finite validated scalar"
    );
    if !value.is_finite() {
        return Err(ComputedValueNormalizationError::new(
            property,
            ComputedValueNormalizationErrorKind::LengthOutOfRange,
        ));
    }
    if value == 0.0 {
        return Ok(0.0);
    }

    let value = value as f32;
    if !value.is_finite() {
        return Err(ComputedValueNormalizationError::new(
            property,
            ComputedValueNormalizationErrorKind::LengthOutOfRange,
        ));
    }

    Ok(value)
}
