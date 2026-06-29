use super::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
    PropertyInvalidValuePolicy, PropertyInvalidationImpact, PropertyLengthSignPolicy,
    PropertySpecifiedValueKind, ShorthandId, SpecifiedToComputedConversionRule,
    data::PROPERTY_LOOKUP_BY_NAME, property_registry, property_value_boundaries,
    property_value_boundary_debug_snapshot, shorthand_registry,
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
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::BorderBottomColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::BorderBottomStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::BorderBottomWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::BorderLeftColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::BorderLeftStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::BorderLeftWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::BorderRightColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::BorderRightStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::BorderRightWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::BorderTopColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::BorderTopStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::BorderTopWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::Color,
            PropertyInheritance::Inherited,
            InitialStyleValue::ColorBlack,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::inherited_paint(),
        ),
        (
            PropertyId::Display,
            PropertyInheritance::NotInherited,
            InitialStyleValue::DisplayInline,
            PropertySpecifiedValueKind::DisplayKeyword,
            PropertyComputedValueKind::DisplayKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::box_tree_layout_paint(),
        ),
        (
            PropertyId::FontSize,
            PropertyInheritance::Inherited,
            InitialStyleValue::FontSizePx16,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::inherited_text_metrics_layout_paint(),
        ),
        (
            PropertyId::Height,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::MarginBottom,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::MarginLeft,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::MarginRight,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::MarginTop,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::MaxWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::NoneKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrNone,
            PropertyComputedValueKind::LengthPercentageOrNone,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::MinWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::Overflow,
            PropertyInheritance::NotInherited,
            InitialStyleValue::OverflowVisible,
            PropertySpecifiedValueKind::OverflowKeyword,
            PropertyComputedValueKind::OverflowKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::overflow_clip_layout_paint(),
        ),
        (
            PropertyId::OutlineColor,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::OutlineStyle,
            PropertyInheritance::NotInherited,
            InitialStyleValue::OutlineStyleNone,
            PropertySpecifiedValueKind::OutlineStyleKeyword,
            PropertyComputedValueKind::OutlineStyleKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::OutlineWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::PaddingBottom,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::PaddingLeft,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::PaddingRight,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::PaddingTop,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::Position,
            PropertyInheritance::NotInherited,
            InitialStyleValue::PositionStatic,
            PropertySpecifiedValueKind::PositionKeyword,
            PropertyComputedValueKind::PositionKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::layout_paint_order_paint(),
        ),
        (
            PropertyId::TextDecorationLine,
            PropertyInheritance::NotInherited,
            InitialStyleValue::TextDecorationLineNone,
            PropertySpecifiedValueKind::TextDecorationLineKeyword,
            PropertyComputedValueKind::TextDecorationLineKeyword,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::paint_only(),
        ),
        (
            PropertyId::Width,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyLengthSignPolicy::NonNegative,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
        (
            PropertyId::ZIndex,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZIndexAuto,
            PropertySpecifiedValueKind::ZIndex,
            PropertyComputedValueKind::ZIndex,
            PropertyLengthSignPolicy::NotLength,
            PropertyInvalidationImpact::conservative_layout_paint_order_paint(),
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
    let paint_only = [
        PropertyId::BackgroundColor,
        PropertyId::BorderBottomColor,
        PropertyId::BorderLeftColor,
        PropertyId::BorderRightColor,
        PropertyId::BorderTopColor,
        PropertyId::OutlineColor,
        PropertyId::OutlineStyle,
        PropertyId::OutlineWidth,
        PropertyId::TextDecorationLine,
    ];
    let layout_and_paint = [
        PropertyId::BorderBottomStyle,
        PropertyId::BorderBottomWidth,
        PropertyId::BorderLeftStyle,
        PropertyId::BorderLeftWidth,
        PropertyId::BorderRightStyle,
        PropertyId::BorderRightWidth,
        PropertyId::BorderTopStyle,
        PropertyId::BorderTopWidth,
        PropertyId::Height,
        PropertyId::MarginBottom,
        PropertyId::MarginLeft,
        PropertyId::MarginRight,
        PropertyId::MarginTop,
        PropertyId::MaxWidth,
        PropertyId::MinWidth,
        PropertyId::PaddingBottom,
        PropertyId::PaddingLeft,
        PropertyId::PaddingRight,
        PropertyId::PaddingTop,
        PropertyId::Width,
    ];

    assert_eq!(
        paint_only.len() + 1 + layout_and_paint.len() + 1 + 1 + 1 + 1 + 1,
        property_registry().entries().len()
    );

    for property in paint_only {
        assert_eq!(
            property.metadata().invalidation_impact,
            PropertyInvalidationImpact::paint_only(),
            "{}",
            property.name()
        );
    }

    assert_eq!(
        PropertyId::Color.metadata().invalidation_impact,
        PropertyInvalidationImpact::inherited_paint()
    );
    assert_eq!(
        PropertyId::Display.metadata().invalidation_impact,
        PropertyInvalidationImpact::box_tree_layout_paint()
    );
    assert_eq!(
        PropertyId::FontSize.metadata().invalidation_impact,
        PropertyInvalidationImpact::inherited_text_metrics_layout_paint()
    );
    assert_eq!(
        PropertyId::Overflow.metadata().invalidation_impact,
        PropertyInvalidationImpact::overflow_clip_layout_paint()
    );
    assert_eq!(
        PropertyId::Position.metadata().invalidation_impact,
        PropertyInvalidationImpact::layout_paint_order_paint()
    );
    assert_eq!(
        PropertyId::ZIndex.metadata().invalidation_impact,
        PropertyInvalidationImpact::conservative_layout_paint_order_paint()
    );

    for property in layout_and_paint {
        assert_eq!(
            property.metadata().invalidation_impact,
            PropertyInvalidationImpact::layout_and_paint(),
            "{}",
            property.name()
        );
    }
}

#[test]
fn future_compositor_impact_is_metadata_only_for_current_runtime() {
    let impact = PropertyInvalidationImpact::future_compositor_metadata();

    assert!(impact.affects_future_compositor());
    assert!(!impact.requires_runtime_layout());
    assert!(!impact.requires_runtime_paint());
    assert_eq!(impact.to_debug_label(), "future-compositor");
}

#[test]
fn property_value_boundaries_are_total_canonical_and_registry_derived() {
    let boundaries = property_value_boundaries().collect::<Vec<_>>();
    assert_eq!(boundaries.len(), PropertyId::ALL.len());
    assert_eq!(boundaries.len(), property_registry().entries().len());

    for (index, boundary) in boundaries.iter().enumerate() {
        let property = PropertyId::ALL[index];
        let metadata = property.metadata();

        assert_eq!(boundary.property, property);
        assert_eq!(boundary.name, property.name());
        assert_eq!(boundary.specified_value, metadata.specified_value);
        assert_eq!(boundary.computed_value, metadata.computed_value);
        assert_eq!(boundary.inheritance, metadata.inheritance);
        assert_eq!(boundary.initial, metadata.initial);
    }
}

#[test]
fn property_value_boundaries_classify_representative_conversion_rules() {
    let boundaries = property_value_boundaries().collect::<Vec<_>>();
    let conversion_for = |property| {
        boundaries
            .iter()
            .find(|boundary| boundary.property == property)
            .unwrap_or_else(|| panic!("missing boundary for {}", property.name()))
            .conversion
    };

    assert_eq!(
        conversion_for(PropertyId::Color),
        SpecifiedToComputedConversionRule::ColorToRgba
    );
    assert_eq!(
        conversion_for(PropertyId::Display),
        SpecifiedToComputedConversionRule::KeywordToComputedEnum
    );
    assert_eq!(
        conversion_for(PropertyId::FontSize),
        SpecifiedToComputedConversionRule::AbsoluteLengthToCssPx
    );
    assert_eq!(
        conversion_for(PropertyId::Width),
        SpecifiedToComputedConversionRule::LengthPercentageOrAutoPreservingPercentages
    );
    assert_eq!(
        conversion_for(PropertyId::MaxWidth),
        SpecifiedToComputedConversionRule::LengthPercentageOrNonePreservingPercentages
    );
    assert_eq!(
        conversion_for(PropertyId::ZIndex),
        SpecifiedToComputedConversionRule::ZIndexAutoOrInteger
    );
}

#[test]
fn property_value_boundary_snapshot_is_deterministic() {
    assert_eq!(
        property_value_boundary_debug_snapshot(),
        include_str!("../../tests/fixtures/properties/value_boundaries.snap")
    );
}

#[test]
fn property_registry_classifies_representative_ad7_impact_flags() {
    for property in [
        PropertyId::BackgroundColor,
        PropertyId::OutlineWidth,
        PropertyId::TextDecorationLine,
    ] {
        let impact = property.metadata().invalidation_impact;
        assert!(impact.affects_paint(), "{}", property.name());
        assert!(!impact.requires_runtime_layout(), "{}", property.name());
        assert!(!impact.is_conservative(), "{}", property.name());
    }

    let color = PropertyId::Color.metadata().invalidation_impact;
    assert!(color.affects_inherited_style());
    assert!(color.affects_paint());
    assert!(!color.requires_runtime_layout());

    let display = PropertyId::Display.metadata().invalidation_impact;
    assert!(display.affects_box_tree());
    assert!(display.requires_runtime_layout());
    assert!(display.requires_runtime_paint());

    let font_size = PropertyId::FontSize.metadata().invalidation_impact;
    assert!(font_size.affects_inherited_style());
    assert!(font_size.affects_text_metrics());
    assert!(font_size.requires_runtime_layout());

    let overflow = PropertyId::Overflow.metadata().invalidation_impact;
    assert!(overflow.affects_overflow_clip());
    assert!(overflow.requires_runtime_layout());
    assert!(overflow.requires_runtime_paint());

    let z_index = PropertyId::ZIndex.metadata().invalidation_impact;
    assert!(z_index.affects_paint_order());
    assert!(z_index.requires_runtime_layout());
    assert!(z_index.requires_runtime_paint());
    assert!(z_index.is_conservative());

    for property in [
        PropertyId::Width,
        PropertyId::PaddingLeft,
        PropertyId::BorderTopWidth,
    ] {
        let impact = property.metadata().invalidation_impact;
        assert!(impact.affects_layout(), "{}", property.name());
        assert!(impact.affects_paint(), "{}", property.name());
        assert!(!impact.affects_box_tree(), "{}", property.name());
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
fn shorthand_registry_keeps_outline_separate_from_supported_longhands() {
    let registry = shorthand_registry();
    let outline = registry
        .lookup("outline")
        .expect("outline shorthand registration");

    assert_eq!(outline.id(), ShorthandId::Outline);
    assert_eq!(outline.name(), "outline");
    assert_eq!(
        outline.longhands(),
        &[
            PropertyId::OutlineColor,
            PropertyId::OutlineStyle,
            PropertyId::OutlineWidth,
        ]
    );
    assert_eq!(
        ShorthandId::from_name("outline"),
        Some(ShorthandId::Outline)
    );
    assert_eq!(PropertyId::from_name("outline"), None);
    assert_eq!(property_registry().lookup("outline"), None);
    assert_eq!(registry.lookup("border"), None);
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
