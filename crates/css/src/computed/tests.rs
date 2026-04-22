use super::{
    ComputedStyle, ComputedStyleBuildError, ComputedStyleBuilder, ComputedStyleResolutionError,
    ComputedValue, ComputedValueDiscriminant, ComputedValueNormalizationErrorKind,
    build_style_tree, build_style_tree_from_computed_styles, build_style_tree_with_stylesheets,
    compute_document_styles, compute_document_styles_from_resolved_styles, compute_style,
    compute_style_from_resolved_style, normalize_specified_value,
};
use crate::{
    InitialStyleValue, ParseOptions, PropertyComputedValueKind, PropertyId, Rule,
    SpecifiedPropertyValue, parse_specified_value, parse_stylesheet_with_options,
    property_registry, resolve_cascade_style_from_rule_inputs, resolve_document_styles,
    resolve_initial_style,
    values::{Display, Length},
};
use html::{Node, internal::Id};
use std::sync::Arc;

fn builder_with_initials_except(skip: &[PropertyId]) -> ComputedStyleBuilder {
    let mut builder = ComputedStyleBuilder::new();
    for property in property_registry().ids() {
        if skip.contains(&property) {
            continue;
        }
        builder
            .record(property, ComputedValue::from_initial(property))
            .expect("initial computed value");
    }
    builder
}

fn specified_value(property: PropertyId, css_declaration: &str) -> crate::SpecifiedPropertyValue {
    let parse = stylesheet(&format!("div {{ {css_declaration}; }}"));
    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected style rule");
    };

    parse_specified_value(property, &rule.declarations.declarations[0].value)
        .unwrap_or_else(|error| panic!("failed to parse {css_declaration:?}: {error}"))
}

fn stylesheet(source: &str) -> crate::StylesheetParse {
    parse_stylesheet_with_options(source, &ParseOptions::stylesheet())
}

fn element(name: &str, attributes: Vec<(&str, Option<&str>)>, children: Vec<Node>) -> Node {
    Node::Element {
        id: Id::INVALID,
        name: Arc::from(name),
        attributes: attributes
            .into_iter()
            .map(|(name, value)| (Arc::from(name), value.map(str::to_string)))
            .collect(),
        style: Vec::new(),
        children,
    }
}

fn normalized_value(property: PropertyId, css_declaration: &str) -> ComputedValue {
    normalize_specified_value(&specified_value(property, css_declaration))
        .unwrap_or_else(|error| panic!("failed to normalize {css_declaration:?}: {error}"))
}

#[test]
fn initial_display_matches_css_initial_value_while_bridge_applies_element_defaults_later() {
    assert!(matches!(
        ComputedStyle::initial().display(),
        Display::Inline
    ));
}

#[test]
fn min_width_auto_clears_previous_length_but_none_is_not_accepted() {
    let style = compute_style(
        Some("div"),
        &[
            ("min-width".to_string(), "10px".to_string()),
            ("min-width".to_string(), "auto".to_string()),
        ],
        None,
    );
    assert!(style.min_width().is_none());

    let style = compute_style(
        Some("div"),
        &[
            ("min-width".to_string(), "10px".to_string()),
            ("min-width".to_string(), "none".to_string()),
        ],
        None,
    );
    assert!(matches!(style.min_width(), Some(Length::Px(px)) if px == 10.0));
}

#[test]
fn max_width_none_clears_previous_length_but_auto_is_not_accepted() {
    let style = compute_style(
        Some("div"),
        &[
            ("max-width".to_string(), "10px".to_string()),
            ("max-width".to_string(), "none".to_string()),
        ],
        None,
    );
    assert!(style.max_width().is_none());

    let style = compute_style(
        Some("div"),
        &[
            ("max-width".to_string(), "10px".to_string()),
            ("max-width".to_string(), "auto".to_string()),
        ],
        None,
    );
    assert!(matches!(style.max_width(), Some(Length::Px(px)) if px == 10.0));
}

