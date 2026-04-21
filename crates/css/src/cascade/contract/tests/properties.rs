use super::super::{
    CascadeInheritance, CascadePropertyId, CascadePropertyLengthSignPolicy, InitialStyleValue,
};

#[test]
fn supported_property_metadata_matches_current_subset_contract() {
    let expected = [
        (
            CascadePropertyId::BackgroundColor,
            CascadeInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
            CascadePropertyLengthSignPolicy::NotLength,
        ),
        (
            CascadePropertyId::Color,
            CascadeInheritance::Inherited,
            InitialStyleValue::ColorBlack,
            CascadePropertyLengthSignPolicy::NotLength,
        ),
        (
            CascadePropertyId::Display,
            CascadeInheritance::NotInherited,
            InitialStyleValue::DisplayInline,
            CascadePropertyLengthSignPolicy::NotLength,
        ),
        (
            CascadePropertyId::FontSize,
            CascadeInheritance::Inherited,
            InitialStyleValue::FontSizePx16,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::Height,
            CascadeInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::MarginBottom,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::AllowNegative,
        ),
        (
            CascadePropertyId::MarginLeft,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::AllowNegative,
        ),
        (
            CascadePropertyId::MarginRight,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::AllowNegative,
        ),
        (
            CascadePropertyId::MarginTop,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::AllowNegative,
        ),
        (
            CascadePropertyId::MaxWidth,
            CascadeInheritance::NotInherited,
            InitialStyleValue::NoneKeyword,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::MinWidth,
            CascadeInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::PaddingBottom,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::PaddingLeft,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::PaddingRight,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::PaddingTop,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
        (
            CascadePropertyId::Width,
            CascadeInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
            CascadePropertyLengthSignPolicy::NonNegative,
        ),
    ];

    assert_eq!(CascadePropertyId::ALL.len(), expected.len());
    for (index, (property, inheritance, initial, length_sign)) in expected.into_iter().enumerate() {
        assert_eq!(CascadePropertyId::ALL[index], property);
        assert_eq!(
            CascadePropertyId::from_name(property.name()),
            Some(property)
        );

        let metadata = property.metadata();
        assert_eq!(metadata.inheritance, inheritance, "{}", property.name());
        assert_eq!(metadata.initial, initial, "{}", property.name());
        assert_eq!(metadata.length_sign, length_sign, "{}", property.name());
        assert_eq!(property.initial_value(), initial, "{}", property.name());
    }
    assert_eq!(CascadePropertyId::from_name("zoom"), None);
}
