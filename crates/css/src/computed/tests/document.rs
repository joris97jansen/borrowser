use super::support::*;
use super::*;
use crate::{StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits};

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
    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

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
fn compute_document_styles_propagates_style_resolution_limits() {
    let stylesheets = vec![stylesheet("div { color: red; }")];
    let dom = element("div", Vec::new(), Vec::new());
    let limits = StyleResolutionLimits {
        max_style_rules_per_document: 0,
        ..StyleResolutionLimits::default()
    };

    let error = compute_document_styles_with_limits(&dom, &stylesheets, &limits)
        .expect_err("computed style resolution must preserve style-pass limit errors");

    assert_eq!(
        error,
        ComputedStyleResolutionError::StyleResolution(StyleResolutionError::LimitExceeded {
            limit: StyleResolutionLimit::StyleRulesPerDocument,
            configured: 0,
        })
    );
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
    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

    let computed = compute_document_styles_from_resolved_styles(&dom, &resolved).expect("computed");

    assert_eq!(computed.entries()[0].style().color(), (0, 128, 128, 255));
    assert_eq!(computed.entries()[1].style().color(), (0, 128, 128, 255));
    assert_eq!(computed.entries()[1].style().font_size(), Length::Px(18.0));
}

#[test]
fn compute_document_styles_reuses_identical_resolved_styles_with_same_parent() {
    let stylesheets = vec![stylesheet("p { color: red; }")];
    let dom = element(
        "div",
        Vec::new(),
        vec![
            element("p", Vec::new(), Vec::new()),
            element("p", Vec::new(), Vec::new()),
            element("p", Vec::new(), Vec::new()),
        ],
    );
    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

    let computed = compute_document_styles_from_resolved_styles_with_reuse_stats(&dom, &resolved)
        .expect("computed document");

    assert_eq!(computed.computed.entries().len(), 4);
    assert_eq!(
        computed.reuse_stats,
        ComputedStyleReuseStats { hits: 2, misses: 2 },
        "root div and first paragraph are misses; matching paragraph siblings reuse"
    );
    assert_eq!(
        computed.computed.entries()[1].style(),
        computed.computed.entries()[2].style()
    );
    assert_eq!(
        computed.computed.entries()[2].style(),
        computed.computed.entries()[3].style()
    );
}

#[test]
fn computed_style_reuse_does_not_cross_different_parent_computed_styles() {
    let stylesheets = vec![stylesheet(concat!(
        ".red { color: red; }",
        ".blue { color: blue; }",
    ))];
    let dom = element(
        "div",
        Vec::new(),
        vec![
            element(
                "section",
                vec![("class", Some("red"))],
                vec![element("p", Vec::new(), Vec::new())],
            ),
            element(
                "section",
                vec![("class", Some("blue"))],
                vec![element("p", Vec::new(), Vec::new())],
            ),
        ],
    );
    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

    let computed = compute_document_styles_from_resolved_styles_with_reuse_stats(&dom, &resolved)
        .expect("computed document");

    assert_eq!(computed.computed.entries().len(), 5);
    let first_p = computed.computed.entries()[2].style();
    let second_p = computed.computed.entries()[4].style();

    assert_eq!(first_p.color(), (255, 0, 0, 255));
    assert_eq!(second_p.color(), (0, 0, 255, 255));
    assert_ne!(
        first_p.color(),
        second_p.color(),
        "identical child resolved styles must not reuse across different parent computed styles"
    );
}

#[test]
fn compute_style_from_resolved_style_rejects_normalization_failures() {
    let stylesheets = vec![stylesheet("div { width: 1e39px; }")];
    let dom = element("div", Vec::new(), Vec::new());
    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

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
fn computed_style_method_propagates_authoritative_errors_instead_of_falling_back() {
    let parent_resolved = resolve_initial_style();
    let child_resolved = resolve_cascade_style_from_rule_inputs(&[], Some(&parent_resolved));

    let error = ComputedStyle::from_resolved_style(&child_resolved, None)
        .expect_err("authoritative computed style must preserve typed errors");

    assert_eq!(
        error,
        ComputedStyleResolutionError::MissingInheritedParent {
            property: PropertyId::Color,
        }
    );
}
