use crate::page::RestyleHint;
use crate::rendering::*;

use super::support::*;

#[test]
fn debug_snapshot_reports_retained_style_artifacts_and_ephemeral_downstream_trees() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );

    let style_output = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(style_output.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(style_output);

    let snapshot = page.render_pipeline_debug_snapshot();
    assert_eq!(
        snapshot,
        RenderPipelineDebugSnapshot {
            has_dom: true,
            resolved_styles: RenderArtifactState::RetainedFresh,
            computed_styles: RenderArtifactState::RetainedFresh,
            styled_tree: RenderArtifactState::BorrowBackedRebuiltOnDemand,
            layout_tree: RenderArtifactState::FrameLocalRebuiltPerFrame,
            paint_output: RenderArtifactState::ImmediateFrameOutput,
            style_dirty: false,
            layout_dirty: true,
            style_invalidation: StyleInvalidationState::None,
        }
    );
}

#[test]
fn attribute_mutation_keeps_style_cache_but_marks_it_stale_until_restored() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>.hot { color: red; } p { color: black; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(initial.root(), "p"), (0, 0, 0, 255));
    drop(initial);

    let p_id = set_first_element_attr(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        "class",
        Some("hot".to_string()),
    );
    let hint = RestyleHint::attributes_changed(vec![p_id]);
    page.mark_dom_changed_for_tests(hint);

    let stale = page.render_pipeline_debug_snapshot();
    assert_eq!(stale.resolved_styles, RenderArtifactState::RetainedStale);
    assert_eq!(stale.computed_styles, RenderArtifactState::RetainedStale);
    assert_eq!(
        stale.style_invalidation,
        StyleInvalidationState::AttributeSuffix
    );
    assert!(stale.style_dirty);
    assert!(stale.layout_dirty);

    let restyled = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(restyled.root(), "p"), (255, 0, 0, 255));
    drop(restyled);

    let refreshed = page.render_pipeline_debug_snapshot();
    assert_eq!(
        refreshed.resolved_styles,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        refreshed.computed_styles,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(refreshed.style_invalidation, StyleInvalidationState::None);
    assert!(!refreshed.style_dirty);
}

#[test]
fn text_mutation_dirties_layout_without_invalidating_computed_style() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(initial.root(), "p"), (255, 0, 0, 255));
    drop(initial);
    page.clear_layout_dirty_for_tests();

    replace_first_text(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "Hello",
        "Goodbye",
    );
    let hint = RestyleHint::text_mutated();
    page.mark_dom_changed_for_tests(hint);

    let snapshot = page.render_pipeline_debug_snapshot();
    assert_eq!(snapshot.resolved_styles, RenderArtifactState::RetainedFresh);
    assert_eq!(snapshot.computed_styles, RenderArtifactState::RetainedFresh);
    assert_eq!(snapshot.style_invalidation, StyleInvalidationState::None);
    assert!(!snapshot.style_dirty);
    assert!(snapshot.layout_dirty);
}

#[test]
fn navigation_reset_clears_page_owned_retained_render_state() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let style_output = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(style_output.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(style_output);

    page.start_nav("https://example.com/next.html");

    assert_eq!(
        page.render_pipeline_debug_snapshot(),
        RenderPipelineDebugSnapshot {
            has_dom: false,
            resolved_styles: RenderArtifactState::Absent,
            computed_styles: RenderArtifactState::Absent,
            styled_tree: RenderArtifactState::Absent,
            layout_tree: RenderArtifactState::Absent,
            paint_output: RenderArtifactState::Absent,
            style_dirty: true,
            layout_dirty: true,
            style_invalidation: StyleInvalidationState::Full,
        }
    );
}
