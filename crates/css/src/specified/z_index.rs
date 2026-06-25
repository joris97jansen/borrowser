use crate::{model::ValueComponent, properties::PropertyId};

use super::{
    core::{integer_value, keyword_value, unsupported_component_error},
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
    value::{SpecifiedZIndex, SpecifiedZIndexValue},
};

pub(super) fn parse_z_index(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<SpecifiedZIndex, SpecifiedValueParseError> {
    if let Some(keyword) = keyword_value(property, component)? {
        return if keyword.canonical() == "auto" {
            Ok(SpecifiedZIndex {
                value: SpecifiedZIndexValue::Auto {
                    span: keyword.span(),
                },
            })
        } else {
            Err(error(
                property,
                SpecifiedValueParseErrorKind::UnsupportedKeyword,
            ))
        };
    }

    let Some(value) = integer_value(property, component)? else {
        return Err(unsupported_component_error(property, component));
    };

    Ok(SpecifiedZIndex {
        value: SpecifiedZIndexValue::Integer(value),
    })
}
