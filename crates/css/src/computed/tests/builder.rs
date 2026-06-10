use super::support::*;
use super::*;

#[test]
fn computed_style_builder_materializes_structured_fields_from_property_entries() {
    let mut builder = builder_with_initials_except(&[
        PropertyId::Color,
        PropertyId::BorderTopColor,
        PropertyId::BorderTopStyle,
        PropertyId::BorderTopWidth,
        PropertyId::MarginTop,
        PropertyId::Width,
    ]);
    builder
        .record(PropertyId::Color, ComputedValue::Color((12, 34, 56, 255)))
        .expect("color");
    builder
        .record(
            PropertyId::BorderTopColor,
            ComputedValue::Color((200, 10, 20, 255)),
        )
        .expect("border-top-color");
    builder
        .record(
            PropertyId::BorderTopStyle,
            ComputedValue::BorderStyle(BorderStyle::Solid),
        )
        .expect("border-top-style");
    builder
        .record(
            PropertyId::BorderTopWidth,
            ComputedValue::Length(Length::Px(3.0)),
        )
        .expect("border-top-width");
    builder
        .record(
            PropertyId::MarginTop,
            ComputedValue::Length(Length::Px(18.0)),
        )
        .expect("margin-top");
    builder
        .record(
            PropertyId::Width,
            ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Length(Length::Px(
                320.0,
            )))),
        )
        .expect("width");

    let style = builder.build().expect("computed style");

    assert_eq!(style.color(), (12, 34, 56, 255));
    assert_eq!(style.border_edges().top.style, BorderStyle::Solid);
    assert_eq!(style.border_edges().top.color, (200, 10, 20, 255));
    assert_eq!(style.box_metrics().border_top, 3.0);
    assert_eq!(style.box_metrics().margin_top, 18.0);
    assert_eq!(
        style.width(),
        Some(LengthPercentage::Length(Length::Px(320.0)))
    );
    assert_eq!(
        style.get(PropertyId::Width).value(),
        ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Length(Length::Px(320.0))))
    );
}

#[test]
fn computed_style_builder_rejects_duplicate_property_records() {
    let mut builder = builder_with_initials_except(&[PropertyId::Color]);
    builder
        .record(PropertyId::Color, ComputedValue::Color((0, 0, 0, 255)))
        .expect("first color");

    let error = builder
        .record(PropertyId::Color, ComputedValue::Color((255, 0, 0, 255)))
        .expect_err("duplicate property must be rejected");

    assert_eq!(
        error,
        ComputedStyleBuildError::DuplicateProperty {
            property: PropertyId::Color,
        }
    );
}

#[test]
fn computed_style_builder_rejects_value_kind_mismatches() {
    let mut builder = builder_with_initials_except(&[PropertyId::Display]);

    let error = builder
        .record(PropertyId::Display, ComputedValue::Color((0, 0, 0, 255)))
        .expect_err("value kind mismatch must be rejected");

    assert_eq!(
        error,
        ComputedStyleBuildError::ValueKindMismatch {
            property: PropertyId::Display,
            expected: crate::PropertyComputedValueKind::DisplayKeyword,
            actual: ComputedValueDiscriminant::Color,
        }
    );
}

#[test]
fn computed_style_builder_requires_total_property_fill() {
    let mut builder = builder_with_initials_except(PropertyId::ALL.as_slice());
    builder
        .record(PropertyId::Color, ComputedValue::Color((0, 0, 0, 255)))
        .expect("color");

    let error = builder
        .build()
        .expect_err("missing properties must be rejected");
    let ComputedStyleBuildError::MissingProperties { missing_properties } = error else {
        panic!("expected missing-properties error");
    };

    assert_eq!(missing_properties.len(), PropertyId::ALL.len() - 1);
    assert!(!missing_properties.contains(&PropertyId::Color));
    assert!(missing_properties.contains(&PropertyId::Display));
}
