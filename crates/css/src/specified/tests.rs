use super::{
    SpecifiedBorderStyleKeyword, SpecifiedColorKeyword, SpecifiedColorSyntax,
    SpecifiedDisplayKeyword, SpecifiedLengthPercentage, SpecifiedLengthPercentageOrAuto,
    SpecifiedLengthUnit, SpecifiedOverflowKeyword, SpecifiedPositionKeyword, SpecifiedValue,
    SpecifiedValueLimits, SpecifiedValueParseErrorKind, parse_specified_value,
    parse_specified_value_with_limits,
};
use crate::{
    ParseOptions, PropertyId, PropertySpecifiedValueKind, Rule, parse_stylesheet_with_options,
    property_registry,
};

fn declaration_value(css_declaration: &str) -> crate::DeclarationValue {
    let parse = parse_stylesheet_with_options(
        &format!("div {{ {css_declaration}; }}"),
        &ParseOptions::stylesheet(),
    );
    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected style rule");
    };

    rule.declarations.declarations[0].value.clone()
}

fn parse(property: PropertyId, css_declaration: &str) -> super::SpecifiedPropertyValue {
    parse_specified_value(property, &declaration_value(css_declaration))
        .unwrap_or_else(|error| panic!("failed to parse {css_declaration:?}: {error}"))
}

fn parse_error(property: PropertyId, css_declaration: &str) -> SpecifiedValueParseErrorKind {
    parse_specified_value(property, &declaration_value(css_declaration))
        .expect_err("specified value must be rejected")
        .kind()
}

#[test]
fn parses_representative_property_aware_specified_values() {
    let color = parse(PropertyId::Color, "color: RED");
    assert_eq!(color.property(), PropertyId::Color);
    assert_eq!(color.kind(), PropertySpecifiedValueKind::Color);
    assert_eq!(color.to_css_text(), "red");
    let SpecifiedValue::Color(color) = color.value() else {
        panic!("expected color");
    };
    assert_eq!(
        color.syntax(),
        &SpecifiedColorSyntax::Keyword(SpecifiedColorKeyword::Red)
    );

    let background = parse(PropertyId::BackgroundColor, "background-color: #Aa00FF");
    assert_eq!(background.to_css_text(), "#aa00ff");
    let SpecifiedValue::Color(background) = background.value() else {
        panic!("expected background color");
    };
    let SpecifiedColorSyntax::Hex(hex) = background.syntax() else {
        panic!("expected hex color");
    };
    assert_eq!(hex.digits(), "aa00ff");
    assert_eq!(hex.rgba(), (170, 0, 255, 255));

    let border_style = parse(PropertyId::BorderTopStyle, "border-top-style: SOLID");
    let SpecifiedValue::BorderStyle(border_style) = border_style.value() else {
        panic!("expected border style");
    };
    assert_eq!(border_style.keyword(), SpecifiedBorderStyleKeyword::Solid);
    assert_eq!(border_style.to_css_text(), "solid");

    let display = parse(PropertyId::Display, "display: inline-block");
    let SpecifiedValue::Display(display) = display.value() else {
        panic!("expected display");
    };
    assert_eq!(display.keyword(), SpecifiedDisplayKeyword::InlineBlock);
    assert_eq!(display.to_css_text(), "inline-block");

    let display_flex = parse(PropertyId::Display, "display: FLEX");
    let SpecifiedValue::Display(display_flex) = display_flex.value() else {
        panic!("expected display");
    };
    assert_eq!(display_flex.keyword(), SpecifiedDisplayKeyword::Flex);
    assert_eq!(display_flex.to_css_text(), "flex");

    let margin = parse(PropertyId::MarginLeft, "margin-left: -4.5px");
    let SpecifiedValue::Length(length) = margin.value() else {
        panic!("expected length");
    };
    assert_eq!(length.number(), "-4.5");
    assert_eq!(length.numeric_value(), -4.5);
    assert_eq!(length.unit(), SpecifiedLengthUnit::Px);
    assert_eq!(margin.to_css_text(), "-4.5px");

    let width = parse(PropertyId::Width, "width: auto");
    assert_eq!(width.to_css_text(), "auto");

    let max_width = parse(PropertyId::MaxWidth, "max-width: none");
    assert_eq!(max_width.to_css_text(), "none");

    let overflow = parse(PropertyId::Overflow, "overflow: HIDDEN");
    let SpecifiedValue::Overflow(overflow) = overflow.value() else {
        panic!("expected overflow");
    };
    assert_eq!(overflow.keyword(), SpecifiedOverflowKeyword::Hidden);
    assert_eq!(overflow.to_css_text(), "hidden");

    let position = parse(PropertyId::Position, "position: ReLaTiVe");
    let SpecifiedValue::Position(position) = position.value() else {
        panic!("expected position");
    };
    assert_eq!(position.keyword(), SpecifiedPositionKeyword::Relative);
    assert_eq!(position.to_css_text(), "relative");
}

