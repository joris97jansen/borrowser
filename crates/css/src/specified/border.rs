use crate::{PropertyId, model::ValueComponent, syntax::CssSpan};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::ident_keyword,
    value::{SpecifiedBorderStyle, SpecifiedBorderStyleKeyword},
};

pub(super) fn parse_border_style(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedBorderStyle, SpecifiedValueParseError> {
    let Some((keyword, span)) = ident_keyword(property, component)? else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedKeyword,
        ));
    };

    let keyword = match keyword.as_str() {
        "none" => SpecifiedBorderStyleKeyword::None,
        "solid" => SpecifiedBorderStyleKeyword::Solid,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ));
        }
    };

    Ok(SpecifiedBorderStyle { span, keyword })
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
