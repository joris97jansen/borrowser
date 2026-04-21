//! Property-aware parsed specified values for supported CSS properties.
//!
//! This module sits between the model-layer `DeclarationValue` syntax tree and
//! computed values. It validates authored values against the supported
//! property registry and produces typed specified values without performing
//! inheritance, initial/default fallback, layout-dependent resolution, or
//! computed-value normalization.

use crate::model::{DeclarationValue, ValueComponent, ValueText, ValueToken};
use crate::properties::{PropertyId, PropertyLengthSignPolicy, PropertySpecifiedValueKind};
use crate::syntax::CssSpan;

/// One parsed specified value for one supported property.
///
/// The `property` field is part of the value contract so cascade candidates
/// cannot accidentally pair a typed value with a different property id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedPropertyValue {
    property: PropertyId,
    value: SpecifiedValue,
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
/// boundaries such as `auto`, `none`, or unitless zero before computation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedValue {
    Color(SpecifiedColor),
    Display(SpecifiedDisplay),
    Length(SpecifiedLength),
    LengthOrAuto(SpecifiedLengthOrAuto),
    LengthOrNone(SpecifiedLengthOrNone),
}

impl SpecifiedValue {
    pub fn kind(&self) -> PropertySpecifiedValueKind {
        match self {
            Self::Color(_) => PropertySpecifiedValueKind::Color,
            Self::Display(_) => PropertySpecifiedValueKind::DisplayKeyword,
            Self::Length(_) => PropertySpecifiedValueKind::AbsoluteLength,
            Self::LengthOrAuto(_) => PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
            Self::LengthOrNone(_) => PropertySpecifiedValueKind::AbsoluteLengthOrNone,
        }
    }