#[test]
fn legacy_compute_style_uses_property_pipeline_for_invalid_values() {
    let style = compute_style(
        Some("div"),
        &[
            ("padding-left".to_string(), "8px".to_string()),
            ("padding-left".to_string(), "-4px".to_string()),
            ("margin-left".to_string(), "-3px".to_string()),
            ("width".to_string(), "-1px".to_string()),
        ],
        None,
    );

    assert_eq!(style.box_metrics().padding_left, 8.0);
    assert_eq!(style.box_metrics().margin_left, -3.0);
    assert_eq!(style.width(), None);
}

#[test]
fn legacy_compute_style_ignores_invalid_link_color_for_ua_fallback() {
    let invalid = compute_style(
        Some("a"),
        &[("color".to_string(), "nonsense".to_string())],
        None,
    );
    assert_eq!(invalid.color(), (0, 0, 238, 255));

    let valid = compute_style(Some("a"), &[("color".to_string(), "red".to_string())], None);
    assert_eq!(valid.color(), (255, 0, 0, 255));
}

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
            "  padding-bottom: 0px\n",
            "  padding-left: 0px\n",
            "  padding-right: 0px\n",
            "  padding-top: 0px\n",
            "  width: auto\n",
        )
    );
}

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
        ComputedValue::LengthOrAuto(Some(Length::Px(0.0)))
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
        ComputedValue::LengthOrAuto(None)
    );
    assert_eq!(
        normalized_value(PropertyId::Height, "height: 25px"),
        ComputedValue::LengthOrAuto(Some(Length::Px(25.0)))
    );
    assert_eq!(
        normalized_value(PropertyId::MaxWidth, "max-width: none"),
        ComputedValue::LengthOrNone(None)
    );
    assert_eq!(
        normalized_value(PropertyId::MaxWidth, "max-width: 40px"),
        ComputedValue::LengthOrNone(Some(Length::Px(40.0)))
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
        (PropertyId::PaddingBottom, "padding-bottom: 1px"),
        (PropertyId::PaddingLeft, "padding-left: 1px"),
        (PropertyId::PaddingRight, "padding-right: 1px"),
        (PropertyId::PaddingTop, "padding-top: 1px"),
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
            super::value::computed_value_discriminant(property.metadata().computed_value),
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

#[test]
fn compute_style_from_resolved_style_materializes_cascade_fallbacks() {
    let stylesheets = vec![stylesheet(concat!(
        "section { color: #0f0; width: 40px; }",
        "span { color: nonsense; width: -1px; display: block; }",
    ))];
    let dom = element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );
    let resolved = resolve_document_styles(&dom, &stylesheets);

    let parent = compute_style_from_resolved_style(resolved.entries()[0].style(), None)
        .expect("parent computed style");
    let child = compute_style_from_resolved_style(resolved.entries()[1].style(), Some(&parent))
        .expect("child computed style");

    assert_eq!(parent.color(), (0, 255, 0, 255));
    assert_eq!(parent.width(), Some(Length::Px(40.0)));
    assert_eq!(child.color(), parent.color());
    assert_eq!(child.width(), None);
    assert_eq!(child.box_metrics().padding_left, 0.0);
    assert_eq!(child.display(), Display::Block);
}

