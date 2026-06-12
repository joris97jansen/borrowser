use crate::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId,
    specified::{SpecifiedPropertyValue, SpecifiedValue},
    values::{
        BorderStyle, Display, Length, LengthPercentage, OutlineStyle, Overflow, Position,
        TextDecorationLine,
    },
};

use super::{
    format::{display_keyword, format_length},
    normalize::{
        normalize_border_style, normalize_color, normalize_display, normalize_length,
        normalize_length_percentage_or_auto, normalize_length_percentage_or_none,
        normalize_outline_style, normalize_text_decoration_line,
    },
};

/// Typed computed-value surface for the current supported property subset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComputedValue {
    BorderStyle(BorderStyle),
    OutlineStyle(OutlineStyle),
    TextDecorationLine(TextDecorationLine),
    Color((u8, u8, u8, u8)),
    Display(Display),
    Overflow(Overflow),
    Position(Position),
    Length(Length),
    LengthPercentageOrAuto(Option<LengthPercentage>),
    LengthPercentageOrNone(Option<LengthPercentage>),
}

impl ComputedValue {
    pub fn discriminant(self) -> ComputedValueDiscriminant {
        match self {
            Self::BorderStyle(_) => ComputedValueDiscriminant::BorderStyle,
            Self::OutlineStyle(_) => ComputedValueDiscriminant::OutlineStyle,
            Self::TextDecorationLine(_) => ComputedValueDiscriminant::TextDecorationLine,
            Self::Color(_) => ComputedValueDiscriminant::Color,
            Self::Display(_) => ComputedValueDiscriminant::Display,
            Self::Overflow(_) => ComputedValueDiscriminant::Overflow,
            Self::Position(_) => ComputedValueDiscriminant::Position,
            Self::Length(_) => ComputedValueDiscriminant::Length,
            Self::LengthPercentageOrAuto(_) => ComputedValueDiscriminant::LengthPercentageOrAuto,
            Self::LengthPercentageOrNone(_) => ComputedValueDiscriminant::LengthPercentageOrNone,
        }
    }

    pub fn from_initial(property: PropertyId) -> Self {
        match property.initial_value() {
            InitialStyleValue::BorderStyleNone => Self::BorderStyle(BorderStyle::None),
            InitialStyleValue::OutlineStyleNone => Self::OutlineStyle(OutlineStyle::None),
            InitialStyleValue::TextDecorationLineNone => {
                Self::TextDecorationLine(TextDecorationLine::None)
            }
            InitialStyleValue::ColorBlack => Self::Color((0, 0, 0, 255)),
            InitialStyleValue::TransparentColor => Self::Color((0, 0, 0, 0)),
            InitialStyleValue::DisplayInline => Self::Display(Display::Inline),
            InitialStyleValue::FontSizePx16 => Self::Length(Length::Px(16.0)),
            InitialStyleValue::ZeroPx => Self::Length(Length::Px(0.0)),
            InitialStyleValue::AutoKeyword => Self::LengthPercentageOrAuto(None),
            InitialStyleValue::NoneKeyword => Self::LengthPercentageOrNone(None),
            InitialStyleValue::OverflowVisible => Self::Overflow(Overflow::Visible),
            InitialStyleValue::PositionStatic => Self::Position(Position::Static),
        }
    }

