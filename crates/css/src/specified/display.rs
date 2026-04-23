use crate::{model::ValueComponent, properties::PropertyId};

use super::{
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    parse::ident_keyword,
    value::{SpecifiedDisplay, SpecifiedDisplayKeyword},
};

pub(super) fn parse_display(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedDisplay, SpecifiedValueParseError> {
    let Some((keyword, span)) = ident_keyword(property, component)? else {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedComponent,
        ));
    };

    let keyword = match keyword.as_str() {
        "block" => SpecifiedDisplayKeyword::Block,
        "inline" => SpecifiedDisplayKeyword::Inline,
        "inline-block" => SpecifiedDisplayKeyword::InlineBlock,
        "list-item" => SpecifiedDisplayKeyword::ListItem,
        "none" => SpecifiedDisplayKeyword::None,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedDisplayKeyword,
            ));
        }
    };

    Ok(SpecifiedDisplay { span, keyword })
}