#[test]
fn compute_document_styles_integrates_cascade_inheritance_defaults_and_computation() {
    let stylesheets = vec![stylesheet(concat!(
        "section { color: red; font-size: 20px; width: 40px; }",
        "span { color: nonsense; background-color: #0f0; padding-left: 3px; display: inline-block; }",
    ))];
    let dom = element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");
    assert_eq!(computed.entries().len(), 2);
    assert_eq!(computed.entries()[0].selector_element_id().get(), 1);
    assert_eq!(computed.entries()[0].element_name(), "section");
    assert_eq!(computed.entries()[1].selector_element_id().get(), 2);
    assert_eq!(computed.entries()[1].element_name(), "span");

    let section = computed.entries()[0].style();
    assert_eq!(section.color(), (255, 0, 0, 255));
    assert_eq!(section.font_size(), Length::Px(20.0));
    assert_eq!(section.width(), Some(Length::Px(40.0)));
    assert_eq!(section.background_color(), (0, 0, 0, 0));

    let span = computed.entries()[1].style();
    assert_eq!(span.color(), section.color());
    assert_eq!(span.font_size(), section.font_size());
    assert_eq!(span.width(), None);
    assert_eq!(span.background_color(), (0, 255, 0, 255));
    assert_eq!(span.box_metrics().padding_left, 3.0);
    assert_eq!(span.display(), Display::InlineBlock);
}

#[test]
fn computed_document_style_snapshot_is_deterministic() {
    let stylesheets = vec![stylesheet(
        "div { color: blue; width: 12px; } span { margin-left: -2px; }",
    )];
    let dom = element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");

    assert_eq!(
        computed.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "computed-document-style\n",
            "element[0]: selector-id=1 name=\"div\"\n",
            "  background-color: rgba(0, 0, 0, 0)\n",
            "  color: rgba(0, 0, 255, 255)\n",
            "  display: inline\n",
            "  font-size: 16px\n",
            "  height: auto\n",
            "  margin-bottom: 0px\n",
            "  margin-left: 0px\n",
            "  margin-right: 0px\n",
            "  margin-top: 0px\n",
            "  max-width: none\n",
            "  min-width: auto\n",
            "  padding-bottom: 0px\n",
            "  padding-left: 0px\n",
            "  padding-right: 0px\n",
            "  padding-top: 0px\n",
            "  width: 12px\n",
            "element[1]: selector-id=2 name=\"span\"\n",
            "  background-color: rgba(0, 0, 0, 0)\n",
            "  color: rgba(0, 0, 255, 255)\n",
            "  display: inline\n",
            "  font-size: 16px\n",
            "  height: auto\n",
            "  margin-bottom: 0px\n",
            "  margin-left: -2px\n",
            "  margin-right: 0px\n",
            "  margin-top: 0px\n",
            "  max-width: none\n",
            "  min-width: auto\n",
            "  padding-bottom: 0px\n",
            "  padding-left: 0px\n",
            "  padding-right: 0px\n",
            "  padding-top: 0px\n",
            "  width: auto\n",
        )
    );
}

#[test]
fn compute_document_styles_from_resolved_styles_uses_existing_cascade_output() {
    let stylesheets = vec![stylesheet("main { color: teal; } p { font-size: 18px; }")];
    let dom = element(
        "main",
        Vec::new(),
        vec![element("p", Vec::new(), Vec::new())],
    );
    let resolved = resolve_document_styles(&dom, &stylesheets);

    let computed = compute_document_styles_from_resolved_styles(&dom, &resolved).expect("computed");

    assert_eq!(computed.entries()[0].style().color(), (0, 128, 128, 255));
    assert_eq!(computed.entries()[1].style().color(), (0, 128, 128, 255));
    assert_eq!(computed.entries()[1].style().font_size(), Length::Px(18.0));
}

#[test]
fn build_style_tree_with_stylesheets_uses_structured_pipeline_without_mutating_dom() {
    let stylesheets = vec![stylesheet("div { color: blue; } span { width: 5px; }")];
    let dom = element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let styled = build_style_tree_with_stylesheets(&dom, &stylesheets).expect("styled document");

    assert_eq!(styled.style.color(), (0, 0, 255, 255));
    assert_eq!(styled.children[0].style.color(), (0, 0, 255, 255));
    assert_eq!(styled.children[0].style.width(), Some(Length::Px(5.0)));
    let Node::Element {
        style, children, ..
    } = &dom
    else {
        panic!("expected element");
    };
    assert!(style.is_empty());
    let Node::Element {
        style: child_style, ..
    } = &children[0]
    else {
        panic!("expected child element");
    };
    assert!(child_style.is_empty());
}

