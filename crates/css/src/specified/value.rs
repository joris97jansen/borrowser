use crate::{
    model::DeclarationValue,
    properties::{PropertyId, PropertySpecifiedValueKind},
    syntax::CssSpan,
    values::{
        CssColorKeyword, CssColorSyntax, CssColorValue, CssHexColor, CssIntegerValue,
        CssLengthPercentageValue, CssLengthUnit, CssLengthValue, CssPercentageValue,
        CssWideKeywordValue,
    },
};

use super::{
    error::SpecifiedValueParseError,
    parse::{parse_specified_declaration_value, parse_specified_value},
};

/// One supported declaration value after property-aware specified parsing.
///
/// CSS-wide keywords are represented here as declaration values rather than
/// property-specific values because cascade winner resolution must compare
/// them before resolving their inheritance/defaulting behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedDeclarationValue {
    Property(SpecifiedPropertyValue),
    CssWideKeyword {
        property: PropertyId,
        value: CssWideKeywordValue,
    },
}

impl SpecifiedDeclarationValue {
    pub fn parse(
        property: PropertyId,
        value: &DeclarationValue,
    ) -> Result<Self, SpecifiedValueParseError> {
        parse_specified_declaration_value(property, value)
    }

    pub fn property(&self) -> PropertyId {
        match self {
            Self::Property(value) => value.property(),
            Self::CssWideKeyword { property, .. } => *property,
        }
    }

    pub fn property_value(&self) -> Option<&SpecifiedPropertyValue> {
        match self {
            Self::Property(value) => Some(value),
            Self::CssWideKeyword { .. } => None,
        }
    }

    pub fn css_wide_keyword(&self) -> Option<CssWideKeywordValue> {
        match self {
            Self::Property(_) => None,
            Self::CssWideKeyword { value, .. } => Some(*value),
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::Property(value) => value.to_css_text(),
            Self::CssWideKeyword { value, .. } => value.to_css_text().to_string(),
        }
    }
}

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
    TextDecorationLine(SpecifiedTextDecorationLine),
    Color(SpecifiedColor),
    Display(SpecifiedDisplay),
    Overflow(SpecifiedOverflow),
    Position(SpecifiedPosition),
    ZIndex(SpecifiedZIndex),
    Length(SpecifiedLength),
    LengthPercentageOrAuto(SpecifiedLengthPercentageOrAuto),
    LengthPercentageOrNone(SpecifiedLengthPercentageOrNone),
}

impl SpecifiedValue {
    pub fn kind(&self) -> PropertySpecifiedValueKind {
        match self {
            Self::BorderStyle(_) => PropertySpecifiedValueKind::BorderStyleKeyword,
            Self::OutlineStyle(_) => PropertySpecifiedValueKind::OutlineStyleKeyword,
            Self::TextDecorationLine(_) => PropertySpecifiedValueKind::TextDecorationLineKeyword,
            Self::Color(_) => PropertySpecifiedValueKind::Color,
            Self::Display(_) => PropertySpecifiedValueKind::DisplayKeyword,
            Self::Overflow(_) => PropertySpecifiedValueKind::OverflowKeyword,
            Self::Position(_) => PropertySpecifiedValueKind::PositionKeyword,
            Self::ZIndex(_) => PropertySpecifiedValueKind::ZIndex,
            Self::Length(_) => PropertySpecifiedValueKind::AbsoluteLength,
            Self::LengthPercentageOrAuto(_) => PropertySpecifiedValueKind::LengthPercentageOrAuto,
            Self::LengthPercentageOrNone(_) => PropertySpecifiedValueKind::LengthPercentageOrNone,
        }
    }

