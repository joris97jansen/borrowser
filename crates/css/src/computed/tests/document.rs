use super::support::*;
use super::*;
use crate::{
    ComputedDocumentStyleInvalidationImpact, SelectorDomBuildError, StyleResolutionError,
    StyleResolutionLimit, StyleResolutionLimits,
};

fn materialize_element_ids(mut dom: Node) -> Node {
    fn assign(node: &mut Node, next_id: &mut u32) {
        if let Node::Element { .. } = node {
            node.set_id(html::internal::Id(*next_id));
            *next_id += 1;
        }
        if let Some(children) = node.children_mut() {
            for child in children {
                assign(child, next_id);
            }
        }
    }

    let mut next_id = 1;
    assign(&mut dom, &mut next_id);
    dom
}

#[test]
fn compute_style_from_resolved_style_materializes_cascade_fallbacks() {
    let stylesheets = vec![stylesheet(concat!(
        "section { color: #0f0; width: 40px; }",
        "span { color: nonsense; width: -1px; display: block; }",
    ))];
    let dom = document_element(
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
    assert_eq!(
        parent.width(),
        Some(LengthPercentage::Length(Length::Px(40.0)))
    );
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
    let dom = document_element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");
    assert_eq!(computed.entries().len(), 2);
    assert_eq!(computed.entries()[0].selector_element_id().get(), 1);
    assert_eq!(
        computed.entries()[0].element_namespace(),
        html::ElementNamespace::Html
    );
    assert_eq!(computed.entries()[0].element_name(), "section");
    assert_eq!(computed.entries()[1].selector_element_id().get(), 2);
    assert_eq!(computed.entries()[1].element_name(), "span");

    let section = computed.entries()[0].style();
    assert_eq!(section.color(), (255, 0, 0, 255));
    assert_eq!(section.font_size(), Length::Px(20.0));
    assert_eq!(
        section.width(),
        Some(LengthPercentage::Length(Length::Px(40.0)))
    );
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
fn tree_structural_pseudo_matching_affects_parser_backed_computed_style() {
    let parsed = html::parse_document(
        "<!doctype html><html><body><p></p><p>content</p></body></html>",
        html::HtmlParseOptions::default(),
    )
    .expect("document parses");
    let stylesheets = vec![stylesheet("p { color: red; } p:empty { color: blue; }")];
    let computed =
        compute_document_styles(&parsed.document, &stylesheets).expect("computed document style");
    let paragraphs = computed
        .entries()
        .iter()
        .filter(|entry| entry.element_name() == "p")
        .collect::<Vec<_>>();

    assert_eq!(paragraphs[0].style().color(), (0, 0, 255, 255));
    assert_eq!(paragraphs[1].style().color(), (255, 0, 0, 255));
}

#[test]
fn document_style_artifacts_retain_the_explicit_matching_environment() {
    let dom = document_element("div", Vec::new(), Vec::new());
    let environment = crate::SelectorMatchingEnvironment::new(html::DocumentMode::LimitedQuirks);
    let stylesheets = vec![stylesheet("div { color: red; }")];

    let resolved = resolve_document_styles_with_environment(&dom, environment, &stylesheets)
        .expect("resolved document style");
    let computed = compute_document_styles_with_environment(&dom, environment, &stylesheets)
        .expect("computed document style");

    assert_eq!(resolved.matching_environment(), environment);
    assert_eq!(computed.matching_environment(), environment);
}

#[test]
fn computed_style_invalidation_treats_a_matching_environment_change_as_unknown() {
    let dom = document_element("div", Vec::new(), Vec::new());
    let no_quirks = compute_document_styles_with_environment(
        &dom,
        crate::SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
        &[],
    )
    .expect("no-quirks computed style");
    let quirks = compute_document_styles_with_environment(
        &dom,
        crate::SelectorMatchingEnvironment::new(html::DocumentMode::Quirks),
        &[],
    )
    .expect("quirks computed style");

    assert_eq!(
        quirks.invalidation_impact_against(&no_quirks),
        ComputedDocumentStyleInvalidationImpact::Unknown
    );
}

#[test]
fn incremental_style_reuse_rejects_a_different_matching_environment() {
    let dom = materialize_element_ids(document_element("div", Vec::new(), Vec::new()));
    let no_quirks = crate::SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);
    let quirks = crate::SelectorMatchingEnvironment::new(html::DocumentMode::Quirks);
    let stylesheets = vec![stylesheet("div { color: red; }")];
    let inputs = [StylesheetCascadeInput::author(&stylesheets[0])];
    let resolved = resolve_document_styles_with_environment(&dom, no_quirks, &stylesheets)
        .expect("initial resolved style");
    let computed = compute_document_styles_from_resolved_styles(&dom, &resolved)
        .expect("initial computed style");
    let plan = attribute_invalidation_plan(vec![html::internal::Id(1)]);

    let error = try_compute_document_styles_for_invalidation_plan_with_limits_with_environment(
        &plan,
        &dom,
        quirks,
        &inputs,
        Some((&resolved, &computed)),
        &StyleResolutionLimits::default(),
    )
    .expect_err("a different matching environment must be an invariant failure");

    assert_eq!(
        error,
        ComputedStyleResolutionError::StyleResolution(
            StyleResolutionError::MatchingEnvironmentMismatch {
                expected: quirks,
                actual: no_quirks,
            },
        )
    );
}

#[test]
fn plan_execution_reports_full_required_without_incremental_state() {
    let dom = document_element("div", Vec::new(), Vec::new());
    let plan = tree_invalidation_plan();

    let execution = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &dom,
        &[],
        None,
        &StyleResolutionLimits::default(),
    )
    .expect("full execution result");

    assert_eq!(execution, StylePlanExecution::FullRequired);
    assert!(!execution.is_incremental_eligible());
}

