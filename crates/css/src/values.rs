use crate::syntax::{CssNumericKind, CssSpan};

/// CSS identifier or keyword value after model-layer token extraction.
///
/// This is a reusable CSS-owned primitive. It is not a syntax token and it is
/// not a computed value. Property parsers decide whether the canonical keyword
/// is supported for a specific property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssKeywordValue {
    span: CssSpan,
    canonical: String,
}

impl CssKeywordValue {
    pub fn new(span: CssSpan, canonical: String) -> Self {
        Self { span, canonical }
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn canonical(&self) -> &str {
        &self.canonical
    }

    pub fn to_css_text(&self) -> &str {
        &self.canonical
    }
}

/// Finite CSS number scalar.
///
/// The authored number representation remains separate from this scalar so
/// specified-value debug output can preserve deterministic CSS text while
/// computed-value normalization consumes validated numeric data.
#[derive(Clone, Copy, Debug)]
pub struct CssNumberScalar(f64);

impl CssNumberScalar {
    pub fn new(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for CssNumberScalar {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for CssNumberScalar {}

/// CSS numeric token value after model-layer token extraction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssNumberValue {
    span: CssSpan,
    repr: String,
    numeric_value: CssNumberScalar,
    kind: CssNumericKind,
}

impl CssNumberValue {
    pub fn new(
        span: CssSpan,
        repr: String,
        numeric_value: CssNumberScalar,
        kind: CssNumericKind,
    ) -> Self {
        Self {
            span,
            repr,
            numeric_value,
            kind,
        }
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn repr(&self) -> &str {
        &self.repr
    }

    pub fn numeric_value(&self) -> f64 {
        self.numeric_value.get()
    }

    pub fn kind(&self) -> CssNumericKind {
        self.kind
    }

    pub fn to_css_text(&self) -> &str {
        &self.repr
    }
}

/// CSS integer primitive after property-level integer range validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssIntegerValue {
    number: CssNumberValue,
    value: i32,
}

impl CssIntegerValue {
    pub fn new(number: CssNumberValue, value: i32) -> Self {
        Self { number, value }
    }

    pub fn span(&self) -> CssSpan {
        self.number.span()
    }

    pub fn number(&self) -> &CssNumberValue {
        &self.number
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn to_css_text(&self) -> &str {
        self.number.to_css_text()
    }
}

/// CSS length unit in the current supported core value subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssLengthUnit {
    Px,
    UnitlessZero,
}

/// CSS length primitive after unit and scalar validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssLengthValue {
    number: CssNumberValue,
    unit: CssLengthUnit,
}

impl CssLengthValue {
    pub fn new(number: CssNumberValue, unit: CssLengthUnit) -> Self {
        Self { number, unit }
    }

    pub fn span(&self) -> CssSpan {
        self.number.span()
    }

    pub fn number(&self) -> &str {
        self.number.repr()
    }

    pub fn numeric_value(&self) -> f64 {
        self.number.numeric_value()
    }

    pub fn number_value(&self) -> &CssNumberValue {
        &self.number
    }

    pub fn unit(&self) -> CssLengthUnit {
        self.unit
    }

    pub fn to_css_text(&self) -> String {
        match self.unit {
            CssLengthUnit::Px => format!("{}px", self.number.repr()),
            CssLengthUnit::UnitlessZero => self.number.repr().to_string(),
        }
    }
}

/// CSS percentage primitive represented by its authored percent scalar.
///
/// `50%` stores numeric value `50.0` here. Computed values convert this to the
/// runtime `Percentage` fraction only during computed-value normalization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssPercentageValue {
    number: CssNumberValue,
}

impl CssPercentageValue {
    pub fn new(number: CssNumberValue) -> Self {
        Self { number }
    }

    pub fn span(&self) -> CssSpan {
        self.number.span()
    }

    pub fn number(&self) -> &str {
        self.number.repr()
    }

    pub fn numeric_value(&self) -> f64 {
        self.number.numeric_value()
    }

    pub fn number_value(&self) -> &CssNumberValue {
        &self.number
    }

    pub fn to_css_text(&self) -> String {
        format!("{}%", self.number.repr())
    }
}

/// CSS `<length-percentage>` primitive for currently supported sizing inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssLengthPercentageValue {
    Length(CssLengthValue),
    Percentage(CssPercentageValue),
}

impl CssLengthPercentageValue {
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

/// Supported color keyword subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssColorKeyword {
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

impl CssColorKeyword {
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

/// Supported hex color primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssHexColor {
    digits: String,
    rgba: (u8, u8, u8, u8),
}

impl CssHexColor {
    pub fn new(digits: String, rgba: (u8, u8, u8, u8)) -> Self {
        Self { digits, rgba }
    }

    pub fn digits(&self) -> &str {
        &self.digits
    }

    pub fn rgba(&self) -> (u8, u8, u8, u8) {
        self.rgba
    }
}

/// Supported color syntaxes in Borrowser's current CSS value subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssColorSyntax {
    Keyword(CssColorKeyword),
    Hex(CssHexColor),
}

/// CSS color primitive after current-subset validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssColorValue {
    span: CssSpan,
    syntax: CssColorSyntax,
}