#[test]
fn legacy_build_style_tree_ignores_invalid_display_for_default_bridge() {
    let dom = Node::Element {
        id: Id::INVALID,
        name: Arc::from("div"),
        attributes: Vec::new(),
        style: vec![("display".to_string(), "nonsense".to_string())],
        children: Vec::new(),
    };

    let styled = build_style_tree(&dom, None);

    assert_eq!(styled.style.display(), Display::Block);
}

#[test]
fn build_style_tree_from_computed_styles_rejects_mismatched_document_style() {
    let source_dom = element("main", Vec::new(), Vec::new());
    let target_dom = element("section", Vec::new(), Vec::new());
    let computed = compute_document_styles(&source_dom, &[]).expect("computed document");

    let error = match build_style_tree_from_computed_styles(&target_dom, &computed) {
        Ok(_) => panic!("mismatched computed document style must be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        ComputedStyleResolutionError::ComputedElementNameMismatch {
            element_index: 0,
            expected: "section".to_string(),
            actual: "main".to_string(),
        }
    );
}

#[test]
fn build_style_tree_from_computed_styles_rejects_selector_identity_mismatch() {
    let dom = element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );
    let mut computed = compute_document_styles(&dom, &[]).expect("computed document");
    let expected = computed.entries[1].selector_element_id;
    let actual = computed.entries[0].selector_element_id;
    computed.entries[1].selector_element_id = actual;

    let error = match build_style_tree_from_computed_styles(&dom, &computed) {
        Ok(_) => panic!("selector identity mismatch must be rejected"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        ComputedStyleResolutionError::ComputedElementIdentityMismatch {
            element_index: 1,
            expected,
            actual,
        }
    );
}

#[test]
fn compute_style_from_resolved_style_rejects_normalization_failures() {
    let stylesheets = vec![stylesheet("div { width: 1e39px; }")];
    let dom = element("div", Vec::new(), Vec::new());
    let resolved = resolve_document_styles(&dom, &stylesheets);

    let error = compute_style_from_resolved_style(resolved.entries()[0].style(), None)
        .expect_err("normalization failure must not produce computed style");

    let ComputedStyleResolutionError::Normalization(error) = error else {
        panic!("expected normalization error");
    };
    assert_eq!(error.property(), PropertyId::Width);
    assert_eq!(
        error.kind(),
        ComputedValueNormalizationErrorKind::LengthOutOfRange
    );
}

#[test]
fn compute_style_from_resolved_style_requires_parent_for_inherited_entries() {
    let parent_resolved = resolve_initial_style();
    let child_resolved = resolve_cascade_style_from_rule_inputs(&[], Some(&parent_resolved));

    let error = compute_style_from_resolved_style(&child_resolved, None)
        .expect_err("inherited entries require parent computed style");

    assert_eq!(
        error,
        ComputedStyleResolutionError::MissingInheritedParent {
            property: PropertyId::Color,
        }
    );
}

#[test]
fn computed_style_method_delegates_to_resolved_style_assembly() {
    let resolved = resolve_initial_style();
    let via_method = ComputedStyle::from_resolved_style(&resolved, None).expect("computed style");
    let via_function = compute_style_from_resolved_style(&resolved, None).expect("computed style");

    assert_eq!(via_method, via_function);
    assert_eq!(
        via_method.get(PropertyId::Display).value(),
        ComputedValue::from_initial(PropertyId::Display)
    );
    assert_eq!(
        via_method.get(PropertyId::Width).value(),
        ComputedValue::from_initial(PropertyId::Width)
    );
    assert_eq!(
        via_method.get(PropertyId::Color).value(),
        ComputedValue::Color((0, 0, 0, 255))
    );
    assert_eq!(
        via_method.get(PropertyId::BackgroundColor).value(),
        ComputedValue::Color((0, 0, 0, 0))
    );
    assert_eq!(
        via_method.get(PropertyId::FontSize).value(),
        ComputedValue::Length(Length::Px(16.0))
    );
    assert_eq!(
        via_method.get(PropertyId::MaxWidth).value(),
        ComputedValue::from_initial(PropertyId::MaxWidth)
    );
    assert_eq!(
        via_method.get(PropertyId::MinWidth).value(),
        ComputedValue::from_initial(PropertyId::MinWidth)
    );
    assert_eq!(
        PropertyId::Display.initial_value(),
        InitialStyleValue::DisplayInline
    );
}

