use crate::input_state::DocumentInputState;
use crate::page::{PageState, RestyleHint};
use crate::rendering::*;
use crate::resources::ResourceManager;
use egui::{CentralPanel, Context, Pos2, RawInput, Rect, Vec2};
use gfx::paint::{PaintArtifact, PaintPhaseInput};
use html::internal::Id;
use layout::{
    LayoutPhaseInput, RetainedLayoutFallbackReason, RetainedLayoutFrameAction,
    RetainedLayoutFrameResult,
};

use super::support::*;

#[test]
fn retained_render_state_initializes_with_epoch_zero() {
    let page = PageState::new();

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(snapshot.render_epoch, RenderEpoch::initial());
    assert!(!snapshot.has_dom);
    assert_eq!(snapshot.resolved_styles, RenderArtifactState::Absent);
    assert_eq!(snapshot.computed_styles, RenderArtifactState::Absent);
    assert_eq!(snapshot.styled_tree, RenderArtifactState::Absent);
    assert_eq!(snapshot.layout_tree, RenderArtifactState::Absent);
    assert_eq!(snapshot.paint_output, RenderArtifactState::Absent);
    assert!(snapshot.style_dirty);
    assert!(snapshot.layout_dirty);
    assert!(snapshot.paint_dirty);
    assert_eq!(
        snapshot.dirty_state.entries,
        vec![
            DirtyEntry::new(
                DirtyPhase::Style,
                DirtyReason::ConservativeUnknownImpact,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::CascadedFromStyle,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            ),
        ]
    );
    assert_eq!(snapshot.style_invalidation, StyleInvalidationState::Full);
    assert_eq!(
        snapshot.style_artifacts,
        RetainedStyleArtifactDebugSnapshot {
            key: None,
            state: RenderArtifactState::Absent,
            last_action: RetainedStyleArtifactAction::None,
            stats: RetainedStyleArtifactStats::default(),
        }
    );
    assert_eq!(
        snapshot.retained_identity_domain,
        RetainedRenderIdentityDomain::initial()
    );
    assert!(snapshot.retained_identities.is_empty());
    assert_eq!(
        snapshot.layout_identity,
        FrameLocalIdentityState::NotRetained
    );
    assert_eq!(
        snapshot.paint_identity,
        FrameLocalIdentityState::NotRetained
    );
    assert_eq!(
        snapshot.stacking_identity,
        FrameLocalIdentityState::NotRetained
    );
    assert_eq!(
        snapshot.traversal_source_order_identity,
        FrameLocalIdentityState::NotRetained
    );
}

#[test]
fn retained_render_state_debug_snapshot_is_deterministic() {
    let page = PageState::new();

    assert_eq!(
        page.retained_render_state_debug_snapshot()
            .to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "retained-render-state\n",
            "render-epoch: 0\n",
            "has-dom: false\n",
            "artifacts:\n",
            "  resolved-styles: absent\n",
            "  computed-styles: absent\n",
            "  styled-tree: absent\n",
            "  layout-tree: absent\n",
            "  paint-output: absent\n",
            "dirty-state:\n",
            "  entries: 3\n",
            "    entry[0]: phase=style reason=conservative-unknown-impact scope=document\n",
            "    entry[1]: phase=layout reason=cascaded-from-style scope=document\n",
            "    entry[2]: phase=paint reason=cascaded-from-layout scope=document\n",
            "  style-dirty: true\n",
            "  layout-dirty: true\n",
            "  paint-dirty: true\n",
            "  style-invalidation: full\n",
            "generations:\n",
            "  dom-generation: 0\n",
            "  style-input-generation: 0\n",
            "  stylesheet-generation: 0\n",
            "  layout-input-generation: 0\n",
            "  layout-style-generation: 0\n",
            "  paint-style-generation: 0\n",
            "  paint-input-generation: 0\n",
            "  text-measurement-generation: 0\n",
            "  replaced-metadata-generation: 0\n",
            "style-artifacts:\n",
            "  key: none\n",
            "  state: absent\n",
            "  last-action: none\n",
            "  reuse-count: 0\n",
            "  recompute-count: 0\n",
            "  discard-count: 0\n",
            "layout-artifacts:\n",
            "  key-seed: identity-domain=0 layout-input-generation=0 layout-style-generation=0 text-measurement-generation=0 replaced-metadata-generation=0\n",
            "  key: none\n",
            "  state: absent\n",
            "  last-action: none\n",
            "  reuse-count: 0\n",
            "  recompute-count: 0\n",
            "  discard-count: 0\n",
            "paint-artifacts:\n",
            "  key: none\n",
            "  state: absent\n",
            "  last-action: none\n",
            "  reuse-count: 0\n",
            "  recompute-count: 0\n",
            "  discard-count: 0\n",
            "retained-identities:\n",
            "  identity-domain: 0\n",
            "  render-artifacts: 0\n",
            "  frame-local-layout-ids: not-retained\n",
            "  frame-local-paint-ids: not-retained\n",
            "  frame-local-stacking-ids: not-retained\n",
            "  frame-local-traversal-source-order-ids: not-retained\n",
        )
    );
}

