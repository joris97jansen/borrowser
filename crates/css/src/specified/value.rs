use crate::{
    model::DeclarationValue,
    properties::{PropertyId, PropertySpecifiedValueKind},
    syntax::CssSpan,
};

use super::{error::SpecifiedValueParseError, parse::parse_specified_value};

/// One parsed specified value for one supported property.
///
/// The `property` field is part of the value contract so cascade candidates
/// cannot accidentally pair a typed value with a different property id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedPropertyValue {
    pub(super) property: PropertyId,
    pub(super) value: SpecifiedValue,
}

impl SpecifiedPropertyValue {
    #[cfg(test)]
    pub(crate) fn from_parts_for_test(property: PropertyId, value: SpecifiedValue) -> Self {
        Self { property, value }
    }

    pub fn parse(
        property: PropertyId,
        value: &DeclarationValue,
    ) -> Result<Self, SpecifiedValueParseError> {
        parse_specified_value(property, value)
    }

    pub fn property(&self) -> PropertyId {
        self.property
    }

    pub fn value(&self) -> &SpecifiedValue {
        &self.value
    }

    pub fn kind(&self) -> PropertySpecifiedValueKind {
        self.value.kind()
    }

    pub fn to_css_text(&self) -> String {
        self.value.to_css_text()
    }
}

/// Typed specified-value variants for the current supported property subset.
///
/// These variants intentionally mirror `PropertySpecifiedValueKind`, not
/// `ComputedValue`: a specified value may still contain authored keyword
/// boundaries such as `auto`, `none`, unitless zero, or unresolved percentages
/// before computation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedValue {
    BorderStyle(SpecifiedBorderStyle),
    OutlineStyle(SpecifiedOutlineStyle),
    Color(SpecifiedColor),
    Display(SpecifiedDisplay),
    Overflow(SpecifiedOverflow),
    Position(SpecifiedPosition),
    Length(SpecifiedLength),
    LengthPercentageOrAuto(SpecifiedLengthPercentageOrAuto),
    LengthPercentageOrNone(SpecifiedLengthPercentageOrNone),
}

impl SpecifiedValue {
    pub fn kind(&self) -> PropertySpecifiedValueKind {
        match self {
            Self::BorderStyle(_) => PropertySpecifiedValueKind::BorderStyleKeyword,
            Self::OutlineStyle(_) => PropertySpecifiedValueKind::OutlineStyleKeyword,
            Self::Color(_) => PropertySpecifiedValueKind::Color,
            Self::Display(_) => PropertySpecifiedValueKind::DisplayKeyword,
            Self::Overflow(_) => PropertySpecifiedValueKind::OverflowKeyword,
            Self::Position(_) => PropertySpecifiedValueKind::PositionKeyword,
            Self::Length(_) => PropertySpecifiedValueKind::AbsoluteLength,
            Self::LengthPercentageOrAuto(_) => PropertySpecifiedValueKind::LengthPercentageOrAuto,
            Self::LengthPercentageOrNone(_) => PropertySpecifiedValueKind::LengthPercentageOrNone,
        }
    }

