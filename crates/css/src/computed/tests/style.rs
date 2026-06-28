use super::support::*;
use super::*;
use crate::{BorderSide, ComputedStyleLayoutImpact, computed::Outline};

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
            "  border-bottom-color: rgba(0, 0, 0, 0)\n",
            "  border-bottom-style: none\n",
            "  border-bottom-width: 0px\n",
            "  border-left-color: rgba(0, 0, 0, 0)\n",
            "  border-left-style: none\n",
            "  border-left-width: 0px\n",
            "  border-right-color: rgba(0, 0, 0, 0)\n",
            "  border-right-style: none\n",
            "  border-right-width: 0px\n",
            "  border-top-color: rgba(0, 0, 0, 0)\n",
            "  border-top-style: none\n",
            "  border-top-width: 0px\n",
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
            "  outline-color: rgba(0, 0, 0, 0)\n",
            "  outline-style: none\n",
            "  outline-width: 0px\n",
            "  padding-bottom: 0px\n",
            "  padding-left: 0px\n",
            "  padding-right: 0px\n",
            "  padding-top: 0px\n",
            "  position: static\n",
            "  text-decoration-line: none\n",
            "  width: auto\n",
            "  z-index: auto\n",
        )
    );
}

#[test]
fn border_and_outline_helpers_expose_computed_width_contribution_semantics() {
    let transparent_solid_border = BorderSide {
        width: 4.0,
        style: BorderStyle::Solid,
        color: (10, 20, 30, 0),
    };
    assert!(transparent_solid_border.has_computed_width_contribution());
    assert_eq!(transparent_solid_border.computed_width_contribution(), 4.0);
    assert!(!transparent_solid_border.is_paint_visible());

    let opaque_solid_border = BorderSide {
        width: 4.0,
        style: BorderStyle::Solid,
        color: (10, 20, 30, 255),
    };
    assert!(opaque_solid_border.has_computed_width_contribution());
    assert_eq!(opaque_solid_border.computed_width_contribution(), 4.0);
    assert!(opaque_solid_border.is_paint_visible());

    let none_border = BorderSide {
        width: 4.0,
        style: BorderStyle::None,
        color: (10, 20, 30, 255),
    };
    assert!(!none_border.has_computed_width_contribution());
    assert_eq!(none_border.computed_width_contribution(), 0.0);
    assert!(!none_border.is_paint_visible());

    let zero_solid_border = BorderSide {
        width: 0.0,
        style: BorderStyle::Solid,
        color: (10, 20, 30, 255),
    };
    assert!(!zero_solid_border.has_computed_width_contribution());
    assert_eq!(zero_solid_border.computed_width_contribution(), 0.0);
    assert!(!zero_solid_border.is_paint_visible());

    let transparent_solid_outline = Outline {
        width: 3.0,
        style: OutlineStyle::Solid,
        color: (40, 50, 60, 0),
    };
    assert!(transparent_solid_outline.has_computed_width_contribution());
    assert!(!transparent_solid_outline.is_paint_visible());

    let none_outline = Outline {
        width: 3.0,
        style: OutlineStyle::None,
        color: (40, 50, 60, 255),
    };
    assert!(!none_outline.has_computed_width_contribution());
    assert!(!none_outline.is_paint_visible());
}

#[test]
fn computed_style_accessors_match_property_entries() {
    let mut builder = builder_with_initials_except(&[
        PropertyId::BackgroundColor,
        PropertyId::BorderTopColor,
        PropertyId::BorderTopStyle,
        PropertyId::BorderTopWidth,
        PropertyId::Color,
        PropertyId::Display,
        PropertyId::FontSize,
        PropertyId::Height,
        PropertyId::MarginTop,
        PropertyId::MaxWidth,
        PropertyId::MinWidth,
        PropertyId::Overflow,
        PropertyId::OutlineColor,
        PropertyId::OutlineStyle,
        PropertyId::OutlineWidth,
        PropertyId::Position,
        PropertyId::PaddingLeft,
        PropertyId::TextDecorationLine,
        PropertyId::Width,
        PropertyId::ZIndex,
    ]);
    builder
        .record(
            PropertyId::BackgroundColor,
            ComputedValue::Color((3, 4, 5, 6)),
        )
        .expect("background-color");
    builder
        .record(
            PropertyId::BorderTopColor,
            ComputedValue::Color((30, 40, 50, 255)),
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
            ComputedValue::Length(Length::Px(2.0)),
        )
        .expect("border-top-width");
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
            PropertyId::OutlineColor,
            ComputedValue::Color((11, 12, 13, 255)),
        )
        .expect("outline-color");
    builder
        .record(
            PropertyId::OutlineStyle,
            ComputedValue::OutlineStyle(OutlineStyle::Solid),
        )
        .expect("outline-style");
    builder
        .record(
            PropertyId::OutlineWidth,
            ComputedValue::Length(Length::Px(5.0)),
        )
        .expect("outline-width");
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
        .record(
            PropertyId::TextDecorationLine,
            ComputedValue::TextDecorationLine(TextDecorationLine::Underline),
        )
        .expect("text-decoration-line");
    builder
        .record(PropertyId::Width, length_percentage_or_auto_px(300.0))
        .expect("width");
    builder
        .record(
            PropertyId::ZIndex,
            ComputedValue::ZIndex(ZIndex::Integer(7)),
        )
        .expect("z-index");

    let style = builder.build().expect("computed style");

    assert_eq!(
        style.get(PropertyId::BackgroundColor).value(),
        ComputedValue::Color(style.background_color())
    );
    assert_eq!(
        style.get(PropertyId::BorderTopColor).value(),
        ComputedValue::Color(style.border_edges().top.color)
    );
    assert_eq!(
        style.get(PropertyId::BorderTopStyle).value(),
        ComputedValue::BorderStyle(style.border_edges().top.style)
    );
    assert_eq!(
        style.get(PropertyId::BorderTopWidth).value(),
        ComputedValue::Length(Length::Px(style.border_edges().top.width))
    );
    assert_eq!(style.box_metrics().border_top, 2.0);
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
        style.get(PropertyId::OutlineColor).value(),
        ComputedValue::Color(style.outline().color)
    );
    assert_eq!(
        style.get(PropertyId::OutlineStyle).value(),
        ComputedValue::OutlineStyle(style.outline().style)
    );
    assert_eq!(
        style.get(PropertyId::OutlineWidth).value(),
        ComputedValue::Length(Length::Px(style.outline().width))
    );
    assert_eq!(style.outline().width, 5.0);
    assert_eq!(style.box_metrics().border_top, 2.0);
    assert_eq!(
        style.get(PropertyId::Position).value(),
        ComputedValue::Position(style.position())
    );
    assert_eq!(
        style.get(PropertyId::PaddingLeft).value(),
        ComputedValue::Length(Length::Px(style.box_metrics().padding_left))
    );
    assert_eq!(
        style.get(PropertyId::TextDecorationLine).value(),
        ComputedValue::TextDecorationLine(style.text_decoration_line())
    );
    assert_eq!(style.text_decoration_line(), TextDecorationLine::Underline);
    assert_eq!(
        style.get(PropertyId::Width).value(),
        ComputedValue::LengthPercentageOrAuto(style.width())
    );
    assert_eq!(
        style.get(PropertyId::ZIndex).value(),
        ComputedValue::ZIndex(style.z_index())
    );
    assert_eq!(style.z_index(), ZIndex::Integer(7));
}