#[test]
fn retained_render_identities_allocate_deterministically_for_initial_document() {
    let page = page_with_node(doc_with_explicit_ids());

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(snapshot.retained_identity_domain.value(), 1);
    assert_eq!(
        snapshot.retained_identities,
        vec![
            retained_identity(1, 1),
            retained_identity(2, 2),
            retained_identity(3, 3),
            retained_identity(4, 4),
            retained_identity(5, 5),
        ]
    );

    assert_eq!(
        snapshot.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "retained-render-state\n",
            "render-epoch: 1\n",
            "has-dom: true\n",
            "artifacts:\n",
            "  resolved-styles: absent\n",
            "  computed-styles: absent\n",
            "  styled-tree: borrow-backed-rebuilt-on-demand\n",
            "  layout-tree: absent\n",
            "  paint-output: absent\n",
            "dirty-state:\n",
            "  entries: 3\n",
            "    entry[0]: phase=style reason=document-replaced scope=document\n",
            "    entry[1]: phase=layout reason=cascaded-from-style scope=document\n",
            "    entry[2]: phase=paint reason=cascaded-from-layout scope=document\n",
            "  style-dirty: true\n",
            "  layout-dirty: true\n",
            "  paint-dirty: true\n",
            "  style-invalidation: full\n",
            "generations:\n",
            "  dom-generation: 1\n",
            "  style-input-generation: 1\n",
            "  stylesheet-generation: 0\n",
            "  layout-input-generation: 1\n",
            "  layout-style-generation: 0\n",
            "  paint-style-generation: 0\n",
            "  paint-input-generation: 0\n",
            "  text-measurement-generation: 0\n",
            "  replaced-metadata-generation: 0\n",
            "style-artifacts:\n",
            "  key: none\n",
            "  state: absent\n",
            "  last-action: none\n",
            "  reuse-count: 0\n",
            "  recompute-count: 0\n",
            "  discard-count: 0\n",
            "layout-artifacts:\n",
            "  key-seed: identity-domain=1 layout-input-generation=1 layout-style-generation=0 text-measurement-generation=0 replaced-metadata-generation=0\n",
            "  key: none\n",
            "  state: absent\n",
            "  last-action: none\n",
            "  reuse-count: 0\n",
            "  recompute-count: 0\n",
            "  discard-count: 0\n",
            "paint-artifacts:\n",
            "  key: none\n",
            "  state: absent\n",
            "  last-action: none\n",
            "  reuse-count: 0\n",
            "  recompute-count: 0\n",
            "  discard-count: 0\n",
            "retained-identities:\n",
            "  identity-domain: 1\n",
            "  render-artifacts: 5\n",
            "    - retained-render-id=1 kind=dom-backed-render-node anchor=dom-node(1)\n",
            "    - retained-render-id=2 kind=dom-backed-render-node anchor=dom-node(2)\n",
            "    - retained-render-id=3 kind=dom-backed-render-node anchor=dom-node(3)\n",
            "    - retained-render-id=4 kind=dom-backed-render-node anchor=dom-node(4)\n",
            "    - retained-render-id=5 kind=dom-backed-render-node anchor=dom-node(5)\n",
            "  frame-local-layout-ids: not-retained\n",
            "  frame-local-paint-ids: not-retained\n",
            "  frame-local-stacking-ids: not-retained\n",
            "  frame-local-traversal-source-order-ids: not-retained\n",
        )
    );
}

#[test]
fn equivalent_fresh_input_produces_deterministic_identity_output_without_cross_document_claims() {
    let first = page_with_node(doc_with_explicit_ids())
        .retained_render_state_debug_snapshot()
        .to_debug_snapshot();
    let second = page_with_node(doc_with_explicit_ids())
        .retained_render_state_debug_snapshot()
        .to_debug_snapshot();

    assert_eq!(first, second);
    assert!(first.contains("  identity-domain: 1\n"));
}

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
    let identity_domain = page
        .retained_render_state_debug_snapshot()
        .retained_identity_domain;
    assert_eq!(
        snapshot,
        RenderPipelineDebugSnapshot {
            has_dom: true,
            resolved_styles: RenderArtifactState::RetainedFresh,
            computed_styles: RenderArtifactState::RetainedFresh,
            styled_tree: RenderArtifactState::BorrowBackedRebuiltOnDemand,
            layout_tree: RenderArtifactState::Absent,
            paint_output: RenderArtifactState::Absent,
            dirty_state: DirtyStateDebugSnapshot {
                entries: vec![
                    DirtyEntry::new(
                        DirtyPhase::Layout,
                        DirtyReason::CascadedFromStyle,
                        DirtyScope::Document,
                    ),
                    DirtyEntry::new(
                        DirtyPhase::Paint,
                        DirtyReason::CascadedFromLayout,
                        DirtyScope::Document,
                    ),
                ],
            },
            style_dirty: false,
            layout_dirty: true,
            paint_dirty: true,
            style_invalidation: StyleInvalidationState::None,
            generations: RetainedRenderGenerationDebugSnapshot {
                dom_generation: 1,
                style_input_generation: 1,
                stylesheet_generation: 1,
                layout_input_generation: 1,
                layout_style_generation: 0,
                paint_style_generation: 0,
                paint_input_generation: 0,
                text_measurement_generation: 0,
                replaced_metadata_generation: 0,
            },
            style_artifacts: RetainedStyleArtifactDebugSnapshot {
                key: Some(RetainedStyleArtifactKey {
                    identity_domain,
                    style_input_generation: 1,
                    stylesheet_generation: 1,
                }),
                state: RenderArtifactState::RetainedFresh,
                last_action: RetainedStyleArtifactAction::InitialCompute,
                stats: RetainedStyleArtifactStats {
                    reuse_count: 0,
                    recompute_count: 1,
                    discard_count: 0,
                },
            },
            layout_artifacts: RetainedLayoutArtifactDebugSnapshot {
                key_seed: layout::RetainedLayoutKeySeed {
                    identity_domain: identity_domain.value(),
                    layout_input_generation: 1,
                    layout_style_generation: 0,
                    text_measurement_generation: 0,
                    replaced_metadata_generation: 0,
                },
                key: None,
                state: RenderArtifactState::Absent,
                last_action: RetainedLayoutArtifactAction::None,
                stats: RetainedLayoutArtifactStats::default(),
            },
            paint_artifacts: RetainedPaintArtifactDebugSnapshot {
                key: None,
                state: RenderArtifactState::Absent,
                last_action: RetainedPaintArtifactAction::None,
                stats: RetainedPaintArtifactStats::default(),
            },
        }
    );
}

#[test]
fn initial_style_computation_records_retained_style_artifact_lifecycle() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );

    let style_output = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(style_output.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(style_output);

    let snapshot = page.retained_render_state_debug_snapshot();
    let key = snapshot
        .style_artifacts
        .key
        .expect("retained style artifacts should have a key after initial compute");
    assert_eq!(key.identity_domain, snapshot.retained_identity_domain);
    assert_eq!(key.style_input_generation, 1);
    assert_eq!(key.stylesheet_generation, 1);
    assert_eq!(
        snapshot.style_artifacts.state,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        snapshot.style_artifacts.last_action,
        RetainedStyleArtifactAction::InitialCompute
    );
    assert_eq!(
        snapshot.style_artifacts.stats,
        RetainedStyleArtifactStats {
            reuse_count: 0,
            recompute_count: 1,
            discard_count: 0,
        }
    );
}

