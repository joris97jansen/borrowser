use crate::{PropertyId, model::ValueComponent, syntax::CssSpan};

use super::{
    core::{keyword_value, unsupported_component_error},
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    value::{SpecifiedBorderStyle, SpecifiedBorderStyleKeyword},
};

pub(super) fn parse_border_style(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedBorderStyle, SpecifiedValueParseError> {
    let Some(keyword) = keyword_value(property, component)? else {
        return Err(unsupported_component_error(property, component));
    };

    let style_keyword = match keyword.canonical() {
        "none" => SpecifiedBorderStyleKeyword::None,
        "solid" => SpecifiedBorderStyleKeyword::Solid,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ));
        }
    };

    Ok(SpecifiedBorderStyle {
        span: keyword.span(),
        keyword: style_keyword,
    })
}

impl SpecifiedBorderStyleKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Solid => "solid",
        }
    }
}

impl SpecifiedBorderStyle {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn keyword(&self) -> SpecifiedBorderStyleKeyword {
        self.keyword
    }

    pub fn to_css_text(&self) -> &'static str {
        self.keyword.as_css_keyword()
    }
}
