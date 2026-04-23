use super::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
    PropertyInvalidValuePolicy, PropertyLengthSignPolicy, PropertySpecifiedValueKind,
    data::PROPERTY_LOOKUP_BY_NAME, property_registry,
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
        ),
        (
            PropertyId::Color,
            PropertyInheritance::Inherited,
            InitialStyleValue::ColorBlack,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyLengthSignPolicy::NotLength,
        ),
        (
            PropertyId::Display,
            PropertyInheritance::NotInherited,
            InitialStyleValue::DisplayInline,
            PropertySpecifiedValueKind::DisplayKeyword,
            PropertyComputedValueKind::DisplayKeyword,
            PropertyLengthSignPolicy::NotLength,
        ),
        (
            PropertyId::FontSize,
            PropertyInheritance::Inherited,
            InitialStyleValue::FontSizePx16,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::Height,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
            PropertyComputedValueKind::AbsoluteLengthOrAuto,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::MarginBottom,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
        ),
        (
            PropertyId::MarginLeft,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
        ),
        (
            PropertyId::MarginRight,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
        ),
        (
            PropertyId::MarginTop,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::AllowNegative,
        ),
        (
            PropertyId::MaxWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::NoneKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrNone,
            PropertyComputedValueKind::AbsoluteLengthOrNone,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::MinWidth,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
            PropertyComputedValueKind::AbsoluteLengthOrAuto,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::PaddingBottom,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::PaddingLeft,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::PaddingRight,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::PaddingTop,
            PropertyInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyLengthSignPolicy::NonNegative,
        ),
        (
            PropertyId::Width,
            PropertyInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
            PropertyComputedValueKind::AbsoluteLengthOrAuto,
            PropertyLengthSignPolicy::NonNegative,
        ),
    ];

    let registry = property_registry();
    assert_eq!(registry.entries().len(), expected.len());
    assert_eq!(PropertyId::ALL.len(), expected.len());

    for (index, (property, inheritance, initial, specified_value, computed_value, length_sign)) in
        expected.into_iter().enumerate()
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
        registry.lookup("width").map(|entry| entry.id()),
        Some(PropertyId::Width)
    );
    assert_eq!(registry.lookup("zoom"), None);
    assert_eq!(registry.lookup("COLOR"), None);
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