#[test]
fn retained_layout_artifact_counters_record_recompute_and_reuse() {
    let mut page = page_with_dom(
        "<!doctype html><html><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );
    seed_retained_layout_for_test(&mut page);

    let artifact = page
        .retained_layout_artifact()
        .expect("retained layout artifact should exist")
        .clone();
    page.record_layout_frame_result(RetainedLayoutFrameResult {
        key: artifact.key(),
        action: RetainedLayoutFrameAction::Reused,
        artifact,
    });

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(snapshot.layout_tree, RenderArtifactState::RetainedFresh);
    assert_eq!(
        snapshot.layout_artifacts.last_action,
        RetainedLayoutArtifactAction::Reused
    );
    assert_eq!(snapshot.layout_artifacts.stats.recompute_count, 1);
    assert_eq!(snapshot.layout_artifacts.stats.reuse_count, 1);
    assert_eq!(snapshot.layout_artifacts.stats.discard_count, 0);
}

#[test]
fn retained_paint_artifact_counters_record_recompute_and_reuse() {
    let mut page = page_with_dom(
        "<!doctype html><html><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );
    seed_retained_paint_for_test(&mut page);

    let snapshot = page.retained_render_state_debug_snapshot();
    let key = snapshot
        .paint_artifacts
        .key
        .expect("retained paint artifact should have key");
    let artifact = page
        .retained_paint_artifact()
        .expect("retained paint artifact should exist")
        .clone();
    page.record_paint_frame_result(RetainedPaintFrameResult {
        key,
        action: RetainedPaintFrameAction::Reused,
        artifact,
    });

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(snapshot.paint_output, RenderArtifactState::RetainedFresh);
    assert_eq!(
        snapshot.paint_artifacts.last_action,
        RetainedPaintArtifactAction::Reused
    );
    assert_eq!(snapshot.paint_artifacts.stats.recompute_count, 1);
    assert_eq!(snapshot.paint_artifacts.stats.reuse_count, 1);
    assert_eq!(snapshot.paint_artifacts.stats.discard_count, 0);
}

#[test]
fn paint_only_style_change_preserves_layout_and_plans_retained_layout_reuse() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>.paint { background-color: red; }</style></head><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );
    seed_retained_paint_for_test(&mut page);
    page.clear_all_dirty_for_tests();

    let dirty_id = {
        let dom = page.dom.as_deref_mut().expect("dom should exist");
        set_first_element_attr(dom, "p", "class", Some("paint".to_string()))
    };
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    let prepared = page
        .prepare_style_phase_for_frame(&PendingRenderWork::default())
        .expect("frame preparation should succeed")
        .expect("document should produce a frame");
    assert!(
        !prepared
            .work_plan
            .dirty_state
            .is_phase_dirty(DirtyPhase::Layout)
    );
    assert!(
        prepared
            .work_plan
            .dirty_state
            .is_phase_dirty(DirtyPhase::Paint)
    );
    assert_eq!(
        prepared.work_plan.relayout.decision,
        RenderWorkDecision::ReuseRetainedLayout
    );
    assert_eq!(
        prepared.work_plan.relayout_execution,
        RelayoutExecution::ReuseRetained
    );
    assert_eq!(
        prepared.work_plan.repaint.decision,
        RenderWorkDecision::Repaint
    );
    assert_eq!(
        prepared.work_plan.repaint_execution,
        RepaintExecution::FullDocument
    );
    let p = find_styled_element(prepared.style_output.root(), "p").expect("p should exist");
    assert_eq!(p.style.background_color(), (255, 0, 0, 255));
    drop(prepared);

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(
        snapshot.layout_artifacts.state,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        snapshot.paint_artifacts.state,
        RenderArtifactState::RetainedStale
    );
}

#[test]
fn layout_affecting_style_change_forces_paint_recompute_planning() {
    let mut page = page_with_dom(
        "<!doctype html><html><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );
    seed_retained_paint_for_test(&mut page);
    page.clear_all_dirty_for_tests();

    let dirty_id = {
        let dom = page.dom.as_deref_mut().expect("dom should exist");
        set_first_element_attr(
            dom,
            "p",
            "style",
            Some("display: block; width: 140px;".to_string()),
        )
    };
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    let prepared = page
        .prepare_style_phase_for_frame(&PendingRenderWork::default())
        .expect("frame preparation should succeed")
        .expect("document should produce a frame");
    assert!(
        prepared
            .work_plan
            .dirty_state
            .is_phase_dirty(DirtyPhase::Layout)
    );
    assert!(
        prepared
            .work_plan
            .dirty_state
            .is_phase_dirty(DirtyPhase::Paint)
    );
    assert_eq!(
        prepared.work_plan.relayout.decision,
        RenderWorkDecision::Relayout
    );
    assert_eq!(
        prepared.work_plan.repaint.decision,
        RenderWorkDecision::Repaint
    );
    drop(prepared);

    assert_eq!(
        page.retained_render_state_debug_snapshot()
            .paint_artifacts
            .state,
        RenderArtifactState::RetainedStale
    );
}

#[test]
fn stacking_order_affecting_style_change_invalidates_retained_paint() {
    let mut page = page_with_dom(
        "<!doctype html><html><body><div style=\"position: relative; z-index: 1;\">One</div><div style=\"position: relative; z-index: 2;\">Two</div></body></html>",
    );
    seed_retained_paint_for_test(&mut page);
    page.clear_all_dirty_for_tests();

    let dirty_id = {
        let dom = page.dom.as_deref_mut().expect("dom should exist");
        set_first_element_attr(
            dom,
            "div",
            "style",
            Some("position: relative; z-index: 5;".to_string()),
        )
    };
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    let prepared = page
        .prepare_style_phase_for_frame(&PendingRenderWork::default())
        .expect("frame preparation should succeed")
        .expect("document should produce a frame");
    assert_eq!(
        prepared.work_plan.repaint.decision,
        RenderWorkDecision::Repaint
    );
    assert!(
        prepared
            .work_plan
            .dirty_state
            .entries()
            .contains(&DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            ))
    );
    drop(prepared);

    assert_eq!(
        page.retained_render_state_debug_snapshot()
            .paint_artifacts
            .state,
        RenderArtifactState::RetainedStale
    );
}

