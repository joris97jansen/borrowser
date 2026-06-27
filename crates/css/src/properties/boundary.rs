use std::fmt::Write;

use super::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
    PropertySpecifiedValueKind, property_registry,
};

/// CSS-owned summary of the specified/computed value boundary for one
/// supported longhand.
///
/// This is an internal inspection contract derived from the property registry.
/// It does not perform parsing, cascade, computed-value normalization, or
/// layout-dependent resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyValueBoundary {
    pub property: PropertyId,
    pub name: &'static str,
    pub specified_value: PropertySpecifiedValueKind,
    pub computed_value: PropertyComputedValueKind,
    pub inheritance: PropertyInheritance,
    pub initial: InitialStyleValue,
    pub conversion: SpecifiedToComputedConversionRule,
}

/// Narrow classification for how a supported specified-value family becomes
/// the corresponding computed-value family.
///
/// The actual conversion still happens through `css::computed`; this enum is
/// only metadata for docs, tests, and deterministic boundary inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedToComputedConversionRule {
    ColorToRgba,
    KeywordToComputedEnum,
    AbsoluteLengthToCssPx,
    LengthPercentageOrAutoPreservingPercentages,
    LengthPercentageOrNonePreservingPercentages,
    ZIndexAutoOrInteger,
}

impl SpecifiedToComputedConversionRule {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::ColorToRgba => "color-to-rgba",
            Self::KeywordToComputedEnum => "keyword-to-computed-enum",
            Self::AbsoluteLengthToCssPx => "absolute-length-to-css-px",
            Self::LengthPercentageOrAutoPreservingPercentages => {
                "length-percentage-or-auto-preserving-percentages"
            }
            Self::LengthPercentageOrNonePreservingPercentages => {
                "length-percentage-or-none-preserving-percentages"
            }
            Self::ZIndexAutoOrInteger => "z-index-auto-or-integer",
        }
    }
}

/// Iterates the specified/computed value boundaries in canonical registry
/// order.
pub fn property_value_boundaries() -> impl Iterator<Item = PropertyValueBoundary> {
    property_registry().ids().map(property_value_boundary)
}

/// Returns the specified/computed value boundary for one supported longhand.
pub fn property_value_boundary(property: PropertyId) -> PropertyValueBoundary {
    let registration = property_registry().get(property);
    let metadata = registration.metadata();
    PropertyValueBoundary {
        property: registration.id(),
        name: registration.name(),
        specified_value: metadata.specified_value,
        computed_value: metadata.computed_value,
        inheritance: metadata.inheritance,
        initial: metadata.initial,
        conversion: specified_to_computed_conversion_rule(
            metadata.specified_value,
            metadata.computed_value,
        ),
    }
}

/// Deterministic debug snapshot for the supported longhand
/// specified/computed value boundaries.
///
/// This is an internal regression surface over typed registry metadata. It is
/// not CSSOM, parser text, or the implementation path for computed values.
pub fn property_value_boundary_debug_snapshot() -> String {
    let boundaries = property_value_boundaries().collect::<Vec<_>>();
    let mut output = String::from("version: 1\nproperty-value-boundaries\n");
    writeln!(&mut output, "properties: {}", boundaries.len()).expect("write snapshot");

    for (index, boundary) in boundaries.iter().enumerate() {
        writeln!(
            &mut output,
            "property[{index}]: {:?} ({})",
            boundary.property, boundary.name
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  specified-value: {}",
            boundary.specified_value.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  computed-value: {}",
            boundary.computed_value.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  inheritance: {}",
            boundary.inheritance.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  initial: {}",
            boundary.initial.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  conversion: {}",
            boundary.conversion.as_debug_label()
        )
        .expect("write snapshot");
    }

    output
}

fn specified_to_computed_conversion_rule(
    specified: PropertySpecifiedValueKind,
    computed: PropertyComputedValueKind,
) -> SpecifiedToComputedConversionRule {
    match (specified, computed) {
        (PropertySpecifiedValueKind::Color, PropertyComputedValueKind::AbsoluteColor) => {
            SpecifiedToComputedConversionRule::ColorToRgba
        }
        (
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
        )
        | (
            PropertySpecifiedValueKind::OutlineStyleKeyword,
            PropertyComputedValueKind::OutlineStyleKeyword,
        )
        | (
            PropertySpecifiedValueKind::TextDecorationLineKeyword,
            PropertyComputedValueKind::TextDecorationLineKeyword,
        )
        | (PropertySpecifiedValueKind::DisplayKeyword, PropertyComputedValueKind::DisplayKeyword)
        | (
            PropertySpecifiedValueKind::OverflowKeyword,
            PropertyComputedValueKind::OverflowKeyword,
        )
        | (
            PropertySpecifiedValueKind::PositionKeyword,
            PropertyComputedValueKind::PositionKeyword,
        ) => SpecifiedToComputedConversionRule::KeywordToComputedEnum,
        (PropertySpecifiedValueKind::ZIndex, PropertyComputedValueKind::ZIndex) => {
            SpecifiedToComputedConversionRule::ZIndexAutoOrInteger
        }
        (PropertySpecifiedValueKind::AbsoluteLength, PropertyComputedValueKind::AbsoluteLength) => {
            SpecifiedToComputedConversionRule::AbsoluteLengthToCssPx
        }
        (
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
        ) => SpecifiedToComputedConversionRule::LengthPercentageOrAutoPreservingPercentages,
        (
            PropertySpecifiedValueKind::LengthPercentageOrNone,
            PropertyComputedValueKind::LengthPercentageOrNone,
        ) => SpecifiedToComputedConversionRule::LengthPercentageOrNonePreservingPercentages,
        _ => unreachable!(
            "property registry contains unsupported specified/computed value kind pairing: {specified:?} -> {computed:?}"
        ),
    }
}