    pub fn span(&self) -> CssSpan {
        match self {
            Self::Color(color) => color.span(),
            Self::Display(display) => display.span(),
            Self::Length(length) => length.span(),
            Self::LengthOrAuto(value) => value.span(),
            Self::LengthOrNone(value) => value.span(),
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::Color(color) => color.to_css_text(),
            Self::Display(display) => display.to_css_text().to_string(),
            Self::Length(length) => length.to_css_text(),
            Self::LengthOrAuto(value) => value.to_css_text(),
            Self::LengthOrNone(value) => value.to_css_text(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedColor {
    span: CssSpan,
    syntax: SpecifiedColorSyntax,
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
    digits: String,
    rgba: (u8, u8, u8, u8),
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
    span: CssSpan,
    keyword: SpecifiedDisplayKeyword,
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
    None,
}

impl SpecifiedDisplayKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Inline => "inline",
            Self::InlineBlock => "inline-block",
            Self::ListItem => "list-item",
            Self::None => "none",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedLength {
    span: CssSpan,
    number: String,
    numeric_value: SpecifiedLengthNumber,
    unit: SpecifiedLengthUnit,
}

#[derive(Clone, Copy, Debug)]
struct SpecifiedLengthNumber(f64);

impl SpecifiedLengthNumber {
    fn new(value: f64) -> Self {
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
pub enum SpecifiedLengthOrAuto {
    Length(SpecifiedLength),
    Auto { span: CssSpan },
}

impl SpecifiedLengthOrAuto {
    pub fn span(&self) -> CssSpan {
        match self {
            Self::Length(length) => length.span(),
            Self::Auto { span } => *span,
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::Length(length) => length.to_css_text(),
            Self::Auto { .. } => "auto".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecifiedLengthOrNone {
    Length(SpecifiedLength),
    None { span: CssSpan },
}

impl SpecifiedLengthOrNone {
    pub fn span(&self) -> CssSpan {
        match self {
            Self::Length(length) => length.span(),
            Self::None { span } => *span,
        }
    }

    pub fn to_css_text(&self) -> String {
        match self {
            Self::Length(length) => length.to_css_text(),
            Self::None { .. } => "none".to_string(),
        }
    }
}

/// Error returned when an authored declaration value cannot be parsed into the
/// property's specified-value representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedValueParseError {
    property: PropertyId,
    kind: SpecifiedValueParseErrorKind,
}

impl SpecifiedValueParseError {
    fn new(property: PropertyId, kind: SpecifiedValueParseErrorKind) -> Self {
        Self { property, kind }
    }

    pub fn property(&self) -> PropertyId {
        self.property
    }

    pub fn kind(&self) -> SpecifiedValueParseErrorKind {
        self.kind
    }
}

impl std::fmt::Display for SpecifiedValueParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "property '{}' value rejected: {}",
            self.property.name(),
            self.kind.as_debug_label()
        )
    }
}

impl std::error::Error for SpecifiedValueParseError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedValueParseErrorKind {
    EmptyValue,
    UnexpectedComponentCount,
    UnsupportedComponent,
    UnresolvedTokenText,
    UnsupportedColorKeyword,
    InvalidHexColor,
    UnsupportedDisplayKeyword,
    UnsupportedLengthUnit,
    InvalidLengthNumber,
    NonZeroUnitlessLength,
    NegativeLengthNotAllowed,
    UnsupportedKeyword,
}

impl SpecifiedValueParseErrorKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::EmptyValue => "empty-value",
            Self::UnexpectedComponentCount => "unexpected-component-count",
            Self::UnsupportedComponent => "unsupported-component",
            Self::UnresolvedTokenText => "unresolved-token-text",
            Self::UnsupportedColorKeyword => "unsupported-color-keyword",
            Self::InvalidHexColor => "invalid-hex-color",
            Self::UnsupportedDisplayKeyword => "unsupported-display-keyword",
            Self::UnsupportedLengthUnit => "unsupported-length-unit",
            Self::InvalidLengthNumber => "invalid-length-number",
            Self::NonZeroUnitlessLength => "non-zero-unitless-length",
            Self::NegativeLengthNotAllowed => "negative-length-not-allowed",
            Self::UnsupportedKeyword => "unsupported-keyword",
        }
    }
}

/// Parses one model-layer declaration value into a property-aware specified
/// value.
pub fn parse_specified_value(
    property: PropertyId,
    value: &DeclarationValue,
) -> Result<SpecifiedPropertyValue, SpecifiedValueParseError> {
    let component = sole_non_trivia_component(property, value)?;
    let specified = match property.metadata().specified_value {
        PropertySpecifiedValueKind::Color => {
            SpecifiedValue::Color(parse_color(property, component)?)
        }
        PropertySpecifiedValueKind::DisplayKeyword => {
            SpecifiedValue::Display(parse_display(property, component)?)
        }
        PropertySpecifiedValueKind::AbsoluteLength => {
            SpecifiedValue::Length(parse_length(property, component)?)
        }
        PropertySpecifiedValueKind::AbsoluteLengthOrAuto => {
            SpecifiedValue::LengthOrAuto(parse_length_or_auto(property, component)?)
        }
        PropertySpecifiedValueKind::AbsoluteLengthOrNone => {
            SpecifiedValue::LengthOrNone(parse_length_or_none(property, component)?)
        }
    };

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

fn sole_non_trivia_component(
    property: PropertyId,
    value: &DeclarationValue,
) -> Result<&ValueComponent, SpecifiedValueParseError> {
    // Current S3-supported properties all use one non-trivia component.
    // Multi-value shorthands, functions, and property-specific component
    // grammars should replace this gate when those value families are added.
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

fn parse_color(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedColor, SpecifiedValueParseError> {
    let ValueComponent::Token(token) = component else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        ));
    };

    let syntax = match token {
        ValueToken::Ident { text, .. } => {
            let keyword = resolve_text(property, text)?.to_ascii_lowercase();
            let Some(keyword) = parse_color_keyword(&keyword) else {
                return Err(error(
                    property,
                    SpecifiedValueParseErrorKind::UnsupportedColorKeyword,
                ));
            };
            SpecifiedColorSyntax::Keyword(keyword)
        }
        ValueToken::Hash { text, .. } => {
            let digits = resolve_text(property, text)?.to_ascii_lowercase();
            let rgba = parse_hex_color_digits(property, &digits)?;
            SpecifiedColorSyntax::Hex(SpecifiedHexColor { digits, rgba })
        }
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedComponent,
            ));
        }
    };

    Ok(SpecifiedColor {
        span: token.span(),
        syntax,
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
        _ => unreachable!("hex color digit length is validated above"),
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

fn parse_display(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedDisplay, SpecifiedValueParseError> {
    let Some((keyword, span)) = ident_keyword(property, component)? else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        ));
    };

    let keyword = match keyword.as_str() {
        "block" => SpecifiedDisplayKeyword::Block,
        "inline" => SpecifiedDisplayKeyword::Inline,
        "inline-block" => SpecifiedDisplayKeyword::InlineBlock,
        "list-item" => SpecifiedDisplayKeyword::ListItem,
        "none" => SpecifiedDisplayKeyword::None,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedDisplayKeyword,
            ));
        }
    };

    Ok(SpecifiedDisplay { span, keyword })
}