#[test]
fn retained_paint_recomputes_when_actual_layout_reuse_key_mismatches() {
    let mut page = page_with_dom(
        "<!doctype html><html><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );
    seed_retained_paint_for_test(&mut page);
    page.clear_all_dirty_for_tests();

    let old_paint_key = page
        .retained_render_state_debug_snapshot()
        .paint_artifacts
        .key
        .expect("seeded retained paint should have a key");
    let prepared = prepare_page_frame(&mut page, PendingRenderWork::default())
        .expect("frame preparation should succeed")
        .expect("document should produce a frame");
    assert_eq!(
        prepared.work_plan.repaint_execution,
        RepaintExecution::ReuseRetained
    );
    assert_eq!(
        prepared.work_plan.relayout_execution,
        RelayoutExecution::ReuseRetained
    );

    let mut input_state = DocumentInputState::new();
    let resources = ResourceManager::new();
    let ctx = Context::default();
    let mut prepared = Some(prepared);
    let mut outcome = None;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::new(0.0, 0.0),
                Vec2::new(640.0, 480.0),
            )),
            ..RawInput::default()
        },
        |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                outcome = Some(execute_prepared_page_frame(
                    ui,
                    prepared.take().expect("prepared frame should execute once"),
                    &mut input_state,
                    &resources,
                ));
            });
        },
    );
    let mut outcome = outcome.expect("frame should execute");

    let retained_layout_result = outcome
        .retained_layout_result
        .as_ref()
        .expect("retained layout result should be reported");
    assert_ne!(retained_layout_result.key, old_paint_key.layout_key);
    assert_eq!(
        retained_layout_result.action,
        RetainedLayoutFrameAction::ConservativeFallback(RetainedLayoutFallbackReason::KeyMismatch)
    );
    let actual_layout_key = retained_layout_result.key;

    let retained_paint_result = outcome
        .retained_paint_result
        .as_ref()
        .expect("retained paint result should be reported");
    assert_eq!(retained_paint_result.key.layout_key, actual_layout_key);
    assert_ne!(
        retained_paint_result.key.layout_key,
        old_paint_key.layout_key
    );
    assert_eq!(
        retained_paint_result.action,
        RetainedPaintFrameAction::Recomputed
    );
    assert_ne!(
        outcome.trace.paint.kind,
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    );
    assert!(
        !outcome
            .trace
            .to_debug_snapshot()
            .contains("paint: phase=paint kind=materialized-from-retained-artifacts")
    );

    page.record_layout_frame_result(
        outcome
            .retained_layout_result
            .take()
            .expect("retained layout result should be recorded"),
    );
    page.record_paint_frame_result(
        outcome
            .retained_paint_result
            .take()
            .expect("retained paint result should be recorded"),
    );

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(
        snapshot
            .paint_artifacts
            .key
            .expect("recomputed retained paint should have a key")
            .layout_key,
        actual_layout_key
    );
    assert_eq!(
        snapshot.paint_artifacts.last_action,
        RetainedPaintArtifactAction::Recomputed
    );
    assert_eq!(snapshot.paint_artifacts.stats.reuse_count, 0);
    assert_eq!(snapshot.paint_artifacts.stats.recompute_count, 2);
}

#[test]
fn paint_only_style_change_preserves_existing_viewport_layout_dirty_entry() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>.paint { background-color: red; }</style></head><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );
    seed_retained_layout_for_test(&mut page);
    page.clear_all_dirty_for_tests();
    page.mark_render_entry_point_for_tests(RenderInvalidationEntryPoint::ViewportChanged);

    let dirty_id = {
        let dom = page.dom.as_deref_mut().expect("dom should exist");
        set_first_element_attr(dom, "p", "class", Some("paint".to_string()))
    };
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    let prepared = page
        .prepare_style_phase_for_frame(&PendingRenderWork::default())
        .expect("frame preparation should succeed")
        .expect("document should produce a frame");

    assert!(
        prepared
            .work_plan
            .dirty_state
            .entries()
            .contains(&DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::ViewportChanged,
                DirtyScope::Viewport,
            )),
        "paint-only style narrowing must preserve unrelated viewport layout dirtiness"
    );
    assert!(
        !prepared
            .work_plan
            .dirty_state
            .entries()
            .contains(&DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::CascadedFromStyle,
                DirtyScope::Document,
            )),
        "paint-only style narrowing should remove only the style-derived layout cascade"
    );
    assert!(
        prepared
            .work_plan
            .dirty_state
            .entries()
            .contains(&DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::PaintOnlyStyleChanged,
                DirtyScope::Document,
            ))
    );
    assert_eq!(
        prepared.work_plan.relayout.decision,
        RenderWorkDecision::ConservativeFallback,
        "retained layout reuse is only valid when no other layout dirtiness remains"
    );
    assert_eq!(
        prepared.work_plan.relayout_execution,
        RelayoutExecution::ConservativeDocumentFallback {
            requested_scope: DirtyScope::Viewport,
            reason: RenderWorkFallbackReason::TargetedRelayoutNotExecutable {
                scope: DirtyScope::Viewport,
            },
        }
    );
    assert!(
        prepared.work_plan.to_debug_snapshot().contains(
            "relayout-execution: strategy=conservative-document-fallback requested-scope=viewport reason=targeted-relayout-not-executable(viewport)\n"
        ),
        "debug output must distinguish requested relayout scope from conservative execution fallback"
    );
}

#[test]
fn layout_affecting_style_change_marks_layout_dirty() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>.wide { width: 120px; }</style></head><body><p style=\"display: block;\">Hello</p></body></html>",
    );
    seed_retained_layout_for_test(&mut page);
    page.clear_all_dirty_for_tests();

    let dirty_id = {
        let dom = page.dom.as_deref_mut().expect("dom should exist");
        set_first_element_attr(dom, "p", "class", Some("wide".to_string()))
    };
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    let prepared = page
        .prepare_style_phase_for_frame(&PendingRenderWork::default())
        .expect("frame preparation should succeed")
        .expect("document should produce a frame");
    assert!(
        prepared
            .work_plan
            .dirty_state
            .is_phase_dirty(DirtyPhase::Layout)
    );
    assert_eq!(prepared.work_plan.relayout.scope, DirtyScope::Document);
    assert_eq!(
        prepared.work_plan.relayout_execution,
        RelayoutExecution::FullDocument
    );
}