    /// Normalizes a property-aware specified value into its runtime computed
    /// value representation.
    ///
    /// This performs canonical value conversion only. It does not apply
    /// inheritance, initial/default fallback, layout-dependent resolution, or
    /// UA/HTML bridge defaults.
    pub fn from_specified(
        specified: &SpecifiedPropertyValue,
    ) -> Result<Self, ComputedValueNormalizationError> {
        let property = specified.property();
        let value = match specified.value() {
            SpecifiedValue::BorderStyle(border_style) => {
                Self::BorderStyle(normalize_border_style(border_style.keyword()))
            }
            SpecifiedValue::OutlineStyle(outline_style) => {
                Self::OutlineStyle(normalize_outline_style(outline_style.keyword()))
            }
            SpecifiedValue::TextDecorationLine(text_decoration_line) => Self::TextDecorationLine(
                normalize_text_decoration_line(text_decoration_line.keyword()),
            ),
            SpecifiedValue::Color(color) => Self::Color(normalize_color(color)),
            SpecifiedValue::Display(display) => Self::Display(normalize_display(display.keyword())),
            SpecifiedValue::Overflow(overflow) => {
                Self::Overflow(normalize_overflow(overflow.keyword()))
            }
            SpecifiedValue::Position(position) => {
                Self::Position(normalize_position(position.keyword()))
            }
            SpecifiedValue::Length(length) => Self::Length(normalize_length(property, length)?),
            SpecifiedValue::LengthPercentageOrAuto(value) => {
                Self::LengthPercentageOrAuto(normalize_length_percentage_or_auto(property, value)?)
            }
            SpecifiedValue::LengthPercentageOrNone(value) => {
                Self::LengthPercentageOrNone(normalize_length_percentage_or_none(property, value)?)
            }
        };

        let expected = property.metadata().computed_value;
        let actual = value.discriminant();
        if actual != computed_value_discriminant(expected) {
            return Err(ComputedValueNormalizationError::new(
                property,
                ComputedValueNormalizationErrorKind::ValueKindMismatch { expected, actual },
            ));
        }

        Ok(value)
    }

    /// Stable one-line label for computed-value debug output.
    ///
    /// This is a public regression/debug contract. Do not replace it with
    /// derived formatting or parser-facing text.
    pub fn to_debug_label(self) -> String {
        match self {
            Self::BorderStyle(style) => border_style_keyword(style).to_string(),
            Self::OutlineStyle(style) => outline_style_keyword(style).to_string(),
            Self::TextDecorationLine(line) => text_decoration_line_keyword(line).to_string(),
            Self::Color((r, g, b, a)) => format!("rgba({r}, {g}, {b}, {a})"),
            Self::Display(display) => display_keyword(display).to_string(),
            Self::Overflow(overflow) => overflow_keyword(overflow).to_string(),
            Self::Position(position) => position_keyword(position).to_string(),
            Self::Length(length) => format_length(length),
            Self::LengthPercentageOrAuto(Some(value)) => format_length_percentage(value),
            Self::LengthPercentageOrAuto(None) => "auto".to_string(),
            Self::LengthPercentageOrNone(Some(value)) => format_length_percentage(value),
            Self::LengthPercentageOrNone(None) => "none".to_string(),
        }
    }
}

/// Error returned when a parsed specified value cannot be normalized into the
/// computed-value contract for its property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedValueNormalizationError {
    property: PropertyId,
    kind: ComputedValueNormalizationErrorKind,
}

impl ComputedValueNormalizationError {
    pub(super) fn new(property: PropertyId, kind: ComputedValueNormalizationErrorKind) -> Self {
        Self { property, kind }
    }

    pub fn property(&self) -> PropertyId {
        self.property
    }

    pub fn kind(&self) -> ComputedValueNormalizationErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ComputedValueNormalizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "property '{}' specified value could not be normalized: {}",
            self.property.name(),
            self.kind.as_debug_label()
        )
    }
}

impl std::error::Error for ComputedValueNormalizationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedValueNormalizationErrorKind {
    LengthOutOfRange,
    ValueKindMismatch {
        expected: PropertyComputedValueKind,
        actual: ComputedValueDiscriminant,
    },
}

impl ComputedValueNormalizationErrorKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::LengthOutOfRange => "length-out-of-range",
            Self::ValueKindMismatch { .. } => "value-kind-mismatch",
        }
    }
}

/// Runtime-discriminant for `ComputedValue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedValueDiscriminant {
    BorderStyle,
    OutlineStyle,
    TextDecorationLine,
    Color,
    Display,
    Overflow,
    Position,
    Length,
    LengthPercentageOrAuto,
    LengthPercentageOrNone,
}

