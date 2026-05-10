use crate::{model::ValueComponent, properties::PropertyId};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::ident_keyword,
    value::{SpecifiedOverflow, SpecifiedOverflowKeyword},
};

pub(super) fn parse_overflow(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedOverflow, SpecifiedValueParseError> {
    let Some((keyword, span)) = ident_keyword(property, component)? else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        ));
    };

    let keyword = match keyword.as_str() {
        "visible" => SpecifiedOverflowKeyword::Visible,
        "hidden" => SpecifiedOverflowKeyword::Hidden,
        "clip" => SpecifiedOverflowKeyword::Clip,
        "scroll" => SpecifiedOverflowKeyword::Scroll,
        "auto" => SpecifiedOverflowKeyword::Auto,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedOverflowKeyword,
            ));
        }
    };

    Ok(SpecifiedOverflow { span, keyword })
}