#[test]
fn retained_render_state_survives_noop_render_without_epoch_churn() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );

    let initial_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(initial_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(initial_style);

    let before_noop = page.retained_render_state_debug_snapshot();
    assert!(before_noop.render_epoch > RenderEpoch::initial());
    assert_eq!(
        before_noop.resolved_styles,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        before_noop.computed_styles,
        RenderArtifactState::RetainedFresh
    );
    assert!(!before_noop.style_dirty);
    assert_eq!(before_noop.style_artifacts.stats.reuse_count, 0);
    assert_eq!(before_noop.style_artifacts.stats.recompute_count, 1);

    let noop_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(noop_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(noop_style);

    let after_noop = page.retained_render_state_debug_snapshot();
    assert_eq!(after_noop.render_epoch, before_noop.render_epoch);
    assert_eq!(after_noop.style_artifacts.stats.reuse_count, 1);
    assert_eq!(after_noop.style_artifacts.stats.recompute_count, 1);
    assert_eq!(
        after_noop.style_artifacts.last_action,
        RetainedStyleArtifactAction::Reused
    );
    assert_eq!(
        after_noop.style_artifacts.key,
        before_noop.style_artifacts.key
    );
}

#[test]
fn viewport_update_reuses_retained_style_artifacts_without_restyle() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(initial_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(initial_style);
    page.clear_all_dirty_for_tests();

    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ViewportChanged,
    ));
    let plan = page.derive_render_work_plan(&pending);
    assert_eq!(
        plan.restyle.decision,
        RenderWorkDecision::ReuseRetainedStyle
    );
    assert_eq!(
        plan.relayout.decision,
        RenderWorkDecision::ConservativeFallback
    );
    assert_eq!(
        plan.relayout_execution,
        RelayoutExecution::ConservativeDocumentFallback {
            requested_scope: DirtyScope::Viewport,
            reason: RenderWorkFallbackReason::TargetedRelayoutNotExecutable {
                scope: DirtyScope::Viewport,
            },
        }
    );
    assert_eq!(plan.repaint.decision, RenderWorkDecision::Repaint);

    let before = page.retained_render_state_debug_snapshot();
    let viewport_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(viewport_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(viewport_style);

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(after.render_epoch, before.render_epoch);
    assert!(!after.style_dirty);
    assert_eq!(after.style_artifacts.stats.reuse_count, 1);
    assert_eq!(
        after.style_artifacts.last_action,
        RetainedStyleArtifactAction::Reused
    );
}

#[test]
fn noop_update_does_not_mark_clean_retained_dirty_state() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(initial_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(initial_style);

    page.clear_all_dirty_for_tests();
    assert!(
        page.retained_render_state_debug_snapshot()
            .dirty_state
            .entries
            .is_empty()
    );

    let noop_style = style_output_for_test(&mut page);
    assert_eq!(
        styled_element_color(noop_style.root(), "p"),
        (255, 0, 0, 255)
    );
    drop(noop_style);

    let after = page.retained_render_state_debug_snapshot();
    assert!(!after.style_dirty);
    assert!(!after.layout_dirty);
    assert!(!after.paint_dirty);
    assert!(after.dirty_state.entries.is_empty());
    assert_eq!(after.style_artifacts.stats.reuse_count, 1);
    assert_eq!(
        after.style_artifacts.last_action,
        RetainedStyleArtifactAction::Reused
    );
}

#[test]
fn ac8_retained_snapshot_reports_actual_reuse_summaries_and_generations() {
    let mut page = page_with_dom(
        "<!doctype html><html><body><p style=\"display: block; width: 100px; color: red;\">Hello</p></body></html>",
    );
    seed_retained_paint_for_test(&mut page);
    page.clear_all_dirty_for_tests();

    let reused_style = style_output_for_test(&mut page);
    drop(reused_style);

    let before_reuse = page.retained_render_state_debug_snapshot();
    let layout_key = before_reuse
        .layout_artifacts
        .key
        .expect("retained layout should have key");
    let layout_artifact = page
        .retained_layout_artifact()
        .expect("retained layout artifact should exist")
        .clone();
    page.record_layout_frame_result(RetainedLayoutFrameResult {
        key: layout_key,
        action: RetainedLayoutFrameAction::Reused,
        artifact: layout_artifact,
    });

    let before_paint_reuse = page.retained_render_state_debug_snapshot();
    let paint_key = before_paint_reuse
        .paint_artifacts
        .key
        .expect("retained paint should have key");
    let paint_artifact = page
        .retained_paint_artifact()
        .expect("retained paint artifact should exist")
        .clone();
    page.record_paint_frame_result(RetainedPaintFrameResult {
        key: paint_key,
        action: RetainedPaintFrameAction::Reused,
        artifact: paint_artifact,
    });

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(snapshot.generations.dom_generation, 1);
    assert_eq!(snapshot.generations.style_input_generation, 1);
    assert_eq!(snapshot.generations.layout_input_generation, 1);
    assert_eq!(snapshot.generations.layout_style_generation, 0);
    assert_eq!(snapshot.generations.paint_style_generation, 0);
    assert_eq!(snapshot.generations.paint_input_generation, 0);
    assert_eq!(
        snapshot.style_artifacts.last_action,
        RetainedStyleArtifactAction::Reused
    );
    assert_eq!(snapshot.style_artifacts.stats.reuse_count, 1);
    assert_eq!(snapshot.style_artifacts.stats.recompute_count, 1);
    assert_eq!(
        snapshot.layout_artifacts.last_action,
        RetainedLayoutArtifactAction::Reused
    );
    assert_eq!(snapshot.layout_artifacts.stats.reuse_count, 1);
    assert_eq!(snapshot.layout_artifacts.stats.recompute_count, 1);
    assert_eq!(
        snapshot.paint_artifacts.last_action,
        RetainedPaintArtifactAction::Reused
    );
    assert_eq!(snapshot.paint_artifacts.stats.reuse_count, 1);
    assert_eq!(snapshot.paint_artifacts.stats.recompute_count, 1);

    let rendered = snapshot.to_debug_snapshot();
    assert!(rendered.contains("generations:\n"));
    assert!(rendered.contains("  dom-generation: 1\n"));
    assert!(rendered.contains("  style-input-generation: 1\n"));
    assert!(rendered.contains("  layout-input-generation: 1\n"));
    assert!(rendered.contains("  paint-input-generation: 0\n"));
    assert!(rendered.contains("style-artifacts:\n"));
    assert!(rendered.contains("  last-action: reused\n"));
    assert!(rendered.contains("layout-artifacts:\n"));
    assert!(rendered.contains("paint-artifacts:\n"));
}

