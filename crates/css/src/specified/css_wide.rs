use crate::{
    model::ValueComponent,
    properties::PropertyId,
    values::{CssWideKeyword, CssWideKeywordValue},
};

use super::{
    core::keyword_value,
    error::{SpecifiedValueParseError, SpecifiedValueParseErrorKind, error},
};

/// Parses a CSS-wide keyword from one declaration value component.
///
/// Unsupported CSS-wide keywords are still recognized here and rejected with a
/// dedicated error so they cannot be confused with property-specific unknown
/// keywords.
pub(super) fn parse_supported_css_wide_keyword(
    property: PropertyId,
    component: &ValueComponent,
) -> Result<Option<CssWideKeywordValue>, SpecifiedValueParseError> {
    let Some(keyword) = keyword_value(property, component)? else {
        return Ok(None);
    };

    let Some(css_wide) = CssWideKeyword::from_canonical(keyword.canonical()) else {
        return Ok(None);
    };

    if !css_wide.is_supported_for_current_cascade() {
        return Err(error(
            property,
            SpecifiedValueParseErrorKind::UnsupportedCssWideKeyword,
        ));
    }

    Ok(Some(CssWideKeywordValue::new(keyword.span(), css_wide)))
}
