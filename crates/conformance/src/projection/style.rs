use super::{HarnessError, error};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug)]
pub(crate) enum StyleProperty {
    Color,
    BackgroundColor,
    FontSize,
    Display,
}
impl StyleProperty {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::BackgroundColor => "background-color",
            Self::FontSize => "font-size",
            Self::Display => "display",
        }
    }
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Color => 0,
            Self::BackgroundColor => 1,
            Self::FontSize => 2,
            Self::Display => 3,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum StyleValue {
    Rgba8([u8; 4]),
    CssPx(f64),
    Keyword(String),
}
fn color(value: [u8; 4]) -> Result<StyleValue, HarnessError> {
    if value[3] != 255 && value != [0, 0, 0, 0] {
        return Err(error(
            "color outside selected opaque sRGB/transparent-black domain",
        ));
    }
    Ok(StyleValue::Rgba8(value))
}
fn font(value: f64) -> Result<StyleValue, HarnessError> {
    if !value.is_finite() || value < 0.0 {
        Err(error("invalid absolute computed font size"))
    } else {
        Ok(StyleValue::CssPx(if value == 0.0 { 0.0 } else { value }))
    }
}
fn display(value: &str) -> Result<StyleValue, HarnessError> {
    if !["block", "inline", "none"].contains(&value) {
        Err(error("display outside selected static domain"))
    } else {
        Ok(StyleValue::Keyword(value.into()))
    }
}
pub(crate) fn borrowser(
    s: &css::ComputedStyle,
    p: StyleProperty,
) -> Result<StyleValue, HarnessError> {
    match p {
        StyleProperty::Color | StyleProperty::BackgroundColor => {
            let (r, g, b, a) = if matches!(p, StyleProperty::Color) {
                s.color()
            } else {
                s.background_color()
            };
            color([r, g, b, a])
        }
        StyleProperty::FontSize => {
            let css::Length::Px(px) = s.font_size();
            font(f64::from(px))
        }
        StyleProperty::Display => display(match s.display() {
            css::Display::Block => "block",
            css::Display::Inline => "inline",
            css::Display::None => "none",
            _ => return Err(error("display outside selected static domain")),
        }),
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn chromium(value: &str, p: StyleProperty) -> Result<StyleValue, HarnessError> {
    match p {
        StyleProperty::Color | StyleProperty::BackgroundColor => {
            if value == "rgba(0, 0, 0, 0)" {
                return color([0, 0, 0, 0]);
            }
            let values = value
                .strip_prefix("rgb(")
                .and_then(|v| v.strip_suffix(')'))
                .ok_or_else(|| error("invalid native sRGB color serialization"))?;
            let channels = values
                .split(", ")
                .map(|v| {
                    v.parse::<u8>()
                        .map_err(|_| error("invalid native color channel"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let [r, g, b]: [u8; 3] = channels
                .try_into()
                .map_err(|_| error("invalid native RGB channel count"))?;
            color([r, g, b, 255])
        }
        StyleProperty::FontSize => font(
            value
                .strip_suffix("px")
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| error("native font-size is not absolute px"))?,
        ),
        StyleProperty::Display => display(value),
    }
}