#[test]
fn ac8_retained_snapshot_is_stable_across_equivalent_incremental_runs() {
    fn rendered_snapshot_after_equivalent_update() -> String {
        let mut page = page_with_dom(
            "<!doctype html><html><body><p style=\"display: block; width: 100px; color: red;\">Hello</p></body></html>",
        );
        seed_retained_paint_for_test(&mut page);
        page.clear_all_dirty_for_tests();
        page.mark_render_entry_point_for_tests(RenderInvalidationEntryPoint::InputStateChanged);
        page.retained_render_state_debug_snapshot()
            .to_debug_snapshot()
    }

    assert_eq!(
        rendered_snapshot_after_equivalent_update(),
        rendered_snapshot_after_equivalent_update()
    );
}

#[test]
fn same_document_text_update_preserves_surviving_retained_identity() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    let text_identity = identity_for_dom_anchor(&before, Id(5));

    replace_first_text(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "Hello",
        "Goodbye",
    );
    page.mark_dom_changed_for_tests(RestyleHint::text_mutated());

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert_eq!(identity_for_dom_anchor(&after, Id(5)), text_identity);
}

#[test]
fn same_document_class_update_preserves_surviving_retained_identity() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    let element_identity = identity_for_dom_anchor(&before, Id(4));

    let dirty_id = set_first_element_attr(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        "class",
        Some("hot".to_string()),
    );
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert_eq!(identity_for_dom_anchor(&after, Id(4)), element_identity);
}

#[test]
fn same_document_removal_prunes_stale_retained_identity() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    assert!(identity_for_dom_anchor_optional(&before, Id(4)).is_some());
    assert!(identity_for_dom_anchor_optional(&before, Id(5)).is_some());

    remove_first_element(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
    );
    page.mark_dom_changed_for_tests(RestyleHint::tree_mutated());

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert!(identity_for_dom_anchor_optional(&after, Id(4)).is_none());
    assert!(identity_for_dom_anchor_optional(&after, Id(5)).is_none());
    assert_eq!(
        after.retained_identities,
        vec![
            retained_identity(1, 1),
            retained_identity(2, 2),
            retained_identity(3, 3),
        ]
    );
}

#[test]
fn same_document_replacement_allocates_new_retained_identity_without_recycling_removed_ids() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    let removed_identity = identity_for_dom_anchor(&before, Id(4));

    replace_first_element(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        paragraph_node(6, 7, "Replacement"),
    );
    page.mark_dom_changed_for_tests(RestyleHint::tree_mutated());

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after.retained_identity_domain,
        before.retained_identity_domain
    );
    assert!(identity_for_dom_anchor_optional(&after, Id(4)).is_none());
    assert_ne!(identity_for_dom_anchor(&after, Id(6)), removed_identity);
    assert_eq!(identity_for_dom_anchor(&after, Id(6)).id.value(), 6);
    assert_eq!(identity_for_dom_anchor(&after, Id(7)).id.value(), 7);
}

#[test]
fn full_document_replacement_starts_new_identity_domain_even_when_dom_ids_match() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    assert_eq!(before.retained_identity_domain.value(), 1);

    let _ = page.replace_dom(
        Box::new(doc_with_explicit_ids()),
        RestyleHint::document_replaced(),
    );

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(after.retained_identity_domain.value(), 2);
    assert_eq!(
        after.retained_identities,
        vec![
            retained_identity(1, 1),
            retained_identity(2, 2),
            retained_identity(3, 3),
            retained_identity(4, 4),
            retained_identity(5, 5),
        ]
    );
    assert_ne!(
        after.retained_identity_domain, before.retained_identity_domain,
        "matching DOM anchor numbers across replace_dom must not prove retained continuity"
    );
}

#[test]
fn document_replacement_discards_style_artifacts_across_identity_domains() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let first_style = style_output_for_test(&mut page);
    drop(first_style);
    let before = page.retained_render_state_debug_snapshot();
    assert_eq!(before.style_artifacts.stats.recompute_count, 1);
    assert_eq!(before.style_artifacts.stats.discard_count, 0);

    let _ = page.replace_dom(
        Box::new(doc_with_explicit_ids()),
        RestyleHint::document_replaced(),
    );

    let discarded = page.retained_render_state_debug_snapshot();
    assert_ne!(
        discarded.retained_identity_domain, before.retained_identity_domain,
        "full document replacement must start a new retained identity domain"
    );
    assert_eq!(discarded.style_artifacts.key, None);
    assert_eq!(discarded.style_artifacts.stats.discard_count, 1);
    assert_eq!(
        discarded.style_artifacts.last_action,
        RetainedStyleArtifactAction::DiscardedForFullInvalidation
    );

    let second_style = style_output_for_test(&mut page);
    drop(second_style);
    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(
        after
            .style_artifacts
            .key
            .expect("style key after replacement recompute")
            .identity_domain,
        after.retained_identity_domain
    );
    assert_eq!(after.style_artifacts.stats.reuse_count, 0);
    assert_eq!(after.style_artifacts.stats.recompute_count, 2);
    assert_eq!(after.style_artifacts.stats.discard_count, 1);
    assert_eq!(
        after.style_artifacts.last_action,
        RetainedStyleArtifactAction::FullRecompute
    );
}

#[test]
fn replace_dom_enforces_identity_boundary_even_with_non_document_replaced_hint() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let before = page.retained_render_state_debug_snapshot();
    assert_eq!(before.retained_identity_domain.value(), 1);
    assert_eq!(identity_for_dom_anchor(&before, Id(4)).id.value(), 4);

    let _ = page.replace_dom(
        Box::new(doc_with_explicit_ids()),
        RestyleHint::text_mutated(),
    );

    let after = page.retained_render_state_debug_snapshot();
    assert_eq!(after.retained_identity_domain.value(), 2);
    assert_eq!(identity_for_dom_anchor(&after, Id(4)).id.value(), 4);
    assert_ne!(
        after.retained_identity_domain, before.retained_identity_domain,
        "replace_dom must isolate retained identities even if the caller supplies a non-boundary hint"
    );
}

