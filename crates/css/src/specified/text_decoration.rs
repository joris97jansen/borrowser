use crate::{PropertyId, model::ValueComponent, syntax::CssSpan};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::ident_keyword,
    value::{SpecifiedTextDecorationLine, SpecifiedTextDecorationLineKeyword},
};

pub(super) fn parse_text_decoration_line(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedTextDecorationLine, SpecifiedValueParseError> {
    let Some((keyword, span)) = ident_keyword(property, component)? else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedKeyword,
        ));
    };

    let keyword = match keyword.as_str() {
        "none" => SpecifiedTextDecorationLineKeyword::None,
        "underline" => SpecifiedTextDecorationLineKeyword::Underline,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ));
        }
    };

    Ok(SpecifiedTextDecorationLine { span, keyword })
}

impl SpecifiedTextDecorationLineKeyword {
    pub fn as_css_keyword(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Underline => "underline",
        }
    }
}

impl SpecifiedTextDecorationLine {
    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn keyword(&self) -> SpecifiedTextDecorationLineKeyword {
        self.keyword
    }

    pub fn to_css_text(&self) -> &'static str {
        self.keyword.as_css_keyword()
    }
}
