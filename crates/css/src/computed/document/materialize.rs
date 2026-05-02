//! Single-element computed-style materialization.

use crate::{
    PropertyId, PropertyInheritance,
    cascade::{ResolvedStyle, ResolvedValueSource},
    property_registry,
};

use super::{
    super::{
        builder::ComputedStyleBuilder,
        style::ComputedStyle,
        value::{ComputedValue, normalize_specified_value},
    },
    error::ComputedStyleResolutionError,
};

/// Materializes the structured cascade handoff into a total computed style.
///
/// Rejected invalid declarations do not appear in `ResolvedStyle` winners.
/// Fallback is therefore applied by the cascade source carried in each entry:
/// another valid winner, inheritance, or the property's initial/default value.
pub fn compute_style_from_resolved_style(
    resolved_style: &ResolvedStyle,
    parent_style: Option<&ComputedStyle>,
) -> Result<ComputedStyle, ComputedStyleResolutionError> {
    let mut builder = ComputedStyleBuilder::new();

    for property in property_registry().ids() {
        let entry = resolved_style
            .get(property)
            .ok_or(ComputedStyleResolutionError::MissingResolvedProperty { property })?;
        let value = computed_value_from_resolved_source(property, entry.source(), parent_style)?;
        builder
            .record(property, value)
            .map_err(ComputedStyleResolutionError::Build)?;
    }

    builder.build().map_err(ComputedStyleResolutionError::Build)
}

fn computed_value_from_resolved_source(
    property: PropertyId,
    source: &ResolvedValueSource,
    parent_style: Option<&ComputedStyle>,
) -> Result<ComputedValue, ComputedStyleResolutionError> {
    match source {
        ResolvedValueSource::Winner(winner) => {
            let specified = winner
                .value
                .parsed()
                .ok_or(ComputedStyleResolutionError::WinnerMissingSpecifiedValue { property })?;
            if specified.property() != property {
                return Err(ComputedStyleResolutionError::WinnerPropertyMismatch {
                    property,
                    value_property: specified.property(),
                });
            }

            normalize_specified_value(specified)
                .map_err(ComputedStyleResolutionError::Normalization)
        }
        ResolvedValueSource::Inherited => {
            if property.metadata().inheritance != PropertyInheritance::Inherited {
                return Err(
                    ComputedStyleResolutionError::NonInheritedPropertyMarkedInherited { property },
                );
            }

            let parent = parent_style
                .ok_or(ComputedStyleResolutionError::MissingInheritedParent { property })?;
            Ok(parent.get(property).value())
        }
        ResolvedValueSource::Initial(initial) => {
            let expected = property.initial_value();
            if *initial != expected {
                return Err(ComputedStyleResolutionError::InitialValueMismatch {
                    property,
                    expected,
                    actual: *initial,
                });
            }

            Ok(ComputedValue::from_initial(property))
        }
    }
}
