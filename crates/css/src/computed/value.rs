use crate::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId,
    specified::{SpecifiedPropertyValue, SpecifiedValue},
    values::{Display, Length, LengthPercentage, Overflow},
};

use super::{
    format::{display_keyword, format_length},
    normalize::{
        normalize_color, normalize_display, normalize_length, normalize_length_percentage_or_auto,
        normalize_length_percentage_or_none,
    },
};

/// Typed computed-value surface for the current supported property subset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComputedValue {
    Color((u8, u8, u8, u8)),
    Display(Display),
    Overflow(Overflow),
    Length(Length),
    LengthPercentageOrAuto(Option<LengthPercentage>),
    LengthPercentageOrNone(Option<LengthPercentage>),
}

impl ComputedValue {
    pub fn discriminant(self) -> ComputedValueDiscriminant {
        match self {
            Self::Color(_) => ComputedValueDiscriminant::Color,
            Self::Display(_) => ComputedValueDiscriminant::Display,
            Self::Overflow(_) => ComputedValueDiscriminant::Overflow,
            Self::Length(_) => ComputedValueDiscriminant::Length,
            Self::LengthPercentageOrAuto(_) => ComputedValueDiscriminant::LengthPercentageOrAuto,
            Self::LengthPercentageOrNone(_) => ComputedValueDiscriminant::LengthPercentageOrNone,
        }
    }

    pub fn from_initial(property: PropertyId) -> Self {
        match property.initial_value() {
            InitialStyleValue::ColorBlack => Self::Color((0, 0, 0, 255)),
            InitialStyleValue::TransparentColor => Self::Color((0, 0, 0, 0)),
            InitialStyleValue::DisplayInline => Self::Display(Display::Inline),
            InitialStyleValue::FontSizePx16 => Self::Length(Length::Px(16.0)),
            InitialStyleValue::ZeroPx => Self::Length(Length::Px(0.0)),
            InitialStyleValue::AutoKeyword => Self::LengthPercentageOrAuto(None),
            InitialStyleValue::NoneKeyword => Self::LengthPercentageOrNone(None),
            InitialStyleValue::OverflowVisible => Self::Overflow(Overflow::Visible),
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
            SpecifiedValue::Color(color) => Self::Color(normalize_color(color)),
            SpecifiedValue::Display(display) => Self::Display(normalize_display(display.keyword())),
            SpecifiedValue::Overflow(overflow) => {
                Self::Overflow(normalize_overflow(overflow.keyword()))
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
            Self::Color((r, g, b, a)) => format!("rgba({r}, {g}, {b}, {a})"),
            Self::Display(display) => display_keyword(display).to_string(),
            Self::Overflow(overflow) => overflow_keyword(overflow).to_string(),
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
    Color,
    Display,
    Overflow,
    Length,
    LengthPercentageOrAuto,
    LengthPercentageOrNone,
}

impl ComputedValueDiscriminant {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Display => "display",
            Self::Overflow => "overflow",
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
        PropertyComputedValueKind::AbsoluteColor => ComputedValueDiscriminant::Color,
        PropertyComputedValueKind::DisplayKeyword => ComputedValueDiscriminant::Display,
        PropertyComputedValueKind::OverflowKeyword => ComputedValueDiscriminant::Overflow,
        PropertyComputedValueKind::AbsoluteLength => ComputedValueDiscriminant::Length,
        PropertyComputedValueKind::LengthPercentageOrAuto => {
            ComputedValueDiscriminant::LengthPercentageOrAuto
        }
        PropertyComputedValueKind::LengthPercentageOrNone => {
            ComputedValueDiscriminant::LengthPercentageOrNone
        }
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

fn format_length_percentage(value: LengthPercentage) -> String {
    match value {
        LengthPercentage::Length(length) => format_length(length),
        LengthPercentage::Percentage(percentage) => format!("{}%", percentage.percent()),
    }
}