impl ComputedValueDiscriminant {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::BorderStyle => "border-style",
            Self::OutlineStyle => "outline-style",
            Self::TextDecorationLine => "text-decoration-line",
            Self::Color => "color",
            Self::Display => "display",
            Self::Overflow => "overflow",
            Self::Position => "position",
            Self::Length => "length",
            Self::LengthPercentageOrAuto => "length-percentage-or-auto",
            Self::LengthPercentageOrNone => "length-percentage-or-none",
        }
    }
}

pub fn normalize_specified_value(
    specified: &SpecifiedPropertyValue,
) -> Result<ComputedValue, ComputedValueNormalizationError> {
    ComputedValue::from_specified(specified)
}

pub(super) fn computed_value_discriminant(
    kind: PropertyComputedValueKind,
) -> ComputedValueDiscriminant {
    match kind {
        PropertyComputedValueKind::BorderStyleKeyword => ComputedValueDiscriminant::BorderStyle,
        PropertyComputedValueKind::OutlineStyleKeyword => ComputedValueDiscriminant::OutlineStyle,
        PropertyComputedValueKind::TextDecorationLineKeyword => {
            ComputedValueDiscriminant::TextDecorationLine
        }
        PropertyComputedValueKind::AbsoluteColor => ComputedValueDiscriminant::Color,
        PropertyComputedValueKind::DisplayKeyword => ComputedValueDiscriminant::Display,
        PropertyComputedValueKind::OverflowKeyword => ComputedValueDiscriminant::Overflow,
        PropertyComputedValueKind::PositionKeyword => ComputedValueDiscriminant::Position,
        PropertyComputedValueKind::AbsoluteLength => ComputedValueDiscriminant::Length,
        PropertyComputedValueKind::LengthPercentageOrAuto => {
            ComputedValueDiscriminant::LengthPercentageOrAuto
        }
        PropertyComputedValueKind::LengthPercentageOrNone => {
            ComputedValueDiscriminant::LengthPercentageOrNone
        }
    }
}

fn border_style_keyword(style: BorderStyle) -> &'static str {
    match style {
        BorderStyle::None => "none",
        BorderStyle::Solid => "solid",
    }
}

fn outline_style_keyword(style: OutlineStyle) -> &'static str {
    match style {
        OutlineStyle::None => "none",
        OutlineStyle::Solid => "solid",
    }
}

fn text_decoration_line_keyword(line: TextDecorationLine) -> &'static str {
    match line {
        TextDecorationLine::None => "none",
        TextDecorationLine::Underline => "underline",
    }
}

fn normalize_position(keyword: crate::SpecifiedPositionKeyword) -> Position {
    match keyword {
        crate::SpecifiedPositionKeyword::Static => Position::Static,
        crate::SpecifiedPositionKeyword::Relative => Position::Relative,
        crate::SpecifiedPositionKeyword::Absolute => Position::Absolute,
        crate::SpecifiedPositionKeyword::Fixed => Position::Fixed,
        crate::SpecifiedPositionKeyword::Sticky => Position::Sticky,
    }
}

fn normalize_overflow(keyword: crate::SpecifiedOverflowKeyword) -> Overflow {
    match keyword {
        crate::SpecifiedOverflowKeyword::Visible => Overflow::Visible,
        crate::SpecifiedOverflowKeyword::Hidden => Overflow::Hidden,
        crate::SpecifiedOverflowKeyword::Clip => Overflow::Clip,
        crate::SpecifiedOverflowKeyword::Scroll => Overflow::Scroll,
        crate::SpecifiedOverflowKeyword::Auto => Overflow::Auto,
    }
}

fn overflow_keyword(overflow: Overflow) -> &'static str {
    match overflow {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Clip => "clip",
        Overflow::Scroll => "scroll",
        Overflow::Auto => "auto",
    }
}

fn position_keyword(position: Position) -> &'static str {
    match position {
        Position::Static => "static",
        Position::Relative => "relative",
        Position::Absolute => "absolute",
        Position::Fixed => "fixed",
        Position::Sticky => "sticky",
    }
}

fn format_length_percentage(value: LengthPercentage) -> String {
    match value {
        LengthPercentage::Length(length) => format_length(length),
        LengthPercentage::Percentage(percentage) => format!("{}%", percentage.percent()),
    }
}