#[test]
fn parses_unitless_zero_as_specified_length_without_computing_it() {
    let width = parse(PropertyId::Width, "width: 0");
    let SpecifiedValue::LengthPercentageOrAuto(SpecifiedLengthPercentageOrAuto::LengthPercentage(
        SpecifiedLengthPercentage::Length(length),
    )) = width.value()
    else {
        panic!("expected length-percentage-or-auto length");
    };

    assert_eq!(length.number(), "0");
    assert_eq!(length.numeric_value(), 0.0);
    assert_eq!(length.unit(), SpecifiedLengthUnit::UnitlessZero);
    assert_eq!(width.to_css_text(), "0");
}

#[test]
fn parses_percentages_for_supported_sizing_properties_without_resolving_them() {
    let width = parse(PropertyId::Width, "width: 50%");
    let SpecifiedValue::LengthPercentageOrAuto(SpecifiedLengthPercentageOrAuto::LengthPercentage(
        SpecifiedLengthPercentage::Percentage(percentage),
    )) = width.value()
    else {
        panic!("expected width percentage");
    };

    assert_eq!(percentage.number(), "50");
    assert_eq!(percentage.numeric_value(), 50.0);
    assert_eq!(width.to_css_text(), "50%");

    let max_width = parse(PropertyId::MaxWidth, "max-width: 25%");
    assert_eq!(max_width.to_css_text(), "25%");
}

#[test]
fn rejects_values_that_do_not_match_the_property_specified_shape() {
    assert_eq!(
        parse_error(PropertyId::Display, "display: grid"),
        SpecifiedValueParseErrorKind::UnsupportedDisplayKeyword
    );
    assert_eq!(
        parse_error(PropertyId::Display, "display: inline-flex"),
        SpecifiedValueParseErrorKind::UnsupportedDisplayKeyword
    );
    assert_eq!(
        parse_error(PropertyId::BorderTopStyle, "border-top-style: dashed"),
        SpecifiedValueParseErrorKind::UnsupportedKeyword
    );
    assert_eq!(
        parse_error(PropertyId::Width, "width: none"),
        SpecifiedValueParseErrorKind::UnsupportedKeyword
    );
    assert_eq!(
        parse_error(PropertyId::MaxWidth, "max-width: auto"),
        SpecifiedValueParseErrorKind::UnsupportedKeyword
    );
    assert_eq!(
        parse_error(PropertyId::Width, "width: -1px"),
        SpecifiedValueParseErrorKind::NegativeLengthNotAllowed
    );
    assert_eq!(
        parse_error(PropertyId::Width, "width: 1"),
        SpecifiedValueParseErrorKind::NonZeroUnitlessLength
    );
    assert_eq!(
        parse_error(PropertyId::Color, "color: rgb(1, 2, 3)"),
        SpecifiedValueParseErrorKind::UnsupportedComponent
    );
    assert_eq!(
        parse_error(PropertyId::Color, "color: #abcd"),
        SpecifiedValueParseErrorKind::InvalidHexColor
    );
    assert_eq!(
        parse_error(PropertyId::Color, "color: red blue"),
        SpecifiedValueParseErrorKind::UnexpectedComponentCount
    );
    assert_eq!(
        parse_error(PropertyId::Overflow, "overflow: overlay"),
        SpecifiedValueParseErrorKind::UnsupportedOverflowKeyword
    );
    assert_eq!(
        parse_error(PropertyId::Position, "position: center"),
        SpecifiedValueParseErrorKind::UnsupportedPositionKeyword
    );
}

#[test]
fn specified_value_parser_enforces_component_limits_before_shape_parsing() {
    let mut value = declaration_value("color: red");
    let component = value
        .components
        .first()
        .expect("representative value component")
        .clone();
    value.components = vec![component; 3];

    let error = parse_specified_value_with_limits(
        PropertyId::Color,
        &value,
        &SpecifiedValueLimits {
            max_components_per_value: 2,
        },
    )
    .expect_err("specified value parser must reject over-limit component vectors");

    assert_eq!(
        error.kind(),
        SpecifiedValueParseErrorKind::ResourceLimitExceeded
    );
}

#[test]
fn supported_property_metadata_matches_emitted_specified_value_kinds() {
    let representative = [
        (PropertyId::BackgroundColor, "background-color: transparent"),
        (PropertyId::BorderBottomColor, "border-bottom-color: red"),
        (PropertyId::BorderBottomStyle, "border-bottom-style: solid"),
        (PropertyId::BorderBottomWidth, "border-bottom-width: 1px"),
        (PropertyId::BorderLeftColor, "border-left-color: green"),
        (PropertyId::BorderLeftStyle, "border-left-style: none"),
        (PropertyId::BorderLeftWidth, "border-left-width: 2px"),
        (PropertyId::BorderRightColor, "border-right-color: blue"),
        (PropertyId::BorderRightStyle, "border-right-style: solid"),
        (PropertyId::BorderRightWidth, "border-right-width: 3px"),
        (PropertyId::BorderTopColor, "border-top-color: black"),
        (PropertyId::BorderTopStyle, "border-top-style: solid"),
        (PropertyId::BorderTopWidth, "border-top-width: 4px"),
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
        let value = parse(property, declaration);
        assert_eq!(
            value.kind(),
            property.metadata().specified_value,
            "{}",
            property.name()
        );
    }
}