#[test]
fn retained_render_epoch_advances_when_failed_recompute_consumes_pending_invalidation() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>div { width: 1e39px; }</style></head><body><div>bad</div></body></html>",
    );
    let before = page.retained_render_state_debug_snapshot();
    assert_eq!(before.style_invalidation, StyleInvalidationState::Full);

    let error = match page.build_style_phase_output() {
        Ok(_) => panic!("style recomputation must fail on out-of-range computed width"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("length-out-of-range"),
        "unexpected style recomputation error: {error}"
    );

    let after = page.retained_render_state_debug_snapshot();
    assert!(
        after.render_epoch > before.render_epoch,
        "consuming retained style invalidation must advance the render epoch even when recomputation fails"
    );
    assert!(after.style_dirty);
    assert_eq!(after.style_invalidation, StyleInvalidationState::None);
}

#[test]
fn retained_render_state_debug_snapshot_does_not_expose_frame_local_ids() {
    let mut page = page_with_node(doc_with_explicit_ids());
    let style_output = style_output_for_test(&mut page);
    drop(style_output);

    let snapshot = page
        .retained_render_state_debug_snapshot()
        .to_debug_snapshot();

    for forbidden in [
        "LayoutBox",
        "BoxId",
        "StackingContextId",
        "stacking-context-id=",
        "PaintId",
        "PaintPrimitiveId",
        "source-order-id=",
        "traversal-id=",
        "paint-operation-index",
        "paint-order-index",
        "0x",
    ] {
        assert!(
            !snapshot.contains(forbidden),
            "retained render state debug snapshot must not expose {forbidden}"
        );
    }

    assert!(snapshot.contains("  frame-local-layout-ids: not-retained\n"));
    assert!(snapshot.contains("  frame-local-paint-ids: not-retained\n"));
    assert!(snapshot.contains("  frame-local-stacking-ids: not-retained\n"));
    assert!(snapshot.contains("  frame-local-traversal-source-order-ids: not-retained\n"));
    assert!(snapshot.contains("anchor=dom-node(4)"));
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
    assert_eq!(
        stale.style_artifacts.state,
        RenderArtifactState::RetainedStale
    );
    assert_eq!(
        stale.style_artifacts.last_action,
        RetainedStyleArtifactAction::InitialCompute
    );
    assert_eq!(stale.style_artifacts.stats.recompute_count, 1);
    assert!(stale.style_dirty);
    assert!(stale.layout_dirty);
    assert!(stale.paint_dirty);
    assert_eq!(
        stale.dirty_state.entries,
        vec![
            DirtyEntry::new(
                DirtyPhase::Style,
                DirtyReason::StyleInputChanged,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::CascadedFromStyle,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            ),
        ]
    );

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
    assert_eq!(
        refreshed.style_artifacts.last_action,
        RetainedStyleArtifactAction::IncrementalSuffixRecompute
    );
    assert_eq!(refreshed.style_artifacts.stats.recompute_count, 2);
    assert_eq!(refreshed.style_artifacts.stats.discard_count, 0);
}

#[test]
fn inline_style_attribute_change_uses_supported_attribute_suffix_scope() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: black; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(initial.root(), "p"), (0, 0, 0, 255));
    drop(initial);

    let p_id = set_first_element_attr(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        "style",
        Some("color: red;".to_string()),
    );
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![p_id]));

    let stale = page.render_pipeline_debug_snapshot();
    assert_eq!(
        stale.style_invalidation,
        StyleInvalidationState::AttributeSuffix
    );
    assert_eq!(
        stale.style_artifacts.state,
        RenderArtifactState::RetainedStale
    );

    let restyled = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(restyled.root(), "p"), (255, 0, 0, 255));
    drop(restyled);

    let refreshed = page.render_pipeline_debug_snapshot();
    assert_eq!(
        refreshed.style_artifacts.last_action,
        RetainedStyleArtifactAction::IncrementalSuffixRecompute
    );
    assert_eq!(refreshed.style_artifacts.stats.recompute_count, 2);
}

#[test]
fn attribute_change_without_dirty_nodes_falls_back_to_full_style_invalidation() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>.hot { color: red; } p { color: black; }</style></head><body><p>Hello</p></body></html>",
    );
    let initial = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(initial.root(), "p"), (0, 0, 0, 255));
    drop(initial);

    set_first_element_attr(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "p",
        "class",
        Some("hot".to_string()),
    );
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(Vec::new()));

    let invalidated = page.render_pipeline_debug_snapshot();
    assert_eq!(invalidated.resolved_styles, RenderArtifactState::Absent);
    assert_eq!(invalidated.computed_styles, RenderArtifactState::Absent);
    assert_eq!(invalidated.style_invalidation, StyleInvalidationState::Full);
    assert_eq!(invalidated.style_artifacts.key, None);
    assert_eq!(invalidated.style_artifacts.stats.discard_count, 1);
    assert_eq!(
        invalidated.style_artifacts.last_action,
        RetainedStyleArtifactAction::DiscardedForFullInvalidation
    );

    let restyled = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(restyled.root(), "p"), (255, 0, 0, 255));
    drop(restyled);

    let refreshed = page.render_pipeline_debug_snapshot();
    assert_eq!(
        refreshed.style_artifacts.last_action,
        RetainedStyleArtifactAction::FullRecompute
    );
    assert_eq!(refreshed.style_artifacts.stats.recompute_count, 2);
    assert_eq!(refreshed.style_artifacts.stats.discard_count, 1);
}