    pub fn span(&self) -> CssSpan {
        match self {
            Self::BorderStyle(border_style) => border_style.span(),
            Self::OutlineStyle(outline_style) => outline_style.span(),
            Self::TextDecorationLine(text_decoration_line) => text_decoration_line.span(),
            Self::Color(color) => color.span(),
            Self::Display(display) => display.span(),
            Self::Overflow(overflow) => overflow.span(),
            Self::Position(position) => position.span(),
            Self::ZIndex(z_index) => z_index.span(),
            Self::Length(length) => length.span(),
            Self::LengthPercentageOrAuto(value) => value.span(),
            Self::LengthPercentageOrNone(value) => value.span(),
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::BorderStyle(border_style) => border_style.to_css_text().to_string(),
            Self::OutlineStyle(outline_style) => outline_style.to_css_text().to_string(),
            Self::TextDecorationLine(text_decoration_line) => {
                text_decoration_line.to_css_text().to_string()
            }
            Self::Color(color) => color.to_css_text(),
            Self::Display(display) => display.to_css_text().to_string(),
            Self::Overflow(overflow) => overflow.to_css_text().to_string(),
            Self::Position(position) => position.to_css_text().to_string(),
            Self::ZIndex(z_index) => z_index.to_css_text(),
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
pub struct SpecifiedTextDecorationLine {
    pub(super) span: CssSpan,
    pub(super) keyword: SpecifiedTextDecorationLineKeyword,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedTextDecorationLineKeyword {
    None,
    Underline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedColor {
    pub(super) value: CssColorValue,
}

impl SpecifiedColor {
    pub fn span(&self) -> CssSpan {
        self.value.span()
    }

    pub fn syntax(&self) -> &SpecifiedColorSyntax {
        self.value.syntax()
    }

    pub fn to_css_text(&self) -> String {
        self.value.to_css_text()
    }
}

pub type SpecifiedColorSyntax = CssColorSyntax;
pub type SpecifiedColorKeyword = CssColorKeyword;
pub type SpecifiedHexColor = CssHexColor;

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
pub struct SpecifiedZIndex {
    pub(super) value: SpecifiedZIndexValue,
}

impl SpecifiedZIndex {
    pub fn span(&self) -> CssSpan {
        match &self.value {
            SpecifiedZIndexValue::Auto { span } => *span,
            SpecifiedZIndexValue::Integer(value) => value.span(),
        }
    }

    pub fn value(&self) -> &SpecifiedZIndexValue {
        &self.value
    }

    pub fn to_css_text(&self) -> String {
        match &self.value {
            SpecifiedZIndexValue::Auto { .. } => "auto".to_string(),
            SpecifiedZIndexValue::Integer(value) => value.to_css_text().to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedZIndexValue {
    Auto { span: CssSpan },
    Integer(CssIntegerValue),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedLength {
    pub(super) value: CssLengthValue,
}

impl SpecifiedLength {
    pub fn span(&self) -> CssSpan {
        self.value.span()
    }

    pub fn number(&self) -> &str {
        self.value.number()
    }

    pub fn numeric_value(&self) -> f64 {
        self.value.numeric_value()
    }

    pub fn unit(&self) -> SpecifiedLengthUnit {
        self.value.unit()
    }

    pub fn value(&self) -> &CssLengthValue {
        &self.value
    }

    pub fn to_css_text(&self) -> String {
        self.value.to_css_text()
    }
}

pub type SpecifiedLengthUnit = CssLengthUnit;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedPercentage {
    pub(super) value: CssPercentageValue,
}

impl SpecifiedPercentage {
    pub fn span(&self) -> CssSpan {
        self.value.span()
    }

    pub fn number(&self) -> &str {
        self.value.number()
    }

    pub fn numeric_value(&self) -> f64 {
        self.value.numeric_value()
    }

    pub fn value(&self) -> &CssPercentageValue {
        &self.value
    }

    pub fn to_css_text(&self) -> String {
        self.value.to_css_text()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedLengthPercentage {
    pub(super) value: CssLengthPercentageValue,
}

impl SpecifiedLengthPercentage {
    pub fn span(&self) -> CssSpan {
        self.value.span()
    }

    pub fn value(&self) -> &CssLengthPercentageValue {
        &self.value
    }

    pub fn to_css_text(&self) -> String {
        self.value.to_css_text()
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