#[test]
fn plan_execution_reports_incremental_unavailable_without_retained_artifacts() {
    let dom = document_element("div", Vec::new(), Vec::new());
    let plan = attribute_invalidation_plan(vec![html::internal::Id(1)]);

    let execution = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &dom,
        &[],
        None,
        &StyleResolutionLimits::default(),
    )
    .expect("unavailable incremental result");

    assert_eq!(execution, StylePlanExecution::IncrementalUnavailable);
    assert!(execution.is_incremental_eligible());
}

#[test]
fn plan_execution_propagates_selector_dom_build_failure_without_retained_artifacts() {
    let invalid = document(element(
        "html",
        Vec::new(),
        vec![document(element("span", Vec::new(), Vec::new()))],
    ));
    let plan = attribute_invalidation_plan(vec![html::internal::Id(1)]);

    let error = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &invalid,
        &[],
        None,
        &StyleResolutionLimits::default(),
    )
    .expect_err("selector-DOM build failure must precede incremental unavailability");

    assert_eq!(
        error,
        ComputedStyleResolutionError::StyleResolution(StyleResolutionError::SelectorDomBuild(
            SelectorDomBuildError::NestedDocument { depth: 2 }
        ),)
    );
}

#[test]
fn plan_execution_preserves_styled_element_limit_without_retained_artifacts() {
    let dom = document_element("div", Vec::new(), Vec::new());
    let plan = attribute_invalidation_plan(vec![html::internal::Id(1)]);
    let limits = StyleResolutionLimits {
        max_styled_elements_per_document: 0,
        ..StyleResolutionLimits::default()
    };

    let error = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &dom,
        &[],
        None,
        &limits,
    )
    .expect_err("style element budget failure must remain a style-resolution error");

    assert_eq!(
        error,
        ComputedStyleResolutionError::StyleResolution(StyleResolutionError::LimitExceeded {
            limit: StyleResolutionLimit::StyledElementsPerDocument,
            configured: 0,
        })
    );
}

#[test]
fn plan_execution_reports_incremental_computed_for_a_valid_suffix() {
    let initial_dom = materialize_element_ids(document_element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    ));
    let changed_dom = materialize_element_ids(document_element(
        "div",
        vec![("class", Some("hot"))],
        vec![element("span", Vec::new(), Vec::new())],
    ));
    let stylesheets = vec![stylesheet(".hot { color: red; }")];
    let inputs = [StylesheetCascadeInput::author(&stylesheets[0])];
    let resolved =
        resolve_document_styles(&initial_dom, &stylesheets).expect("initial resolved styles");
    let initial_computed =
        compute_document_styles_from_resolved_styles_with_reuse_stats(&initial_dom, &resolved)
            .expect("initial computed styles");
    let plan = attribute_invalidation_plan(vec![html::internal::Id(1)]);

    let execution = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &changed_dom,
        &inputs,
        Some((&resolved, &initial_computed.computed)),
        &StyleResolutionLimits::default(),
    )
    .expect("successful incremental result");

    let StylePlanExecution::IncrementalComputed(incremental) = execution else {
        panic!("expected a computed incremental result");
    };
    assert!(incremental.recomputed_len > 0);
    assert_eq!(
        incremental.computed.entries()[0].style().color(),
        (255, 0, 0, 255)
    );
    assert_eq!(
        incremental.computed.entries()[1].style().color(),
        (255, 0, 0, 255)
    );
}