#[test]
fn stylesheet_update_discards_retained_style_artifacts() {
    let mut page = page_with_dom("<!doctype html><html><body><p>Hello</p></body></html>");
    let initial = style_output_for_test(&mut page);
    drop(initial);

    let slot = page.register_css("https://example.com/style.css");
    let invalidation = page.apply_css_block(slot, "p { color: red; }");
    assert!(invalidation.is_some());

    let discarded = page.render_pipeline_debug_snapshot();
    assert_eq!(discarded.resolved_styles, RenderArtifactState::Absent);
    assert_eq!(discarded.computed_styles, RenderArtifactState::Absent);
    assert_eq!(discarded.style_invalidation, StyleInvalidationState::Full);
    assert_eq!(discarded.style_artifacts.key, None);
    assert_eq!(discarded.style_artifacts.stats.discard_count, 1);
    assert_eq!(
        discarded.style_artifacts.last_action,
        RetainedStyleArtifactAction::DiscardedForFullInvalidation
    );

    let restyled = style_output_for_test(&mut page);
    assert_eq!(styled_element_color(restyled.root(), "p"), (255, 0, 0, 255));
    drop(restyled);

    let refreshed = page.render_pipeline_debug_snapshot();
    assert_eq!(
        refreshed.style_artifacts.last_action,
        RetainedStyleArtifactAction::FullRecompute
    );
    assert_eq!(refreshed.style_artifacts.stats.recompute_count, 2);
    assert_eq!(refreshed.style_artifacts.stats.discard_count, 1);
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
    assert!(snapshot.paint_dirty);
    assert_eq!(
        snapshot.dirty_state.entries,
        vec![
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::TextContentChanged,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            ),
        ]
    );
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
            dirty_state: DirtyStateDebugSnapshot {
                entries: vec![
                    DirtyEntry::new(
                        DirtyPhase::Style,
                        DirtyReason::ConservativeUnknownImpact,
                        DirtyScope::Document,
                    ),
                    DirtyEntry::new(
                        DirtyPhase::Layout,
                        DirtyReason::CascadedFromStyle,
                        DirtyScope::Document,
                    ),
                    DirtyEntry::new(
                        DirtyPhase::Paint,
                        DirtyReason::CascadedFromLayout,
                        DirtyScope::Document,
                    ),
                ],
            },
            style_dirty: true,
            layout_dirty: true,
            paint_dirty: true,
            style_invalidation: StyleInvalidationState::Full,
            generations: RetainedRenderGenerationDebugSnapshot::default(),
            style_artifacts: RetainedStyleArtifactDebugSnapshot {
                key: None,
                state: RenderArtifactState::Absent,
                last_action: RetainedStyleArtifactAction::None,
                stats: RetainedStyleArtifactStats::default(),
            },
            layout_artifacts: RetainedLayoutArtifactDebugSnapshot {
                key_seed: layout::RetainedLayoutKeySeed {
                    identity_domain: 0,
                    layout_input_generation: 0,
                    layout_style_generation: 0,
                    text_measurement_generation: 0,
                    replaced_metadata_generation: 0,
                },
                key: None,
                state: RenderArtifactState::Absent,
                last_action: RetainedLayoutArtifactAction::None,
                stats: RetainedLayoutArtifactStats::default(),
            },
            paint_artifacts: RetainedPaintArtifactDebugSnapshot {
                key: None,
                state: RenderArtifactState::Absent,
                last_action: RetainedPaintArtifactAction::None,
                stats: RetainedPaintArtifactStats::default(),
            },
        }
    );
    assert_eq!(
        page.retained_render_state_debug_snapshot().render_epoch,
        RenderEpoch::initial()
    );
    assert_eq!(
        page.retained_render_state_debug_snapshot()
            .retained_identity_domain,
        RetainedRenderIdentityDomain::initial()
    );
    assert!(
        page.retained_render_state_debug_snapshot()
            .retained_identities
            .is_empty()
    );
}

fn retained_identity(retained_id: u64, dom_anchor: u32) -> RetainedRenderIdentity {
    RetainedRenderIdentity {
        id: RetainedRenderId::from_raw(retained_id),
        kind: RetainedRenderArtifactKind::DomBackedRenderNode,
        anchor: RetainedRenderAnchor::DomNode(Id(dom_anchor)),
    }
}

fn seed_retained_layout_for_test(page: &mut PageState) {
    let key = page.retained_layout_key_seed().for_viewport_width(320.0);
    let style_output = style_output_for_test(page);
    let layout_output = layout::layout_document(LayoutPhaseInput::from_style_output(
        &style_output,
        320.0,
        &TestMeasurer,
        None,
    ));
    let artifact = layout::RetainedLayoutArtifact::from_layout_output(key, &layout_output);
    drop(layout_output);
    drop(style_output);
    page.record_layout_frame_result(RetainedLayoutFrameResult {
        key,
        action: RetainedLayoutFrameAction::Recomputed,
        artifact,
    });
}

fn seed_retained_paint_for_test(page: &mut PageState) {
    let layout_key = page.retained_layout_key_seed().for_viewport_width(320.0);
    let paint_key = page.retained_paint_key_seed().for_layout_key(layout_key);
    let style_output = style_output_for_test(page);
    let layout_output = layout::layout_document(LayoutPhaseInput::from_style_output(
        &style_output,
        320.0,
        &TestMeasurer,
        None,
    ));
    let layout_artifact =
        layout::RetainedLayoutArtifact::from_layout_output(layout_key, &layout_output);
    let paint_artifact =
        PaintArtifact::from_phase_input(PaintPhaseInput::new(&layout_output), &TestMeasurer);
    drop(layout_output);
    drop(style_output);
    page.record_layout_frame_result(RetainedLayoutFrameResult {
        key: layout_key,
        action: RetainedLayoutFrameAction::Recomputed,
        artifact: layout_artifact,
    });
    page.record_paint_frame_result(RetainedPaintFrameResult {
        key: paint_key,
        action: RetainedPaintFrameAction::Recomputed,
        artifact: paint_artifact,
    });
}

fn identity_for_dom_anchor(
    snapshot: &RetainedRenderStateDebugSnapshot,
    dom_anchor: Id,
) -> RetainedRenderIdentity {
    identity_for_dom_anchor_optional(snapshot, dom_anchor).expect("retained identity should exist")
}

fn identity_for_dom_anchor_optional(
    snapshot: &RetainedRenderStateDebugSnapshot,
    dom_anchor: Id,
) -> Option<RetainedRenderIdentity> {
    snapshot
        .retained_identities
        .iter()
        .copied()
        .find(|identity| identity.anchor == RetainedRenderAnchor::DomNode(dom_anchor))
}
