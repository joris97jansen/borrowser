use crate::{PropertyId, model::ValueComponent, syntax::CssSpan};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::ident_keyword,
    value::{SpecifiedOutlineStyle, SpecifiedOutlineStyleKeyword},
};

pub(super) fn parse_outline_style(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedOutlineStyle, SpecifiedValueParseError> {
    let Some((keyword, span)) = ident_keyword(property, component)? else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedKeyword,
        ));
    };

    let keyword = match keyword.as_str() {
        "none" => SpecifiedOutlineStyleKeyword::None,
        "solid" => SpecifiedOutlineStyleKeyword::Solid,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ));
        }
    };

    Ok(SpecifiedOutlineStyle { span, keyword })
}

impl SpecifiedOutlineStyleKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Solid => "solid",
        }
    }
}

impl SpecifiedOutlineStyle {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn keyword(&self) -> SpecifiedOutlineStyleKeyword {
        self.keyword
    }

    pub fn to_css_text(&self) -> &'static str {
        self.keyword.as_css_keyword()
    }
}
