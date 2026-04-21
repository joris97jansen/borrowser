use super::super::resolve_document_styles;
use super::support::{element, stylesheet};
use crate::{CascadePropertyId, CascadeSpecificity, ResolvedValueSource};

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

    let resolved = resolve_document_styles(&dom, &stylesheets);

    let html::Node::Element {
        style, children, ..
    } = &dom
    else {
        panic!("expected element");
    };
    assert!(style.is_empty());
    let html::Node::Element {
        style: child_style, ..
    } = &children[0]
    else {
        panic!("expected child element");
    };
    assert!(child_style.is_empty());

    assert_eq!(resolved.entries().len(), 2);
    assert_eq!(resolved.entries()[0].element_name(), "main");
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

    let resolved = resolve_document_styles(&dom, &stylesheets);

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

    let resolved = resolve_document_styles(&dom, &stylesheets);
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

    let resolved = resolve_document_styles(&dom, &stylesheets);
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
