use super::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
    PropertyInvalidValuePolicy, PropertyInvalidationImpact, PropertyLengthSignPolicy,
    PropertySpecifiedValueKind, data::PROPERTY_LOOKUP_BY_NAME, property_registry,
};

#[test]
fn property_registry_entries_are_total_canonical_and_metadata_backed() {
    let expected = [
        (
            PropertyId::BackgroundColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::BorderBottomColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::BorderBottomStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::BorderBottomWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::BorderLeftColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::BorderLeftStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::BorderLeftWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::BorderRightColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::BorderRightStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::BorderRightWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::BorderTopColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::BorderTopStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::BorderTopWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::Color,
            PropertyInheritance::Inherited,
            InitialStyleValue::ColorBlack,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::Display,
            PropertyInheritance::NotInherited,
            InitialStyleValue::DisplayInline,
            PropertySpecifiedValueKind::DisplayKeyword,
            PropertyComputedValueKind::DisplayKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::FontSize,
            PropertyInheritance::Inherited,
            InitialStyleValue::FontSizePx16,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::Height,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::MarginBottom,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::MarginLeft,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::MarginRight,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::MarginTop,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::MaxWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::NoneKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrNone,
            PropertyComputedValueKind::LengthPercentageOrNone,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::MinWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::Overflow,
            PropertyInheritance::NotInherited,
            InitialStyleValue::OverflowVisible,
            PropertySpecifiedValueKind::OverflowKeyword,
            PropertyComputedValueKind::OverflowKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::OutlineColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::OutlineStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::OutlineStyleNone,
            PropertySpecifiedValueKind::OutlineStyleKeyword,
            PropertyComputedValueKind::OutlineStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::OutlineWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::PaddingBottom,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::PaddingLeft,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::PaddingRight,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::PaddingTop,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::Position,
            PropertyInheritance::NotInherited,
            InitialStyleValue::PositionStatic,
            PropertySpecifiedValueKind::PositionKeyword,
            PropertyComputedValueKind::PositionKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::TextDecorationLine,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TextDecorationLineNone,
            PropertySpecifiedValueKind::TextDecorationLineKeyword,
            PropertyComputedValueKind::TextDecorationLineKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RepaintOnly,
        ),
        (
            PropertyId::Width,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
        (
            PropertyId::ZIndex,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZIndexAuto,
            PropertySpecifiedValueKind::ZIndex,
            PropertyComputedValueKind::ZIndex,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::RelayoutAndRepaint,
        ),
    ];

    let registry = property_registry();
    assert_eq!(registry.entries().len(), expected.len());
    assert_eq!(PropertyId::ALL.len(), expected.len());

    for (
        index,
        (
            property,
            inheritance,
            initial,
            specified_value,
            computed_value,
            length_sign,
            invalidation_impact,
        ),
    ) in expected.into_iter().enumerate()
    {
        assert_eq!(PropertyId::ALL[index], property);

        let registration = &registry.entries()[index];
        assert_eq!(registration.id(), property);
        assert_eq!(registration.name(), property.name());
        assert_eq!(PropertyId::from_name(property.name()), Some(property));
        assert_eq!(registry.lookup_id(property.name()), Some(property));

        let metadata = registration.metadata();
        assert_eq!(metadata.inheritance, inheritance, "{}", property.name());
        assert_eq!(metadata.initial, initial, "{}", property.name());
        assert_eq!(
            metadata.specified_value,
            specified_value,
            "{}",
            property.name()
        );
        assert_eq!(
            metadata.computed_value,
            computed_value,
            "{}",
            property.name()
        );
        assert_eq!(
            metadata.invalid_value_policy,
            PropertyInvalidValuePolicy::RejectDeclaration,
            "{}",
            property.name()
        );
        assert_eq!(metadata.length_sign, length_sign, "{}", property.name());
        assert_eq!(
            metadata.invalidation_impact,
            invalidation_impact,
            "{}",
            property.name()
        );
        assert_eq!(property.initial_value(), initial, "{}", property.name());
    }
}

#[test]
fn property_registry_lookup_is_deterministic_for_representative_property_names() {
    let registry = property_registry();

    assert_eq!(
        registry.lookup("background-color").map(|entry| entry.id()),
        Some(PropertyId::BackgroundColor)
    );
    assert_eq!(
        registry.lookup("font-size").map(|entry| entry.id()),
        Some(PropertyId::FontSize)
    );
    assert_eq!(
        registry.lookup("padding-left").map(|entry| entry.id()),
        Some(PropertyId::PaddingLeft)
    );
    assert_eq!(
        registry.lookup("overflow").map(|entry| entry.id()),
        Some(PropertyId::Overflow)
    );
    assert_eq!(
        registry.lookup("outline-style").map(|entry| entry.id()),
        Some(PropertyId::OutlineStyle)
    );
    assert_eq!(
        registry
            .lookup("text-decoration-line")
            .map(|entry| entry.id()),
        Some(PropertyId::TextDecorationLine)
    );
    assert_eq!(
        registry.lookup("width").map(|entry| entry.id()),
        Some(PropertyId::Width)
    );
    assert_eq!(
        registry.lookup("z-index").map(|entry| entry.id()),
        Some(PropertyId::ZIndex)
    );
    assert_eq!(registry.lookup("zoom"), None);
    assert_eq!(registry.lookup("COLOR"), None);
}

#[test]
fn property_registry_invalidation_impact_is_explicit_for_every_supported_longhand() {
    let repaint_only = [
        PropertyId::BackgroundColor,
        PropertyId::BorderBottomColor,
        PropertyId::BorderLeftColor,
        PropertyId::BorderRightColor,
        PropertyId::BorderTopColor,
        PropertyId::Color,
        PropertyId::OutlineColor,
        PropertyId::OutlineStyle,
        PropertyId::OutlineWidth,
        PropertyId::TextDecorationLine,
    ];
    let relayout_and_repaint = [
        PropertyId::BorderBottomStyle,
        PropertyId::BorderBottomWidth,
        PropertyId::BorderLeftStyle,
        PropertyId::BorderLeftWidth,
        PropertyId::BorderRightStyle,
        PropertyId::BorderRightWidth,
        PropertyId::BorderTopStyle,
        PropertyId::BorderTopWidth,
        PropertyId::Display,
        PropertyId::FontSize,
        PropertyId::Height,
        PropertyId::MarginBottom,
        PropertyId::MarginLeft,
        PropertyId::MarginRight,
        PropertyId::MarginTop,
        PropertyId::MaxWidth,
        PropertyId::MinWidth,
        PropertyId::Overflow,
        PropertyId::PaddingBottom,
        PropertyId::PaddingLeft,
        PropertyId::PaddingRight,
        PropertyId::PaddingTop,
        PropertyId::Position,
        PropertyId::Width,
        PropertyId::ZIndex,
    ];

    assert_eq!(
        repaint_only.len() + relayout_and_repaint.len(),
        property_registry().entries().len()
    );

    for property in repaint_only {
        assert_eq!(
            property.metadata().invalidation_impact,
            PropertyInvalidationImpact::RepaintOnly,
            "{}",
            property.name()
        );
    }

    for property in relayout_and_repaint {
        assert_eq!(
            property.metadata().invalidation_impact,
            PropertyInvalidationImpact::RelayoutAndRepaint,
            "{}",
            property.name()
        );
    }
}

#[test]
fn property_registry_classifies_representative_paint_and_layout_impact() {
    for property in [
        PropertyId::Color,
        PropertyId::BackgroundColor,
        PropertyId::OutlineWidth,
        PropertyId::TextDecorationLine,
    ] {
        assert_eq!(
            property.metadata().invalidation_impact,
            PropertyInvalidationImpact::RepaintOnly,
            "{}",
            property.name()
        );
    }

    for property in [
        PropertyId::Display,
        PropertyId::FontSize,
        PropertyId::Width,
        PropertyId::PaddingLeft,
        PropertyId::BorderTopWidth,
        PropertyId::Overflow,
    ] {
        assert_eq!(
            property.metadata().invalidation_impact,
            PropertyInvalidationImpact::RelayoutAndRepaint,
            "{}",
            property.name()
        );
    }
}

#[test]
fn unsupported_flex_properties_are_not_registered_for_cascade_or_computed_style() {
    let registry = property_registry();
    let unsupported_flex_properties = [
        "align-content",
        "align-items",
        "align-self",
        "column-gap",
        "flex",
        "flex-basis",
        "flex-direction",
        "flex-flow",
        "flex-grow",
        "flex-shrink",
        "flex-wrap",
        "gap",
        "justify-content",
        "order",
        "row-gap",
    ];

    for name in unsupported_flex_properties {
        assert_eq!(PropertyId::from_name(name), None, "{name}");
        assert_eq!(registry.lookup(name), None, "{name}");
        assert!(
            PropertyId::ALL
                .iter()
                .all(|property| property.name() != name),
            "{name}"
        );
    }
}

#[test]
fn unsupported_shorthands_are_not_registered_for_cascade_or_computed_style() {
    let registry = property_registry();
    let unsupported_shorthand_properties = [
        "background",
        "border",
        "border-color",
        "border-style",
        "border-width",
        "font",
        "margin",
        "outline",
        "padding",
        "text-decoration",
    ];

    for name in unsupported_shorthand_properties {
        assert_eq!(PropertyId::from_name(name), None, "{name}");
        assert_eq!(registry.lookup(name), None, "{name}");
        assert!(
            PropertyId::ALL
                .iter()
                .all(|property| property.name() != name),
            "{name}"
        );
    }
}

#[test]
fn unsupported_text_decoration_properties_are_not_registered_for_cascade_or_computed_style() {
    let registry = property_registry();
    let unsupported_text_decoration_properties = [
        "text-decoration",
        "text-decoration-color",
        "text-decoration-style",
        "text-decoration-thickness",
    ];

    for name in unsupported_text_decoration_properties {
        assert_eq!(PropertyId::from_name(name), None, "{name}");
        assert_eq!(registry.lookup(name), None, "{name}");
        assert!(
            PropertyId::ALL
                .iter()
                .all(|property| property.name() != name),
            "{name}"
        );
    }
}

#[test]
fn property_lookup_table_is_sorted_for_binary_search() {
    let names = PROPERTY_LOOKUP_BY_NAME
        .iter()
        .map(|entry| entry.name)
        .collect::<Vec<_>>();

    let mut sorted = names.clone();
    sorted.sort_unstable();

    assert_eq!(names, sorted);
}

#[test]
fn property_registry_get_returns_registration_for_every_supported_id() {
    let registry = property_registry();

    for property in PropertyId::ALL {
        let registration = registry.get(property);
        assert_eq!(registration.id(), property);
        assert_eq!(registration.name(), property.name());
        assert_eq!(registration.metadata(), property.metadata());
    }
}
