use crate::{model::ValueComponent, properties::PropertyId};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::ident_keyword,
    value::{SpecifiedPosition, SpecifiedPositionKeyword},
};

pub(super) fn parse_position(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedPosition, SpecifiedValueParseError> {
    let Some((keyword, span)) = ident_keyword(property, component)? else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        ));
    };

    let keyword = match keyword.as_str() {
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

    Ok(SpecifiedPosition { span, keyword })
}