#[test]
fn computed_style_builder_materializes_structured_fields_from_property_entries() {
    let mut builder = builder_with_initials_except(&[
        PropertyId::Color,
        PropertyId::MarginTop,
        PropertyId::Width,
    ]);
    builder
        .record(PropertyId::Color, ComputedValue::Color((12, 34, 56, 255)))
        .expect("color");
    builder
        .record(
            PropertyId::MarginTop,
            ComputedValue::Length(Length::Px(18.0)),
        )
        .expect("margin-top");
    builder
        .record(
            PropertyId::Width,
            ComputedValue::LengthOrAuto(Some(Length::Px(320.0))),
        )
        .expect("width");

    let style = builder.build().expect("computed style");

    assert_eq!(style.color(), (12, 34, 56, 255));
    assert_eq!(style.box_metrics().margin_top, 18.0);
    assert_eq!(style.width(), Some(Length::Px(320.0)));
    assert_eq!(
        style.get(PropertyId::Width).value(),
        ComputedValue::LengthOrAuto(Some(Length::Px(320.0)))
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
        .record(
            PropertyId::Height,
            ComputedValue::LengthOrAuto(Some(Length::Px(30.0))),
        )
        .expect("height");
    builder
        .record(
            PropertyId::MarginTop,
            ComputedValue::Length(Length::Px(4.0)),
        )
        .expect("margin-top");
    builder
        .record(
            PropertyId::MaxWidth,
            ComputedValue::LengthOrNone(Some(Length::Px(500.0))),
        )
        .expect("max-width");
    builder
        .record(PropertyId::MinWidth, ComputedValue::LengthOrAuto(None))
        .expect("min-width");
    builder
        .record(
            PropertyId::PaddingLeft,
            ComputedValue::Length(Length::Px(6.0)),
        )
        .expect("padding-left");
    builder
        .record(
            PropertyId::Width,
            ComputedValue::LengthOrAuto(Some(Length::Px(300.0))),
        )
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
        ComputedValue::LengthOrAuto(style.height())
    );
    assert_eq!(
        style.get(PropertyId::MarginTop).value(),
        ComputedValue::Length(Length::Px(style.box_metrics().margin_top))
    );
    assert_eq!(
        style.get(PropertyId::MaxWidth).value(),
        ComputedValue::LengthOrNone(style.max_width())
    );
    assert_eq!(
        style.get(PropertyId::MinWidth).value(),
        ComputedValue::LengthOrAuto(style.min_width())
    );
    assert_eq!(
        style.get(PropertyId::PaddingLeft).value(),
        ComputedValue::Length(Length::Px(style.box_metrics().padding_left))
    );
    assert_eq!(
        style.get(PropertyId::Width).value(),
        ComputedValue::LengthOrAuto(style.width())
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
        (
            PropertyId::Height,
            ComputedValue::LengthOrAuto(Some(Length::Px(10.0))),
        ),
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
        (
            PropertyId::MaxWidth,
            ComputedValue::LengthOrNone(Some(Length::Px(15.0))),
        ),
        (
            PropertyId::MinWidth,
            ComputedValue::LengthOrAuto(Some(Length::Px(16.0))),
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
            PropertyId::Width,
            ComputedValue::LengthOrAuto(Some(Length::Px(21.0))),
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
