use super::super::{
    StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits, resolve_document_styles,
    try_resolve_document_styles_with_limits,
};
use super::support::{element, namespaced_element, stylesheet};
use crate::{
    CascadePropertyId, CascadeSpecificity, ResolvedValueSource, StylesheetCascadeInput,
    resolve_document_styles_from_cascade_inputs,
};

#[test]
fn ua_namespace_groups_constrain_every_compound_without_constraining_author_rules() {
    let ua = stylesheet(concat!(
        "html > .notice { width: 11px; } ",
        ".notice { height: 12px; } ",
        "* { padding-left: 13px; }",
    ));
    let author = stylesheet("html > .notice { margin-left: 14px; } .notice { color: green; }");
    let dom = namespaced_element(
        html::ElementNamespace::Svg,
        "html",
        Vec::new(),
        vec![
            element("div", vec![("class", Some("notice"))], Vec::new()),
            namespaced_element(
                html::ElementNamespace::Svg,
                "div",
                vec![("class", Some("notice"))],
                Vec::new(),
            ),
        ],
    );
    let inputs = [
        StylesheetCascadeInput::user_agent_for_namespace(&ua, html::ElementNamespace::Html),
        StylesheetCascadeInput::author(&author),
    ];
    let resolved = resolve_document_styles_from_cascade_inputs(&dom, &inputs).unwrap();
    assert_eq!(
        resolved.entries()[0].element_namespace(),
        html::ElementNamespace::Svg
    );
    assert_eq!(
        resolved.entries()[1].element_namespace(),
        html::ElementNamespace::Html
    );
    assert_eq!(
        resolved.entries()[2].element_namespace(),
        html::ElementNamespace::Svg
    );
    let html_child = resolved.entries()[1].style();
    let foreign_child = resolved.entries()[2].style();

    assert_eq!(
        html_child
            .get(CascadePropertyId::Width)
            .expect("width")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::AutoKeyword),
        "the foreign lookalike parent must fail the UA html compound"
    );
    assert_eq!(
        html_child
            .get(CascadePropertyId::Height)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("12px"),
        "a typeless UA compound still matches an HTML candidate"
    );
    assert_eq!(
        html_child
            .get(CascadePropertyId::PaddingLeft)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("13px"),
        "a UA universal selector is constrained at its candidate compound"
    );
    assert_eq!(
        html_child
            .get(CascadePropertyId::MarginLeft)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("14px"),
        "author selectors retain their current unconstrained namespace semantics"
    );
    assert_eq!(
        foreign_child
            .get(CascadePropertyId::Height)
            .expect("height")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::AutoKeyword)
    );
    assert_eq!(
        foreign_child
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("green"),
        "foreign elements remain available to currently supported author matching"
    );
}

#[test]
fn resolve_document_styles_produces_structured_output_without_mutating_dom() {
    let stylesheets = vec![stylesheet(
        "main .hero { color: blue; } div { color: red; }",
    )];
    let dom = element(
        "main",
        Vec::new(),
        vec![element("div", vec![("class", Some("hero"))], Vec::new())],
    );

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

    let html::Node::Element { element } = &dom else {
        panic!("expected element");
    };
    assert!(element.style().is_empty());
    let html::Node::Element { element: child } = &element.children()[0] else {
        panic!("expected child element");
    };
    assert!(child.style().is_empty());

    assert_eq!(resolved.entries().len(), 2);
    assert_eq!(resolved.entries()[0].element_name(), "main");
    assert_eq!(
        resolved.entries()[0].element_namespace(),
        html::ElementNamespace::Html
    );
    assert_eq!(resolved.entries()[1].element_name(), "div");
    assert_eq!(
        resolved.entries()[1]
            .style()
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("blue")
    );
    assert_eq!(
        resolved.entries()[1]
            .style()
            .get(CascadePropertyId::Display)
            .expect("display")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::DisplayInline)
    );
}

#[test]
fn resolve_document_styles_threads_parent_style_for_inheritance() {
    let stylesheets = vec![stylesheet("section { color: red; }")];
    let dom = element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");

    assert_eq!(
        resolved.entries()[0]
            .style()
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("red")
    );
    assert_eq!(
        resolved.entries()[1]
            .style()
            .get(CascadePropertyId::Color)
            .expect("child color")
            .source(),
        &ResolvedValueSource::Inherited
    );
    assert_eq!(
        resolved.entries()[1]
            .style()
            .get(CascadePropertyId::BackgroundColor)
            .expect("child background")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::TransparentColor)
    );
}

#[test]
fn resolve_document_styles_integrates_inline_style_as_structured_author_output() {
    let stylesheets = vec![stylesheet(".hero { color: red; width: 10px; }")];
    let dom = element(
        "div",
        vec![
            ("class", Some("hero")),
            ("style", Some("color: blue; width: 20px;")),
        ],
        Vec::new(),
    );

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");
    let style = resolved.entries()[0].style();

    assert_eq!(
        style
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("blue")
    );
    assert_eq!(
        style
            .get(CascadePropertyId::Width)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("20px")
    );
    let color_winner = style
        .get(CascadePropertyId::Color)
        .and_then(|entry| entry.winner())
        .expect("inline color winner");
    assert_eq!(
        color_winner.priority.specificity,
        CascadeSpecificity::InlineStyle
    );
    assert_eq!(color_winner.priority.rule_order, 1);
}

#[test]
fn resolve_document_styles_rejects_invalid_supported_values_before_winner_resolution() {
    let stylesheets = vec![stylesheet(
        "div { color: red; color: nonsense; display: block; display: grid; }",
    )];
    let dom = element("div", Vec::new(), Vec::new());

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");
    let style = resolved.entries()[0].style();

    assert_eq!(
        style
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("red")
    );
    assert_eq!(
        style
            .get(CascadePropertyId::Display)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("block")
    );
}

