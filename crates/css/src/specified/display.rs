use crate::{model::ValueComponent, properties::PropertyId};

use super::{
    core::{keyword_value, unsupported_component_error},
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    value::{SpecifiedDisplay, SpecifiedDisplayKeyword},
};

pub(super) fn parse_display(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedDisplay, SpecifiedValueParseError> {
    let Some(keyword) = keyword_value(property, component)? else {
        return Err(unsupported_component_error(property, component));
    };

    let display_keyword = match keyword.canonical() {
        "block" => SpecifiedDisplayKeyword::Block,
        "inline" => SpecifiedDisplayKeyword::Inline,
        "inline-block" => SpecifiedDisplayKeyword::InlineBlock,
        "list-item" => SpecifiedDisplayKeyword::ListItem,
        "flex" => SpecifiedDisplayKeyword::Flex,
        "none" => SpecifiedDisplayKeyword::None,
        _ => {
            return Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedDisplayKeyword,
            ));
        }
    };

    Ok(SpecifiedDisplay {
        span: keyword.span(),
        keyword: display_keyword,
    })
}
