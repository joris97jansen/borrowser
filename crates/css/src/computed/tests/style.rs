use super::support::*;
use super::*;

#[test]
fn computed_style_initial_snapshot_is_total_and_canonical() {
    let style = ComputedStyle::initial();

    let entries = style.entries().collect::<Vec<_>>();
    assert_eq!(entries.len(), PropertyId::ALL.len());
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(entry.property(), PropertyId::ALL[index]);
    }

    assert_eq!(
        style.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "computed-style\n",
            "  background-color: rgba(0, 0, 0, 0)\n",
            "  color: rgba(0, 0, 0, 255)\n",
            "  display: inline\n",
            "  font-size: 16px\n",
            "  height: auto\n",
            "  margin-bottom: 0px\n",
            "  margin-left: 0px\n",
            "  margin-right: 0px\n",
            "  margin-top: 0px\n",
            "  max-width: none\n",
            "  min-width: auto\n",
            "  overflow: visible\n",
            "  padding-bottom: 0px\n",
            "  padding-left: 0px\n",
            "  padding-right: 0px\n",
            "  padding-top: 0px\n",
            "  position: static\n",
            "  width: auto\n",
        )
    );
}

#[test]
fn computed_style_accessors_match_property_entries() {
    let mut builder = builder_with_initials_except(&[
        PropertyId::BackgroundColor,
        PropertyId::Color,
        PropertyId::Display,
        PropertyId::FontSize,
        PropertyId::Height,
        PropertyId::MarginTop,
        PropertyId::MaxWidth,
        PropertyId::MinWidth,
        PropertyId::Overflow,
        PropertyId::Position,
        PropertyId::PaddingLeft,
        PropertyId::Width,
    ]);
    builder
        .record(
            PropertyId::BackgroundColor,
            ComputedValue::Color((3, 4, 5, 6)),
        )
        .expect("background-color");
    builder
        .record(PropertyId::Color, ComputedValue::Color((7, 8, 9, 255)))
        .expect("color");
    builder
        .record(PropertyId::Display, ComputedValue::Display(Display::Block))
        .expect("display");
    builder
        .record(
            PropertyId::FontSize,
            ComputedValue::Length(Length::Px(22.0)),
        )
        .expect("font-size");
    builder
        .record(PropertyId::Height, length_percentage_or_auto_px(30.0))
        .expect("height");
    builder
        .record(
            PropertyId::MarginTop,
            ComputedValue::Length(Length::Px(4.0)),
        )
        .expect("margin-top");
    builder
        .record(PropertyId::MaxWidth, length_percentage_or_none_px(500.0))
        .expect("max-width");
    builder
        .record(
            PropertyId::MinWidth,
            ComputedValue::LengthPercentageOrAuto(None),
        )
        .expect("min-width");
    builder
        .record(
            PropertyId::Overflow,
            ComputedValue::Overflow(Overflow::Hidden),
        )
        .expect("overflow");
    builder
        .record(
            PropertyId::Position,
            ComputedValue::Position(Position::Relative),
        )
        .expect("position");
    builder
        .record(
            PropertyId::PaddingLeft,
            ComputedValue::Length(Length::Px(6.0)),
        )
        .expect("padding-left");
    builder
        .record(PropertyId::Width, length_percentage_or_auto_px(300.0))
        .expect("width");

    let style = builder.build().expect("computed style");

    assert_eq!(
        style.get(PropertyId::BackgroundColor).value(),
        ComputedValue::Color(style.background_color())
    );
    assert_eq!(
        style.get(PropertyId::Color).value(),
        ComputedValue::Color(style.color())
    );
    assert_eq!(
        style.get(PropertyId::Display).value(),
        ComputedValue::Display(style.display())
    );
    assert_eq!(
        style.get(PropertyId::FontSize).value(),
        ComputedValue::Length(style.font_size())
    );
    assert_eq!(
        style.get(PropertyId::Height).value(),
        ComputedValue::LengthPercentageOrAuto(style.height())
    );
    assert_eq!(
        style.get(PropertyId::MarginTop).value(),
        ComputedValue::Length(Length::Px(style.box_metrics().margin_top))
    );
    assert_eq!(
        style.get(PropertyId::MaxWidth).value(),
        ComputedValue::LengthPercentageOrNone(style.max_width())
    );
    assert_eq!(
        style.get(PropertyId::MinWidth).value(),
        ComputedValue::LengthPercentageOrAuto(style.min_width())
    );
    assert_eq!(
        style.get(PropertyId::Overflow).value(),
        ComputedValue::Overflow(style.overflow())
    );
    assert_eq!(
        style.get(PropertyId::Position).value(),
        ComputedValue::Position(style.position())
    );
    assert_eq!(
        style.get(PropertyId::PaddingLeft).value(),
        ComputedValue::Length(Length::Px(style.box_metrics().padding_left))
    );
    assert_eq!(
        style.get(PropertyId::Width).value(),
        ComputedValue::LengthPercentageOrAuto(style.width())
    );
}

