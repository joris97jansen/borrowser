use crate::{model::ValueComponent, properties::PropertyId};

use super::{
    core::{keyword_value, unsupported_component_error},
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    value::{SpecifiedOverflow, SpecifiedOverflowKeyword},
};

pub(super) fn parse_overflow(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedOverflow, SpecifiedValueParseError> {
    let Some(keyword) = keyword_value(property, component)? else {
        return Err(unsupported_component_error(property, component));
    };

    let overflow_keyword = match keyword.canonical() {
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

    Ok(SpecifiedOverflow {
        span: keyword.span(),
        keyword: overflow_keyword,
    })
}