#[test]
fn plan_aware_suffix_recomputes_following_sibling_selector_effects() {
    let initial_dom = materialize_element_ids(document_element(
        "section",
        Vec::new(),
        vec![
            element("div", Vec::new(), Vec::new()),
            element("p", Vec::new(), Vec::new()),
        ],
    ));
    let changed_dom = materialize_element_ids(document_element(
        "section",
        Vec::new(),
        vec![
            element("div", vec![("class", Some("on"))], Vec::new()),
            element("p", Vec::new(), Vec::new()),
        ],
    ));
    let stylesheets = vec![stylesheet(".on + p { color: red; }")];
    let inputs = [StylesheetCascadeInput::author(&stylesheets[0])];
    let resolved =
        resolve_document_styles(&initial_dom, &stylesheets).expect("initial resolved styles");
    let initial_computed =
        compute_document_styles_from_resolved_styles_with_reuse_stats(&initial_dom, &resolved)
            .expect("initial computed styles");
    let plan = attribute_invalidation_plan(vec![html::internal::Id(2)]);
    let execution = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &changed_dom,
        &inputs,
        Some((&resolved, &initial_computed.computed)),
        &StyleResolutionLimits::default(),
    )
    .expect("sibling incremental result");

    let StylePlanExecution::IncrementalComputed(incremental) = execution else {
        panic!("expected a computed incremental result");
    };
    assert_eq!(
        incremental.computed.entries()[2].style().color(),
        (255, 0, 0, 255)
    );
}

#[test]
fn plan_aware_suffix_recomputes_inherited_descendant_effects() {
    let initial_dom = materialize_element_ids(document_element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    ));
    let changed_dom = materialize_element_ids(document_element(
        "div",
        vec![("class", Some("on"))],
        vec![element("span", Vec::new(), Vec::new())],
    ));
    let stylesheets = vec![stylesheet(".on { color: red; }")];
    let inputs = [StylesheetCascadeInput::author(&stylesheets[0])];
    let resolved =
        resolve_document_styles(&initial_dom, &stylesheets).expect("initial resolved styles");
    let initial_computed =
        compute_document_styles_from_resolved_styles_with_reuse_stats(&initial_dom, &resolved)
            .expect("initial computed styles");
    let plan = attribute_invalidation_plan(vec![html::internal::Id(1)]);
    let execution = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &changed_dom,
        &inputs,
        Some((&resolved, &initial_computed.computed)),
        &StyleResolutionLimits::default(),
    )
    .expect("inheritance incremental result");

    let StylePlanExecution::IncrementalComputed(incremental) = execution else {
        panic!("expected a computed incremental result");
    };
    assert_eq!(
        incremental.computed.entries()[1].style().color(),
        (255, 0, 0, 255)
    );
}

#[test]
fn plan_aware_suffix_recomputes_descendant_selector_effects() {
    let initial_dom = materialize_element_ids(document_element(
        "div",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    ));
    let changed_dom = materialize_element_ids(document_element(
        "div",
        vec![("class", Some("hot"))],
        vec![element("span", Vec::new(), Vec::new())],
    ));
    let stylesheets = vec![stylesheet(".hot span { color: red; }")];
    let inputs = [StylesheetCascadeInput::author(&stylesheets[0])];
    let resolved =
        resolve_document_styles(&initial_dom, &stylesheets).expect("initial resolved styles");
    let initial_computed =
        compute_document_styles_from_resolved_styles_with_reuse_stats(&initial_dom, &resolved)
            .expect("initial computed styles");
    let plan = attribute_invalidation_plan(vec![html::internal::Id(1)]);
    let execution = try_compute_document_styles_for_invalidation_plan_with_limits(
        &plan,
        &changed_dom,
        &inputs,
        Some((&resolved, &initial_computed.computed)),
        &StyleResolutionLimits::default(),
    )
    .expect("descendant selector incremental result");

    let StylePlanExecution::IncrementalComputed(incremental) = execution else {
        panic!("expected a computed incremental result");
    };
    assert_eq!(
        incremental.computed.entries()[1].style().color(),
        (255, 0, 0, 255)
    );
}

