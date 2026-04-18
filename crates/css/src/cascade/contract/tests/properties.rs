use super::super::{CascadeInheritance, CascadePropertyId, InitialStyleValue};

#[test]
fn supported_property_metadata_matches_current_subset_contract() {
    let expected = [
        (
            CascadePropertyId::BackgroundColor,
            CascadeInheritance::NotInherited,
            InitialStyleValue::TransparentColor,
        ),
        (
            CascadePropertyId::Color,
            CascadeInheritance::Inherited,
            InitialStyleValue::ColorBlack,
        ),
        (
            CascadePropertyId::Display,
            CascadeInheritance::NotInherited,
            InitialStyleValue::DisplayInline,
        ),
        (
            CascadePropertyId::FontSize,
            CascadeInheritance::Inherited,
            InitialStyleValue::FontSizePx16,
        ),
        (
            CascadePropertyId::Height,
            CascadeInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
        ),
        (
            CascadePropertyId::MarginBottom,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::MarginLeft,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::MarginRight,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::MarginTop,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::MaxWidth,
            CascadeInheritance::NotInherited,
            InitialStyleValue::NoneKeyword,
        ),
        (
            CascadePropertyId::MinWidth,
            CascadeInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
        ),
        (
            CascadePropertyId::PaddingBottom,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::PaddingLeft,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::PaddingRight,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::PaddingTop,
            CascadeInheritance::NotInherited,
            InitialStyleValue::ZeroPx,
        ),
        (
            CascadePropertyId::Width,
            CascadeInheritance::NotInherited,
            InitialStyleValue::AutoKeyword,
        ),
    ];

    assert_eq!(CascadePropertyId::ALL.len(), expected.len());
    for (index, (property, inheritance, initial)) in expected.into_iter().enumerate() {
        assert_eq!(CascadePropertyId::ALL[index], property);
        assert_eq!(
            CascadePropertyId::from_name(property.name()),
            Some(property)
        );

        let metadata = property.metadata();
        assert_eq!(metadata.inheritance, inheritance, "{}", property.name());
        assert_eq!(metadata.initial, initial, "{}", property.name());
        assert_eq!(property.initial_value(), initial, "{}", property.name());
    }
    assert_eq!(CascadePropertyId::from_name("zoom"), None);
}
