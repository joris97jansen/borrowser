use super::super::{
    StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits, resolve_document_styles,
    resolve_document_styles_debug_snapshot,
    try_resolve_document_styles_incremental_suffix_with_limits,
    try_resolve_document_styles_with_limits,
};
use super::support::{
    document, document_element, element, matching_environment, namespaced_document_element,
    namespaced_element, stylesheet,
};
use crate::{
    CascadePropertyId, CascadeRuleMatch, CascadeSpecificity, RawRuleIndex, ResolvedValueSource,
    Rule, SelectorDomBuildError, SelectorDomIndex, SelectorMatchability, SelectorMatchingContext,
    StylesheetCollectionInput, StylesheetConditionInput, StylesheetOrder, StylesheetRuleRef,
    StylesheetSourceId, resolve_document_styles_from_cascade_inputs,
};

#[test]
fn ua_namespace_groups_constrain_every_compound_without_constraining_author_rules() {
    let ua = stylesheet(concat!(
        "html > .notice { width: 11px; } ",
        ".notice { height: 12px; } ",
        "* { padding-left: 13px; }",
    ));
    let author = stylesheet("html > .notice { margin-left: 14px; } .notice { color: green; }");
    let dom = namespaced_document_element(
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
        StylesheetCollectionInput::user_agent_for_namespace(
            StylesheetSourceId::built_in_user_agent(),
            StylesheetOrder::new(0),
            &ua,
            html::ElementNamespace::Html,
        ),
        StylesheetCollectionInput::author(
            StylesheetSourceId::compatibility_generation_index(0),
            StylesheetOrder::new(1),
            &author,
            StylesheetConditionInput::None,
        ),
    ];
    let resolved =
        resolve_document_styles_from_cascade_inputs(&dom, matching_environment(), &inputs).unwrap();
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
    let dom = document_element(
        "main",
        Vec::new(),
        vec![element("div", vec![("class", Some("hero"))], Vec::new())],
    );

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");

    let html::Node::Document { children, .. } = &dom else {
        panic!("expected document");
    };
    let html::Node::Element { element } = &children[0] else {
        panic!("expected document element");
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
fn parser_produced_invalid_and_unsupported_rules_are_non_applicable_to_cascade() {
    let sheet = stylesheet(
        "a:hover { color: red; } div::before { color: green; } [lang=] { color: blue; }",
    );
    let dom = element("a", Vec::new(), Vec::new());
    let html::Node::Element { element } = &dom else {
        panic!("expected element subtree root");
    };
    let index = SelectorDomIndex::try_from_element_subtree(element)
        .expect("valid explicit element subtree");
    let context = SelectorMatchingContext::new(&index, matching_environment());
    let element = index.elements().next().expect("indexed element");

    let expected = [
        SelectorMatchability::Unsupported,
        SelectorMatchability::Unsupported,
        SelectorMatchability::Invalid,
    ];
    let mut checked = 0;

    for (rule_index, rule) in sheet.stylesheet.rules.iter().enumerate() {
        let Rule::Style(rule) = rule else {
            continue;
        };

        let outcome = context
            .match_selector_list(element, &rule.selectors)
            .expect("selector matching should not hit a limit");
        assert_eq!(outcome.matchability(), expected[checked]);
        assert!(!outcome.matched_any());

        let rule_match = CascadeRuleMatch::new(
            StylesheetRuleRef::new(
                StylesheetSourceId::compatibility_generation_index(0),
                RawRuleIndex::from_usize(rule_index).expect("test rule index is representable"),
            ),
            outcome,
        );
        assert!(!rule_match.contributes_candidates());
        checked += 1;
    }

    assert_eq!(checked, expected.len());
}

#[test]
fn selector_list_effective_specificity_uses_only_actual_matches_in_cascade() {
    let stylesheets = vec![stylesheet(
        "#missing, div { color: red; } .target { color: blue; }",
    )];
    let dom = document_element("div", vec![("class", Some("target"))], Vec::new());

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");
    let color = resolved.entries()[0]
        .style()
        .get(CascadePropertyId::Color)
        .and_then(|entry| entry.winner())
        .expect("color winner");

    assert_eq!(color.value.to_css_text().as_deref(), Some("blue"));
    assert_eq!(
        color.priority.specificity(),
        CascadeSpecificity::Selector(crate::Specificity::new(0, 1, 0))
    );

    let snapshot =
        crate::resolve_document_styles_debug_snapshot(&dom, matching_environment(), &stylesheets)
            .expect("document style debug snapshot");
    assert!(
        snapshot.contains(
            "rule-input[0]: source=stylesheet[2/0] origin=author specificity=selector(0,0,1)"
        ),
        "the unmatched #missing selector must not raise the first rule's effective specificity:\n{snapshot}"
    );
    assert!(
        snapshot.contains(
            "rule-input[1]: source=stylesheet[2/1] origin=author specificity=selector(0,1,0)"
        ),
        "the .target rule must carry class specificity:\n{snapshot}"
    );
    assert!(
        snapshot.contains(
            "color: winner(source=stylesheet[2/1]/declaration[0], band=author-normal, specificity=selector(0,1,0)"
        ),
        "the class selector must win through cascade priority:\n{snapshot}"
    );
}

#[test]
fn tree_structural_pseudo_enters_the_normal_cascade_with_b_specificity() {
    let sheet = stylesheet("p { color: red; } p:empty { color: blue; }");
    let parsed = html::parse_document(
        "<!doctype html><html><body><p></p><p>content</p></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("document parses");
    let resolved = resolve_document_styles(&parsed.document, matching_environment(), &[sheet])
        .expect("resolved document style");
    let paragraphs = resolved
        .entries()
        .iter()
        .filter(|entry| entry.element_name() == "p")
        .collect::<Vec<_>>();

    let empty_winner = paragraphs[0]
        .style()
        .get(CascadePropertyId::Color)
        .and_then(|entry| entry.winner())
        .expect("empty paragraph color winner");
    assert_eq!(empty_winner.value.to_css_text().as_deref(), Some("blue"));
    assert_eq!(
        empty_winner.priority.specificity(),
        CascadeSpecificity::Selector(crate::Specificity::new(0, 1, 1))
    );

    let non_empty_winner = paragraphs[1]
        .style()
        .get(CascadePropertyId::Color)
        .and_then(|entry| entry.winner())
        .expect("non-empty paragraph color winner");
    assert_eq!(non_empty_winner.value.to_css_text().as_deref(), Some("red"));
    assert_eq!(
        non_empty_winner.priority.specificity(),
        CascadeSpecificity::Selector(crate::Specificity::C)
    );
}

#[test]
fn resolve_document_styles_threads_parent_style_for_inheritance() {
    let stylesheets = vec![stylesheet("section { color: red; }")];
    let dom = document_element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");

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
    let dom = document_element(
        "div",
        vec![
            ("class", Some("hero")),
            ("style", Some("color: blue; width: 20px;")),
        ],
        Vec::new(),
    );

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");
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
        color_winner.priority.specificity(),
        CascadeSpecificity::InlineStyle
    );
    assert_eq!(
        color_winner.priority.source_order(),
        crate::CascadeSourceOrder::InlineStyle
    );
}

#[test]
fn resolve_document_styles_rejects_invalid_supported_values_before_winner_resolution() {
    let stylesheets = vec![stylesheet(
        "div { color: red; color: nonsense; display: block; display: grid; }",
    )];
    let dom = document_element("div", Vec::new(), Vec::new());

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");
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
    let dom = document_element(
        "div",
        vec![("style", Some("color red width: 10px; color: blue;"))],
        Vec::new(),
    );

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");
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
    let dom = document_element("div", Vec::new(), Vec::new());

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");
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
    let dom = document_element("div", Vec::new(), Vec::new());

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");
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
    let dom = document_element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let resolved = resolve_document_styles(&dom, matching_environment(), &stylesheets)
        .expect("resolved document style");
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
    let dom = document_element("div", Vec::new(), Vec::new());
    let limits = StyleResolutionLimits {
        max_top_level_rules_per_document: 0,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(
        &dom,
        matching_environment(),
        &stylesheets,
        &limits,
    )
    .expect_err("style rule limit must fail deterministically");

    assert_eq!(
        error,
        StyleResolutionError::RuleCollectionBuild(crate::RuleCollectionBuildError::LimitExceeded {
            limit: StyleResolutionLimit::TopLevelRulesPerDocument,
            configured: 0,
            observed: 1,
        },)
    );
    assert_eq!(
        error.to_string(),
        "rule collection observed 1 entries above top-level-rules-per-document limit 0"
    );
}

#[test]
fn try_resolve_document_styles_reports_styled_element_limits_before_work() {
    let dom = document_element("main", Vec::new(), Vec::new());
    let limits = StyleResolutionLimits {
        max_styled_elements_per_document: 0,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(&dom, matching_environment(), &[], &limits)
        .expect_err("styled element limit must fail deterministically");

    assert_eq!(
        error,
        StyleResolutionError::LimitExceeded {
            limit: StyleResolutionLimit::StyledElementsPerDocument,
            configured: 0,
        }
    );
    assert!(
        SelectorDomIndex::try_from_document(&dom).is_ok(),
        "a caller's lower style budget must not invalidate the selector projection"
    );
}

#[test]
fn selector_dom_build_failure_propagates_through_cascade_and_debug_apis() {
    let invalid = document(element(
        "html",
        Vec::new(),
        vec![document(element("span", Vec::new(), Vec::new()))],
    ));
    let expected =
        StyleResolutionError::SelectorDomBuild(SelectorDomBuildError::NestedDocument { depth: 2 });

    assert_eq!(
        resolve_document_styles(&invalid, matching_environment(), &[])
            .expect_err("cascade must propagate selector projection failure"),
        expected
    );
    assert_eq!(
        resolve_document_styles_from_cascade_inputs(&invalid, matching_environment(), &[])
            .expect_err("cascade-input path must propagate selector projection failure"),
        expected
    );
    assert_eq!(
        resolve_document_styles_debug_snapshot(&invalid, matching_environment(), &[])
            .expect_err("debug API must propagate selector projection failure"),
        expected
    );
}

#[test]
fn incremental_resolution_does_not_convert_selector_dom_build_failure_to_unavailable() {
    let valid = document_element("html", Vec::new(), Vec::new());
    let previous = resolve_document_styles(&valid, matching_environment(), &[])
        .expect("valid previous style result");
    let invalid = document(element(
        "html",
        Vec::new(),
        vec![document(element("span", Vec::new(), Vec::new()))],
    ));

    let error = try_resolve_document_styles_incremental_suffix_with_limits(
        &invalid,
        matching_environment(),
        &[],
        &previous,
        &[],
        &StyleResolutionLimits::default(),
    )
    .expect_err("build failure must precede the ordinary incremental-unavailable fallback");

    assert_eq!(
        error,
        StyleResolutionError::SelectorDomBuild(SelectorDomBuildError::NestedDocument { depth: 2 })
    );
}

#[test]
fn try_resolve_document_styles_reports_inline_style_byte_limits_before_parsing() {
    let dom = document_element(
        "div",
        vec![("style", Some("color: red; width: 10px;"))],
        Vec::new(),
    );
    let limits = StyleResolutionLimits {
        max_inline_style_bytes: 4,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(&dom, matching_environment(), &[], &limits)
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

    let dom = document_element("div", Vec::new(), Vec::new());
    let configured = (u32::MAX as usize).saturating_add(1);
    let limits = StyleResolutionLimits {
        max_top_level_rules_per_document: configured,
        ..StyleResolutionLimits::default()
    };

    let error = try_resolve_document_styles_with_limits(&dom, matching_environment(), &[], &limits)
        .expect_err("unrepresentable style-pass configuration must be rejected explicitly");

    assert_eq!(
        error,
        StyleResolutionError::UnsupportedConfiguration {
            limit: StyleResolutionLimit::TopLevelRulesPerDocument,
            configured,
            max_supported: u32::MAX as usize,
        }
    );
}
