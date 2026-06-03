use super::support::*;
use super::*;

#[test]
fn computed_value_normalizes_specified_colors_to_rgba() {
    assert_eq!(
        normalized_value(PropertyId::Color, "color: RED"),
        ComputedValue::Color((255, 0, 0, 255))
    );
    assert_eq!(
        normalized_value(PropertyId::BackgroundColor, "background-color: transparent"),
        ComputedValue::Color((0, 0, 0, 0))
    );
    assert_eq!(
        normalized_value(PropertyId::Color, "color: #0fA"),
        ComputedValue::Color((0, 255, 170, 255))
    );
    assert_eq!(
        normalized_value(PropertyId::Color, "color: #1122cc"),
        ComputedValue::Color((17, 34, 204, 255))
    );
}

#[test]
fn computed_value_normalizes_display_keywords_to_runtime_enum() {
    assert_eq!(
        normalized_value(PropertyId::Display, "display: inline-block"),
        ComputedValue::Display(Display::InlineBlock)
    );
    assert_eq!(
        normalized_value(PropertyId::Display, "display: none"),
        ComputedValue::Display(Display::None)
    );
    assert_eq!(
        normalized_value(PropertyId::Display, "display: flex"),
        ComputedValue::Display(Display::Flex)
    );
}

#[test]
fn computed_value_normalizes_overflow_keywords_to_runtime_enum() {
    assert_eq!(
        normalized_value(PropertyId::Overflow, "overflow: hidden"),
        ComputedValue::Overflow(Overflow::Hidden)
    );
    assert_eq!(
        normalized_value(PropertyId::Overflow, "overflow: clip"),
        ComputedValue::Overflow(Overflow::Clip)
    );
}

#[test]
fn computed_value_normalizes_position_keywords_to_runtime_enum() {
    assert_eq!(
        normalized_value(PropertyId::Position, "position: relative"),
        ComputedValue::Position(Position::Relative)
    );
    assert_eq!(
        normalized_value(PropertyId::Position, "position: absolute"),
        ComputedValue::Position(Position::Absolute)
    );
}

#[test]
fn computed_value_normalizes_lengths_to_css_px() {
    assert_eq!(
        normalized_value(PropertyId::FontSize, "font-size: 16px"),
        ComputedValue::Length(Length::Px(16.0))
    );
    assert_eq!(
        normalized_value(PropertyId::MarginLeft, "margin-left: -4.5px"),
        ComputedValue::Length(Length::Px(-4.5))
    );
    assert_eq!(
        normalized_value(PropertyId::Width, "width: 0"),
        ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Length(Length::Px(0.0))))
    );
    assert_eq!(
        normalized_value(PropertyId::MarginTop, "margin-top: -0px"),
        ComputedValue::Length(Length::Px(0.0))
    );
}

#[test]
fn computed_value_preserves_auto_and_none_branches() {
    assert_eq!(
        normalized_value(PropertyId::Width, "width: auto"),
        ComputedValue::LengthPercentageOrAuto(None)
    );
    assert_eq!(
        normalized_value(PropertyId::Height, "height: 25px"),
        ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Length(Length::Px(25.0))))
    );
    assert_eq!(
        normalized_value(PropertyId::MaxWidth, "max-width: none"),
        ComputedValue::LengthPercentageOrNone(None)
    );
    assert_eq!(
        normalized_value(PropertyId::MaxWidth, "max-width: 40px"),
        ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Length(Length::Px(40.0))))
    );
}

#[test]
fn computed_value_preserves_length_percentages_for_layout_resolution() {
    assert_eq!(
        normalized_value(PropertyId::Width, "width: 50%"),
        ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Percentage(
            Percentage::from_percent(50.0).expect("finite percentage"),
        )))
    );
    assert_eq!(
        normalized_value(PropertyId::MaxWidth, "max-width: 25%"),
        ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Percentage(
            Percentage::from_percent(25.0).expect("finite percentage"),
        )))
    );
}

#[test]
fn computed_value_normalization_matches_property_metadata_for_supported_subset() {
    let representative = [
        (PropertyId::BackgroundColor, "background-color: transparent"),
        (PropertyId::Color, "color: black"),
        (PropertyId::Display, "display: block"),
        (PropertyId::FontSize, "font-size: 16px"),
        (PropertyId::Height, "height: auto"),
        (PropertyId::MarginBottom, "margin-bottom: 1px"),
        (PropertyId::MarginLeft, "margin-left: 1px"),
        (PropertyId::MarginRight, "margin-right: 1px"),
        (PropertyId::MarginTop, "margin-top: 1px"),
        (PropertyId::MaxWidth, "max-width: none"),
        (PropertyId::MinWidth, "min-width: auto"),
        (PropertyId::Overflow, "overflow: visible"),
        (PropertyId::PaddingBottom, "padding-bottom: 1px"),
        (PropertyId::PaddingLeft, "padding-left: 1px"),
        (PropertyId::PaddingRight, "padding-right: 1px"),
        (PropertyId::PaddingTop, "padding-top: 1px"),
        (PropertyId::Position, "position: static"),
        (PropertyId::Width, "width: auto"),
    ];

    for property in property_registry().ids() {
        let (_, declaration) = representative
            .iter()
            .copied()
            .find(|(candidate, _)| *candidate == property)
            .unwrap_or_else(|| panic!("missing representative for {}", property.name()));
        assert_eq!(
            normalized_value(property, declaration).discriminant(),
            computed_value_discriminant(property.metadata().computed_value),
            "{}",
            property.name()
        );
    }
}

#[test]
fn computed_value_normalization_reports_length_out_of_runtime_range() {
    let error = normalize_specified_value(&specified_value(PropertyId::Width, "width: 1e39px"))
        .expect_err("length too large for current runtime scalar must be rejected");

    assert_eq!(error.property(), PropertyId::Width);
    assert_eq!(
        error.kind(),
        ComputedValueNormalizationErrorKind::LengthOutOfRange
    );
}

#[test]
fn computed_value_normalization_reports_metadata_value_kind_mismatch() {
    let color_value = specified_value(PropertyId::Color, "color: red")
        .value()
        .clone();
    let mismatched = SpecifiedPropertyValue::from_parts_for_test(PropertyId::Display, color_value);

    let error = normalize_specified_value(&mismatched)
        .expect_err("metadata/value mismatch must be rejected");

    assert_eq!(error.property(), PropertyId::Display);
    assert_eq!(
        error.kind(),
        ComputedValueNormalizationErrorKind::ValueKindMismatch {
            expected: PropertyComputedValueKind::DisplayKeyword,
            actual: ComputedValueDiscriminant::Color,
        }
    );
}