#[test]
fn compute_document_styles_materializes_ad5_initial_and_inherited_boundaries() {
    let stylesheets = vec![stylesheet(concat!(
        "section { color: #0f0; font-size: 20px; width: 40px; background-color: red; display: block; }",
        "span { background-color: #00f; }",
    ))];
    let dom = document_element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");
    let section = computed.entries()[0].style();
    let span = computed.entries()[1].style();

    assert_eq!(
        section.get(PropertyId::Color).value(),
        ComputedValue::Color((0, 255, 0, 255))
    );
    assert_eq!(
        section.get(PropertyId::Width).value(),
        ComputedValue::LengthPercentageOrAuto(Some(LengthPercentage::Length(Length::Px(40.0))))
    );
    assert_eq!(
        span.get(PropertyId::Color).value(),
        section.get(PropertyId::Color).value(),
        "color is inherited by default through CSS-owned computed materialization"
    );
    assert_eq!(
        span.get(PropertyId::FontSize).value(),
        section.get(PropertyId::FontSize).value(),
        "font-size is inherited by default through CSS-owned computed materialization"
    );
    assert_eq!(
        span.get(PropertyId::Width).value(),
        ComputedValue::LengthPercentageOrAuto(None),
        "non-inherited width falls back to the CSS initial auto value"
    );
    assert_eq!(
        span.get(PropertyId::Display).value(),
        ComputedValue::Display(Display::Inline),
        "non-inherited display falls back to the CSS initial inline value"
    );
    assert_eq!(
        span.get(PropertyId::BackgroundColor).value(),
        ComputedValue::Color((0, 0, 255, 255)),
        "paint-relevant declarations materialize as typed computed color values"
    );
}

#[test]
fn compute_document_styles_materializes_resolved_css_wide_keywords() {
    let stylesheets = vec![stylesheet(concat!(
        "section { color: red; font-size: 20px; width: 40px; display: block; }",
        "span { color: unset; font-size: inherit; width: inherit; display: initial; }",
    ))];
    let dom = document_element(
        "section",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    );

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");
    let section = computed.entries()[0].style();
    let span = computed.entries()[1].style();

    assert_eq!(section.color(), (255, 0, 0, 255));
    assert_eq!(section.font_size(), Length::Px(20.0));
    assert_eq!(
        section.width(),
        Some(LengthPercentage::Length(Length::Px(40.0)))
    );
    assert_eq!(section.display(), Display::Block);

    assert_eq!(span.color(), section.color());
    assert_eq!(span.font_size(), section.font_size());
    assert_eq!(span.width(), section.width());
    assert_eq!(span.display(), Display::Inline);
}

#[test]
fn compute_document_styles_materializes_outline_shorthand_through_longhand_pipeline() {
    let stylesheets = vec![stylesheet("div { outline: 2px solid red; }")];
    let dom = document_element("div", Vec::new(), Vec::new());

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");
    let outline = computed.entries()[0].style().outline();

    assert_eq!(outline.color, (255, 0, 0, 255));
    assert_eq!(outline.style, OutlineStyle::Solid);
    assert_eq!(outline.width, 2.0);
}

#[test]
fn compute_document_styles_preserves_authored_order_around_outline_shorthand_resets() {
    let dom = document_element("div", Vec::new(), Vec::new());
    let longhand_then_shorthand = compute_document_styles(
        &dom,
        &[stylesheet("div { outline-width: 4px; outline: solid; }")],
    )
    .expect("computed document");
    let shorthand_then_longhand = compute_document_styles(
        &dom,
        &[stylesheet("div { outline: solid; outline-width: 4px; }")],
    )
    .expect("computed document");

    let reset_outline = longhand_then_shorthand.entries()[0].style().outline();
    assert_eq!(reset_outline.style, OutlineStyle::Solid);
    assert_eq!(
        reset_outline.width, 0.0,
        "later shorthand omitted width resets through internal initial expansion"
    );

    let overridden_outline = shorthand_then_longhand.entries()[0].style().outline();
    assert_eq!(overridden_outline.style, OutlineStyle::Solid);
    assert_eq!(
        overridden_outline.width, 4.0,
        "later authored longhand still wins by declaration order"
    );
}

#[test]
fn compute_document_styles_materializes_root_css_wide_fallbacks_to_initial() {
    let stylesheets = vec![stylesheet(
        "div { color: inherit; font-size: unset; width: inherit; display: unset; }",
    )];
    let dom = document_element("div", Vec::new(), Vec::new());

    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");
    let style = computed.entries()[0].style();

    assert_eq!(style.color(), (0, 0, 0, 255));
    assert_eq!(style.font_size(), Length::Px(16.0));
    assert_eq!(style.width(), None);
    assert_eq!(style.display(), Display::Inline);
}