#[test]
fn resolve_document_styles_recovers_malformed_inline_declaration_list() {
    let stylesheets = Vec::new();
    let dom = element(
        "div",
        vec![("style", Some("color red width: 10px; color: blue;"))],
        Vec::new(),
    );

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");
    let style = resolved.entries()[0].style();

    assert_eq!(
        style
            .get(CascadePropertyId::Width)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("10px")
    );
    assert_eq!(
        style
            .get(CascadePropertyId::Color)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("blue")
    );
}

#[test]
fn resolve_document_styles_rejects_invalid_outline_shorthand_atomically() {
    let stylesheets = vec![stylesheet(
        "div { outline-color: red; outline-style: solid; outline: 1px 2px; }",
    )];
    let dom = element("div", Vec::new(), Vec::new());

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");
    let style = resolved.entries()[0].style();

    assert_eq!(
        style
            .get(CascadePropertyId::OutlineColor)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("red")
    );
    assert_eq!(
        style
            .get(CascadePropertyId::OutlineStyle)
            .and_then(|entry| entry.winner())
            .and_then(|winner| winner.value.to_css_text())
            .as_deref(),
        Some("solid")
    );
    assert_eq!(
        style
            .get(CascadePropertyId::OutlineWidth)
            .expect("outline-width")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::ZeroPx),
        "invalid shorthand must not partially emit an outline-width candidate"
    );
}

#[test]
fn resolve_document_styles_keeps_border_shorthand_unsupported() {
    let stylesheets = vec![stylesheet("div { border: 1px solid red; }")];
    let dom = element("div", Vec::new(), Vec::new());

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");
    let style = resolved.entries()[0].style();

    assert_eq!(
        style
            .get(CascadePropertyId::BorderTopColor)
            .expect("border-top-color")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::TransparentColor)
    );
    assert_eq!(
        style
            .get(CascadePropertyId::BorderTopStyle)
            .expect("border-top-style")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::BorderStyleNone)
    );
    assert_eq!(
        style
            .get(CascadePropertyId::BorderTopWidth)
            .expect("border-top-width")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::ZeroPx)
    );
}

#[test]
fn resolve_document_styles_falls_back_after_invalid_supported_values() {
    let stylesheets = vec![stylesheet(concat!(
        "section { color: red; }",
        "span { color: nonsense; width: -1px; padding-left: -2px; }",
    ))];
    let dom = element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let resolved = resolve_document_styles(&dom, &stylesheets).expect("resolved document style");
    let child_style = resolved.entries()[1].style();

    assert_eq!(
        child_style
            .get(CascadePropertyId::Color)
            .expect("child color")
            .source(),
        &ResolvedValueSource::Inherited
    );
    assert_eq!(
        child_style
            .get(CascadePropertyId::Width)
            .expect("child width")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::AutoKeyword)
    );
    assert_eq!(
        child_style
            .get(CascadePropertyId::PaddingLeft)
            .expect("child padding-left")
            .source(),
        &ResolvedValueSource::Initial(crate::InitialStyleValue::ZeroPx)
    );
}

#[test]
fn try_resolve_document_styles_reports_style_pass_limits() {
    let stylesheets = vec![stylesheet("div { color: red; }")];
    let dom = element("div", Vec::new(), Vec::new());
    let limits = StyleResolutionLimits {
        max_style_rules_per_document: 0,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(&dom, &stylesheets, &limits)
        .expect_err("style rule limit must fail deterministically");

    assert_eq!(
        error,
        StyleResolutionError::LimitExceeded {
            limit: StyleResolutionLimit::StyleRulesPerDocument,
            configured: 0,
        }
    );
    assert_eq!(
        error.to_string(),
        "style resolution exceeded style-rules-per-document limit 0"
    );
}

#[test]
fn try_resolve_document_styles_reports_styled_element_limits_before_work() {
    let dom = element("main", Vec::new(), Vec::new());
    let limits = StyleResolutionLimits {
        max_styled_elements_per_document: 0,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(&dom, &[], &limits)
        .expect_err("styled element limit must fail deterministically");

    assert_eq!(
        error,
        StyleResolutionError::LimitExceeded {
            limit: StyleResolutionLimit::StyledElementsPerDocument,
            configured: 0,
        }
    );
}

#[test]
fn try_resolve_document_styles_reports_inline_style_byte_limits_before_parsing() {
    let dom = element(
        "div",
        vec![("style", Some("color: red; width: 10px;"))],
        Vec::new(),
    );
    let limits = StyleResolutionLimits {
        max_inline_style_bytes: 4,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(&dom, &[], &limits)
        .expect_err("inline style byte limit must fail before inline parsing");

    assert_eq!(
        error,
        StyleResolutionError::LimitExceeded {
            limit: StyleResolutionLimit::InlineStyleBytes,
            configured: 4,
        }
    );
}

#[test]
fn try_resolve_document_styles_rejects_unrepresentable_limit_configuration() {
    if usize::BITS <= 32 {
        return;
    }

    let dom = element("div", Vec::new(), Vec::new());
    let configured = (u32::MAX as usize).saturating_add(1);
    let limits = StyleResolutionLimits {
        max_style_rules_per_document: configured,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(&dom, &[], &limits)
        .expect_err("unrepresentable style-pass configuration must be rejected explicitly");

    assert_eq!(
        error,
        StyleResolutionError::UnsupportedConfiguration {
            limit: StyleResolutionLimit::StyleRulesPerDocument,
            configured,
            max_supported: u32::MAX as usize,
        }
    );
}