impl CssColorValue {
    pub fn new(span: CssSpan, syntax: CssColorSyntax) -> Self {
        Self { span, syntax }
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn syntax(&self) -> &CssColorSyntax {
        &self.syntax
    }

    pub fn to_css_text(&self) -> String {
        match &self.syntax {
            CssColorSyntax::Keyword(keyword) => keyword.as_css_keyword().to_string(),
            CssColorSyntax::Hex(hex) => format!("#{}", hex.digits()),
        }
    }
}

/// CSS URL primitive wrapper.
///
/// This is only an authored value wrapper. It performs no URL resolution, base
/// URL handling, origin handling, fetch integration, image loading, or network
/// behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssUrlValue {
    span: CssSpan,
    value: String,
}

impl CssUrlValue {
    pub fn new(span: CssSpan, value: String) -> Self {
        Self { span, value }
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// CSS string primitive wrapper after model-layer token extraction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssStringValue {
    span: CssSpan,
    value: String,
}

impl CssStringValue {
    pub fn new(span: CssSpan, value: String) -> Self {
        Self { span, value }
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// CSS function primitive wrapper for future property grammars.
///
/// AD2 current-property parsers reject functions deterministically before
/// constructing accepted specified values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssFunctionValue {
    span: CssSpan,
    name: String,
}

impl CssFunctionValue {
    pub fn new(span: CssSpan, name: String) -> Self {
        Self { span, name }
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// CSS Length value, currently only supports `px`,
/// but keep this extensible for `em`, `rem`, `pt`, etc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Px(f32),
}

/// CSS percentage value represented as a fraction.
///
/// `1.0` is 100%, `0.5` is 50%. Sign/range validity is property-specific and
/// enforced before a percentage reaches computed style.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Percentage(f32);

impl Percentage {
    pub fn from_fraction(value: f32) -> Option<Self> {
        if value.is_finite() {
            Some(Self(if value == 0.0 { 0.0 } else { value }))
        } else {
            None
        }
    }

    pub fn from_percent(value: f32) -> Option<Self> {
        Self::from_fraction(value / 100.0)
    }

    pub fn fraction(self) -> f32 {
        self.0
    }

    pub fn percent(self) -> f32 {
        self.0 * 100.0
    }
}

/// CSS `<length-percentage>` computed value.
///
/// Percentages remain unresolved at computed-value time. Layout resolves them
/// against the appropriate containing-size basis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthPercentage {
    Length(Length),
    Percentage(Percentage),
}

/// CSS `display` value. This will be expanded over time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    InlineBlock,
    ListItem,
    Flex,
    None,
}

/// CSS physical border style for the current supported subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Solid,
}

/// CSS outline style for the current supported rectangular outline subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineStyle {
    None,
    Solid,
}

/// CSS `text-decoration-line` value for the current supported text decoration subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDecorationLine {
    None,
    Underline,
}

/// CSS `overflow` keyword for the current supported single-axis shorthand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

/// CSS `position` keyword for the current positioning foundation subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// CSS `z-index` value for the current stacking subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZIndex {
    Auto,
    Integer(i32),
}

pub fn parse_color(value: &str) -> Option<(u8, u8, u8, u8)> {
    let s = value.trim().to_ascii_lowercase();
    // HEX
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            return Some((r, g, b, 255));
        } else if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b, 255));
        }
    }

    let named = match s.as_str() {
        "black" => (0, 0, 0, 255),
        "blue" => (0, 0, 255, 255),
        "cyan" => (0, 255, 255, 255),
        "gray" | "grey" => (128, 128, 128, 255),
        "green" => (0, 128, 0, 255),
        "magenta" => (255, 0, 255, 255),
        "maroon" => (128, 0, 0, 255),
        "navy" => (0, 0, 128, 255),
        "olive" => (128, 128, 0, 255),
        "purple" => (128, 0, 128, 255),
        "red" => (255, 0, 0, 255),
        "silver" => (192, 192, 192, 255),
        "teal" => (0, 128, 128, 255),
        "white" => (255, 255, 255, 255),
        "yellow" => (255, 255, 0, 255),
        _ => return None,
    };
    Some(named)
}

/// Parse a `font-size` value into a Length.
/// For now we only support `NNpx` (e.g., "16px", "12.5px").
pub fn parse_length(value: &str) -> Option<Length> {
    let v = value.trim();

    // Only support `<number>px` for now.
    if let Some(px_str) = v.strip_suffix("px") {
        let num = px_str.trim().parse::<f32>().ok()?;
        if num.is_finite() && num > 0.0 {
            return Some(Length::Px(num));
        }
    }
    // Future: em/rem/%/pt/etc
    None
}

/// Parse a `display` value into a Display enum.
/// We keep this strict and only support a small subset for now.
pub fn parse_display(value: &str) -> Option<Display> {
    let v = value.trim().to_ascii_lowercase();

    match v.as_str() {
        "block" => Some(Display::Block),
        "inline" => Some(Display::Inline),
        "inline-block" => Some(Display::InlineBlock),
        "list-item" => Some(Display::ListItem),
        "flex" => Some(Display::Flex),
        "none" => Some(Display::None),
        _ => None, // unknown / unsupported → ignored
    }
}