#[test]
fn computed_document_style_invalidation_impact_distinguishes_paint_layout_and_unknown() {
    let dom = document_element(
        "section",
        Vec::new(),
        vec![element("p", Vec::new(), Vec::new())],
    );
    let base = compute_document_styles(&dom, &[stylesheet("p { color: red; }")])
        .expect("base computed document");
    let paint_only = compute_document_styles(&dom, &[stylesheet("p { color: blue; }")])
        .expect("paint-only computed document");
    let layout_affecting = compute_document_styles(&dom, &[stylesheet("p { width: 20px; }")])
        .expect("layout-affecting computed document");
    let different_shape = compute_document_styles(
        &document_element("section", Vec::new(), Vec::new()),
        &[stylesheet("section { color: red; }")],
    )
    .expect("different shape computed document");
    let different_namespace = compute_document_styles(
        &namespaced_document_element(
            html::ElementNamespace::Svg,
            "section",
            Vec::new(),
            vec![namespaced_element(
                html::ElementNamespace::Svg,
                "p",
                Vec::new(),
                Vec::new(),
            )],
        ),
        &[stylesheet("p { color: red; }")],
    )
    .expect("different-namespace computed document");

    assert_eq!(
        paint_only.invalidation_impact_against(&base),
        ComputedDocumentStyleInvalidationImpact::PaintOnly
    );
    assert_eq!(
        layout_affecting.invalidation_impact_against(&base),
        ComputedDocumentStyleInvalidationImpact::LayoutAffecting
    );
    assert_eq!(
        different_shape.invalidation_impact_against(&base),
        ComputedDocumentStyleInvalidationImpact::Unknown
    );
    assert_eq!(
        different_namespace.invalidation_impact_against(&base),
        ComputedDocumentStyleInvalidationImpact::Unknown
    );
}

#[test]
fn compute_document_styles_propagates_style_resolution_limits() {
    let stylesheets = vec![stylesheet("div { color: red; }")];
    let dom = document_element("div", Vec::new(), Vec::new());
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
fn computed_style_apis_propagate_selector_dom_build_failures() {
    let valid = document_element("html", Vec::new(), Vec::new());
    let resolved = resolve_document_styles(&valid, &[]).expect("valid resolved styles");
    let computed = compute_document_styles_from_resolved_styles(&valid, &resolved)
        .expect("valid computed styles");
    let invalid = document(element(
        "html",
        Vec::new(),
        vec![document(element("span", Vec::new(), Vec::new()))],
    ));
    let build_error = SelectorDomBuildError::NestedDocument { depth: 2 };

    assert_eq!(
        compute_document_styles(&invalid, &[])
            .expect_err("integrated computed styles must propagate cascade projection failure"),
        ComputedStyleResolutionError::StyleResolution(StyleResolutionError::SelectorDomBuild(
            build_error
        ),)
    );
    assert_eq!(
        compute_document_styles_from_resolved_styles(&invalid, &resolved)
            .expect_err("computed reconstruction must rebuild the projection fallibly"),
        ComputedStyleResolutionError::SelectorDomBuild(build_error)
    );
    let style_tree_error = match build_style_tree_from_computed_styles(&invalid, &computed) {
        Ok(_) => panic!("style-tree reconstruction must propagate projection failure"),
        Err(error) => error,
    };
    assert_eq!(
        style_tree_error,
        ComputedStyleResolutionError::SelectorDomBuild(build_error)
    );
    let integrated_style_tree_error = match build_style_tree_with_stylesheets(&invalid, &[]) {
        Ok(_) => panic!("integrated style-tree construction must propagate projection failure"),
        Err(error) => error,
    };
    assert_eq!(
        integrated_style_tree_error,
        ComputedStyleResolutionError::StyleResolution(StyleResolutionError::SelectorDomBuild(
            build_error
        ),)
    );
}

#[test]
fn computed_document_style_snapshot_is_deterministic() {
    let stylesheets = vec![stylesheet(
        "div { color: blue; width: 12px; } span { margin-left: -2px; }",
    )];
    let dom = document_element(
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
            "element[0]: selector-id=1 namespace=html name=\"div\"\n",
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
            "  width: 12px\n",
            "  z-index: auto\n",
            "element[1]: selector-id=2 namespace=html name=\"span\"\n",
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
fn compute_document_styles_from_resolved_styles_uses_existing_cascade_output() {
    let stylesheets = vec![stylesheet("main { color: teal; } p { font-size: 18px; }")];
    let dom = document_element(
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
    let dom = document_element(
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
    let dom = document_element(
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
    let dom = document_element("div", Vec::new(), Vec::new());
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