#[test]
fn computed_style_layout_impact_is_owned_by_computed_style() {
    let base = ComputedStyle::initial();
    let paint_only = base
        .with_property(
            PropertyId::BackgroundColor,
            ComputedValue::Color((20, 30, 40, 255)),
        )
        .expect("background color update");
    let layout_affecting = base
        .with_property(PropertyId::Width, length_percentage_or_auto_px(120.0))
        .expect("width update");

    assert_eq!(
        paint_only.layout_impact_against(&base),
        ComputedStyleLayoutImpact::PaintOnly
    );
    assert_eq!(
        layout_affecting.layout_impact_against(&base),
        ComputedStyleLayoutImpact::LayoutAffecting
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
        (
            PropertyId::BorderBottomColor,
            ComputedValue::Color((10, 20, 30, 255)),
        ),
        (
            PropertyId::BorderBottomStyle,
            ComputedValue::BorderStyle(BorderStyle::Solid),
        ),
        (
            PropertyId::BorderBottomWidth,
            ComputedValue::Length(Length::Px(1.0)),
        ),
        (
            PropertyId::BorderLeftColor,
            ComputedValue::Color((40, 50, 60, 255)),
        ),
        (
            PropertyId::BorderLeftStyle,
            ComputedValue::BorderStyle(BorderStyle::None),
        ),
        (
            PropertyId::BorderLeftWidth,
            ComputedValue::Length(Length::Px(2.0)),
        ),
        (
            PropertyId::BorderRightColor,
            ComputedValue::Color((70, 80, 90, 255)),
        ),
        (
            PropertyId::BorderRightStyle,
            ComputedValue::BorderStyle(BorderStyle::Solid),
        ),
        (
            PropertyId::BorderRightWidth,
            ComputedValue::Length(Length::Px(3.0)),
        ),
        (
            PropertyId::BorderTopColor,
            ComputedValue::Color((100, 110, 120, 255)),
        ),
        (
            PropertyId::BorderTopStyle,
            ComputedValue::BorderStyle(BorderStyle::Solid),
        ),
        (
            PropertyId::BorderTopWidth,
            ComputedValue::Length(Length::Px(4.0)),
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
            PropertyId::OutlineColor,
            ComputedValue::Color((22, 23, 24, 255)),
        ),
        (
            PropertyId::OutlineStyle,
            ComputedValue::OutlineStyle(OutlineStyle::Solid),
        ),
        (
            PropertyId::OutlineWidth,
            ComputedValue::Length(Length::Px(25.0)),
        ),
        (
            PropertyId::PaddingBottom,
            ComputedValue::Length(Length::Px(26.0)),
        ),
        (
            PropertyId::PaddingLeft,
            ComputedValue::Length(Length::Px(27.0)),
        ),
        (
            PropertyId::PaddingRight,
            ComputedValue::Length(Length::Px(28.0)),
        ),
        (
            PropertyId::PaddingTop,
            ComputedValue::Length(Length::Px(29.0)),
        ),
        (
            PropertyId::Position,
            ComputedValue::Position(Position::Sticky),
        ),
        (
            PropertyId::TextDecorationLine,
            ComputedValue::TextDecorationLine(TextDecorationLine::Underline),
        ),
        (PropertyId::Width, length_percentage_or_auto_px(30.0)),
        (
            PropertyId::ZIndex,
            ComputedValue::ZIndex(ZIndex::Integer(-2)),
        ),
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
