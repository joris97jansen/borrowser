use crate::{model::ValueComponent, properties::PropertyId};

use super::{
    core::{keyword_value, unsupported_component_error},
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    value::{SpecifiedPosition, SpecifiedPositionKeyword},
};

pub(super) fn parse_position(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedPosition, SpecifiedValueParseError> {
    let Some(keyword) = keyword_value(property, component)? else {
        return Err(unsupported_component_error(property, component));
    };

    let position_keyword = match keyword.canonical() {
        "static" => SpecifiedPositionKeyword::Static,
        "relative" => SpecifiedPositionKeyword::Relative,
        "absolute" => SpecifiedPositionKeyword::Absolute,
        "fixed" => SpecifiedPositionKeyword::Fixed,
        "sticky" => SpecifiedPositionKeyword::Sticky,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedPositionKeyword,
            ));
        }
    };

    Ok(SpecifiedPosition {
        span: keyword.span(),
        keyword: position_keyword,
    })
}