fn parse_length_or_auto(
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

fn parse_length_or_none(
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

fn parse_length(
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
        PropertyLengthSignPolicy::NotLength => unreachable!(
            "property '{}' reached length parsing without accepting length specified values",
            property.name()
        ),
    }
}

fn ident_keyword(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<Option<(String, CssSpan)>, SpecifiedValueParseError> {
    let ValueComponent::Token(ValueToken::Ident { span, text }) = component else {
        return Ok(None);
    };

    Ok(Some((
        resolve_text(property, text)?.to_ascii_lowercase(),
        *span,
    )))
}

fn parse_number_text(
    property: PropertyId,
    text: &ValueText,
) -> Result<(String, f64), SpecifiedValueParseError> {
    let number = resolve_text(property, text)?.to_string();
    let numeric_value = number
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| error(property, SpecifiedValueParseErrorKind::InvalidLengthNumber))?;

    Ok((number, numeric_value))
}

fn resolve_text(property: PropertyId, text: &ValueText) -> Result<&str, SpecifiedValueParseError> {
    text.text
        .as_deref()
        .ok_or_else(|| error(property, SpecifiedValueParseErrorKind::UnresolvedTokenText))
}

fn error(property: PropertyId, kind: SpecifiedValueParseErrorKind) -> SpecifiedValueParseError {
    SpecifiedValueParseError::new(property, kind)
}

#[cfg(test)]
mod tests {
    use super::{
        SpecifiedColorKeyword, SpecifiedColorSyntax, SpecifiedDisplayKeyword, SpecifiedLengthUnit,
        SpecifiedValue, SpecifiedValueParseErrorKind, parse_specified_value,
    };
    use crate::{
        ParseOptions, PropertyId, PropertySpecifiedValueKind, Rule, parse_stylesheet_with_options,
        property_registry,
    };

    fn declaration_value(css_declaration: &str) -> crate::DeclarationValue {
        let parse = parse_stylesheet_with_options(
            &format!("div {{ {css_declaration}; }}"),
            &ParseOptions::stylesheet(),
        );
        let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
            panic!("expected style rule");
        };

        rule.declarations.declarations[0].value.clone()
    }

    fn parse(property: PropertyId, css_declaration: &str) -> super::SpecifiedPropertyValue {
        parse_specified_value(property, &declaration_value(css_declaration))
            .unwrap_or_else(|error| panic!("failed to parse {css_declaration:?}: {error}"))
    }

    fn parse_error(property: PropertyId, css_declaration: &str) -> SpecifiedValueParseErrorKind {
        parse_specified_value(property, &declaration_value(css_declaration))
            .expect_err("specified value must be rejected")
            .kind()
    }

    #[test]
    fn parses_representative_property_aware_specified_values() {
        let color = parse(PropertyId::Color, "color: RED");
        assert_eq!(color.property(), PropertyId::Color);
        assert_eq!(color.kind(), PropertySpecifiedValueKind::Color);
        assert_eq!(color.to_css_text(), "red");
        let SpecifiedValue::Color(color) = color.value() else {
            panic!("expected color");
        };
        assert_eq!(
            color.syntax(),
            &SpecifiedColorSyntax::Keyword(SpecifiedColorKeyword::Red)
        );

        let background = parse(PropertyId::BackgroundColor, "background-color: #Aa00FF");
        assert_eq!(background.to_css_text(), "#aa00ff");
        let SpecifiedValue::Color(background) = background.value() else {
            panic!("expected background color");
        };
        let SpecifiedColorSyntax::Hex(hex) = background.syntax() else {
            panic!("expected hex color");
        };
        assert_eq!(hex.digits(), "aa00ff");
        assert_eq!(hex.rgba(), (170, 0, 255, 255));

        let display = parse(PropertyId::Display, "display: inline-block");
        let SpecifiedValue::Display(display) = display.value() else {
            panic!("expected display");
        };
        assert_eq!(display.keyword(), SpecifiedDisplayKeyword::InlineBlock);
        assert_eq!(display.to_css_text(), "inline-block");

        let margin = parse(PropertyId::MarginLeft, "margin-left: -4.5px");
        let SpecifiedValue::Length(length) = margin.value() else {
            panic!("expected length");
        };
        assert_eq!(length.number(), "-4.5");
        assert_eq!(length.numeric_value(), -4.5);
        assert_eq!(length.unit(), SpecifiedLengthUnit::Px);
        assert_eq!(margin.to_css_text(), "-4.5px");

        let width = parse(PropertyId::Width, "width: auto");
        assert_eq!(width.to_css_text(), "auto");

        let max_width = parse(PropertyId::MaxWidth, "max-width: none");
        assert_eq!(max_width.to_css_text(), "none");
    }

    #[test]
    fn parses_unitless_zero_as_specified_length_without_computing_it() {
        let width = parse(PropertyId::Width, "width: 0");
        let SpecifiedValue::LengthOrAuto(super::SpecifiedLengthOrAuto::Length(length)) =
            width.value()
        else {
            panic!("expected length-or-auto length");
        };

        assert_eq!(length.number(), "0");
        assert_eq!(length.numeric_value(), 0.0);
        assert_eq!(length.unit(), SpecifiedLengthUnit::UnitlessZero);
        assert_eq!(width.to_css_text(), "0");
    }

    #[test]
    fn rejects_values_that_do_not_match_the_property_specified_shape() {
        assert_eq!(
            parse_error(PropertyId::Display, "display: grid"),
            SpecifiedValueParseErrorKind::UnsupportedDisplayKeyword
        );
        assert_eq!(
            parse_error(PropertyId::Width, "width: none"),
            SpecifiedValueParseErrorKind::UnsupportedKeyword
        );
        assert_eq!(
            parse_error(PropertyId::MaxWidth, "max-width: auto"),
            SpecifiedValueParseErrorKind::UnsupportedKeyword
        );
        assert_eq!(
            parse_error(PropertyId::Width, "width: -1px"),
            SpecifiedValueParseErrorKind::NegativeLengthNotAllowed
        );
        assert_eq!(
            parse_error(PropertyId::Width, "width: 1"),
            SpecifiedValueParseErrorKind::NonZeroUnitlessLength
        );
        assert_eq!(
            parse_error(PropertyId::Color, "color: rgb(1, 2, 3)"),
            SpecifiedValueParseErrorKind::UnsupportedComponent
        );
        assert_eq!(
            parse_error(PropertyId::Color, "color: #abcd"),
            SpecifiedValueParseErrorKind::InvalidHexColor
        );
        assert_eq!(
            parse_error(PropertyId::Color, "color: red blue"),
            SpecifiedValueParseErrorKind::UnexpectedComponentCount
        );
    }

    #[test]
    fn supported_property_metadata_matches_emitted_specified_value_kinds() {
        let representative = [
            (PropertyId::BackgroundColor, "background-color: transparent"),
            (PropertyId::Color, "color: black"),
            (PropertyId::Display, "display: block"),
            (PropertyId::FontSize, "font-size: 16px"),
            (PropertyId::Height, "height: auto"),
            (PropertyId::MarginBottom, "margin-bottom: 1px"),
            (PropertyId::MarginLeft, "margin-left: 1px"),
            (PropertyId::MarginRight, "margin-right: 1px"),
            (PropertyId::MarginTop, "margin-top: 1px"),
            (PropertyId::MaxWidth, "max-width: none"),
            (PropertyId::MinWidth, "min-width: auto"),
            (PropertyId::PaddingBottom, "padding-bottom: 1px"),
            (PropertyId::PaddingLeft, "padding-left: 1px"),
            (PropertyId::PaddingRight, "padding-right: 1px"),
            (PropertyId::PaddingTop, "padding-top: 1px"),
            (PropertyId::Width, "width: auto"),
        ];

        for property in property_registry().ids() {
            let (_, declaration) = representative
                .iter()
                .copied()
                .find(|(candidate, _)| *candidate == property)
                .unwrap_or_else(|| panic!("missing representative for {}", property.name()));
            let value = parse(property, declaration);
            assert_eq!(
                value.kind(),
                property.metadata().specified_value,
                "{}",
                property.name()
            );
        }
    }
}
