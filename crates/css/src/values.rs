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