    pub fn span(&self) -> CssSpan {
        match self {
            Self::BorderStyle(border_style) => border_style.span(),
            Self::OutlineStyle(outline_style) => outline_style.span(),
            Self::Color(color) => color.span(),
            Self::Display(display) => display.span(),
            Self::Overflow(overflow) => overflow.span(),
            Self::Position(position) => position.span(),
            Self::Length(length) => length.span(),
            Self::LengthPercentageOrAuto(value) => value.span(),
            Self::LengthPercentageOrNone(value) => value.span(),
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::BorderStyle(border_style) => border_style.to_css_text().to_string(),
            Self::OutlineStyle(outline_style) => outline_style.to_css_text().to_string(),
            Self::Color(color) => color.to_css_text(),
            Self::Display(display) => display.to_css_text().to_string(),
            Self::Overflow(overflow) => overflow.to_css_text().to_string(),
            Self::Position(position) => position.to_css_text().to_string(),
            Self::Length(length) => length.to_css_text(),
            Self::LengthPercentageOrAuto(value) => value.to_css_text(),
            Self::LengthPercentageOrNone(value) => value.to_css_text(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedBorderStyle {
    pub(super) span: CssSpan,
    pub(super) keyword: SpecifiedBorderStyleKeyword,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedBorderStyleKeyword {
    None,
    Solid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedOutlineStyle {
    pub(super) span: CssSpan,
    pub(super) keyword: SpecifiedOutlineStyleKeyword,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedOutlineStyleKeyword {
    None,
    Solid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedColor {
    pub(super) span: CssSpan,
    pub(super) syntax: SpecifiedColorSyntax,
}

impl SpecifiedColor {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn syntax(&self) -> &SpecifiedColorSyntax {
        &self.syntax
    }

    pub fn to_css_text(&self) -> String {
        match &self.syntax {
            SpecifiedColorSyntax::Keyword(keyword) => keyword.as_css_keyword().to_string(),
            SpecifiedColorSyntax::Hex(hex) => format!("#{}", hex.digits()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedColorSyntax {
    Keyword(SpecifiedColorKeyword),
    Hex(SpecifiedHexColor),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedColorKeyword {
    Black,
    Blue,
    Cyan,
    Gray,
    Green,
    Magenta,
    Maroon,
    Navy,
    Olive,
    Purple,
    Red,
    Silver,
    Teal,
    Transparent,
    White,
    Yellow,
}

impl SpecifiedColorKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Blue => "blue",
            Self::Cyan => "cyan",
            Self::Gray => "gray",
            Self::Green => "green",
            Self::Magenta => "magenta",
            Self::Maroon => "maroon",
            Self::Navy => "navy",
            Self::Olive => "olive",
            Self::Purple => "purple",
            Self::Red => "red",
            Self::Silver => "silver",
            Self::Teal => "teal",
            Self::Transparent => "transparent",
            Self::White => "white",
            Self::Yellow => "yellow",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedHexColor {
    pub(super) digits: String,
    pub(super) rgba: (u8, u8, u8, u8),
}

impl SpecifiedHexColor {
    pub fn digits(&self) -> &str {
        &self.digits
    }

    pub fn rgba(&self) -> (u8, u8, u8, u8) {
        self.rgba
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedDisplay {
    pub(super) span: CssSpan,
    pub(super) keyword: SpecifiedDisplayKeyword,
}

impl SpecifiedDisplay {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn keyword(&self) -> SpecifiedDisplayKeyword {
        self.keyword
    }

    pub fn to_css_text(&self) -> &'static str {
        self.keyword.as_css_keyword()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedDisplayKeyword {
    Block,
    Inline,
    InlineBlock,
    ListItem,
    Flex,
    None,
}

impl SpecifiedDisplayKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Inline => "inline",
            Self::InlineBlock => "inline-block",
            Self::ListItem => "list-item",
            Self::Flex => "flex",
            Self::None => "none",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedOverflow {
    pub(super) span: CssSpan,
    pub(super) keyword: SpecifiedOverflowKeyword,
}

impl SpecifiedOverflow {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn keyword(&self) -> SpecifiedOverflowKeyword {
        self.keyword
    }

    pub fn to_css_text(&self) -> &'static str {
        self.keyword.as_css_keyword()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedOverflowKeyword {
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

impl SpecifiedOverflowKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::Clip => "clip",
            Self::Scroll => "scroll",
            Self::Auto => "auto",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedPosition {
    pub(super) span: CssSpan,
    pub(super) keyword: SpecifiedPositionKeyword,
}

impl SpecifiedPosition {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn keyword(&self) -> SpecifiedPositionKeyword {
        self.keyword
    }

    pub fn to_css_text(&self) -> &'static str {
        self.keyword.as_css_keyword()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedPositionKeyword {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

impl SpecifiedPositionKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Relative => "relative",
            Self::Absolute => "absolute",
            Self::Fixed => "fixed",
            Self::Sticky => "sticky",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedLength {
    pub(super) span: CssSpan,
    pub(super) number: String,
    pub(super) numeric_value: SpecifiedLengthNumber,
    pub(super) unit: SpecifiedLengthUnit,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SpecifiedLengthNumber(f64);

impl SpecifiedLengthNumber {
    pub(super) fn new(value: f64) -> Self {
        Self(value)
    }

    fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for SpecifiedLengthNumber {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for SpecifiedLengthNumber {}

impl SpecifiedLength {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn number(&self) -> &str {
        &self.number
    }

    pub fn numeric_value(&self) -> f64 {
        self.numeric_value.get()
    }

    pub fn unit(&self) -> SpecifiedLengthUnit {
        self.unit
    }

    pub fn to_css_text(&self) -> String {
        match self.unit {
            SpecifiedLengthUnit::Px => format!("{}px", self.number),
            SpecifiedLengthUnit::UnitlessZero => self.number.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedLengthUnit {
    Px,
    UnitlessZero,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedPercentage {
    pub(super) span: CssSpan,
    pub(super) number: String,
    pub(super) numeric_value: SpecifiedPercentageNumber,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SpecifiedPercentageNumber(f64);

impl SpecifiedPercentageNumber {
    pub(super) fn new(value: f64) -> Self {
        Self(value)
    }

    fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for SpecifiedPercentageNumber {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for SpecifiedPercentageNumber {}

impl SpecifiedPercentage {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn number(&self) -> &str {
        &self.number
    }

    pub fn numeric_value(&self) -> f64 {
        self.numeric_value.get()
    }

    pub fn to_css_text(&self) -> String {
        format!("{}%", self.number)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedLengthPercentage {
    Length(SpecifiedLength),
    Percentage(SpecifiedPercentage),
}

impl SpecifiedLengthPercentage {
    pub fn span(&self) -> CssSpan {
        match self {
            Self::Length(length) => length.span(),
            Self::Percentage(percentage) => percentage.span(),
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::Length(length) => length.to_css_text(),
            Self::Percentage(percentage) => percentage.to_css_text(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedLengthPercentageOrAuto {
    LengthPercentage(SpecifiedLengthPercentage),
    Auto { span: CssSpan },
}

impl SpecifiedLengthPercentageOrAuto {
    pub fn span(&self) -> CssSpan {
        match self {
            Self::LengthPercentage(value) => value.span(),
            Self::Auto { span } => *span,
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::LengthPercentage(value) => value.to_css_text(),
            Self::Auto { .. } => "auto".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedLengthPercentageOrNone {
    LengthPercentage(SpecifiedLengthPercentage),
    None { span: CssSpan },
}

impl SpecifiedLengthPercentageOrNone {
    pub fn span(&self) -> CssSpan {
        match self {
            Self::LengthPercentage(value) => value.span(),
            Self::None { span } => *span,
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::LengthPercentage(value) => value.to_css_text(),
            Self::None { .. } => "none".to_string(),
        }
    }
}