#[test]
fn computed_style_with_property_preserves_builder_invariants() {
    let style = ComputedStyle::initial()
        .with_property(
            PropertyId::Color,
            ComputedValue::Color((120, 130, 140, 255)),
        )
        .expect("style update");

    assert_eq!(style.color(), (120, 130, 140, 255));
    assert_eq!(
        style.background_color(),
        ComputedStyle::initial().background_color()
    );
    assert_eq!(style.entries().count(), property_registry().ids().count());

    let error = ComputedStyle::initial()
        .with_property(PropertyId::FontSize, ComputedValue::Color((0, 0, 0, 255)))
        .expect_err("value-kind mismatch must still be rejected");

    assert_eq!(
        error,
        ComputedStyleBuildError::ValueKindMismatch {
            property: PropertyId::FontSize,
            expected: PropertyComputedValueKind::AbsoluteLength,
            actual: ComputedValueDiscriminant::Color,
        }
    );
}

#[test]
fn computed_style_get_round_trips_all_builder_supported_properties_losslessly() {
    let expected = [
        (
            PropertyId::BackgroundColor,
            ComputedValue::Color((1, 2, 3, 4)),
        ),
        (PropertyId::Color, ComputedValue::Color((5, 6, 7, 8))),
        (PropertyId::Display, ComputedValue::Display(Display::Block)),
        (PropertyId::FontSize, ComputedValue::Length(Length::Px(9.0))),
        (PropertyId::Height, length_percentage_or_auto_px(10.0)),
        (
            PropertyId::MarginBottom,
            ComputedValue::Length(Length::Px(11.0)),
        ),
        (
            PropertyId::MarginLeft,
            ComputedValue::Length(Length::Px(12.0)),
        ),
        (
            PropertyId::MarginRight,
            ComputedValue::Length(Length::Px(13.0)),
        ),
        (
            PropertyId::MarginTop,
            ComputedValue::Length(Length::Px(14.0)),
        ),
        (PropertyId::MaxWidth, length_percentage_or_none_px(15.0)),
        (PropertyId::MinWidth, length_percentage_or_auto_px(16.0)),
        (
            PropertyId::Overflow,
            ComputedValue::Overflow(Overflow::Scroll),
        ),
        (
            PropertyId::PaddingBottom,
            ComputedValue::Length(Length::Px(17.0)),
        ),
        (
            PropertyId::PaddingLeft,
            ComputedValue::Length(Length::Px(18.0)),
        ),
        (
            PropertyId::PaddingRight,
            ComputedValue::Length(Length::Px(19.0)),
        ),
        (
            PropertyId::PaddingTop,
            ComputedValue::Length(Length::Px(20.0)),
        ),
        (
            PropertyId::Position,
            ComputedValue::Position(Position::Sticky),
        ),
        (PropertyId::Width, length_percentage_or_auto_px(21.0)),
    ];

    let mut builder = builder_with_initials_except(PropertyId::ALL.as_slice());
    for (property, value) in expected {
        builder.record(property, value).unwrap_or_else(|error| {
            panic!(
                "failed to record test value for '{}': {error}",
                property.name()
            )
        });
    }
    let style = builder.build().expect("computed style");

    for (property, value) in expected {
        assert_eq!(style.get(property).property(), property);
        assert_eq!(style.get(property).value(), value, "{}", property.name());
    }
}

fn length_percentage_or_auto_px(value: f32) -> ComputedValue {
    ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Length(Length::Px(value))))
}

fn length_percentage_or_none_px(value: f32) -> ComputedValue {
    ComputedValue::LengthPercentageOrNone(Some(LengthPercentage::Length(Length::Px(value))))
}
