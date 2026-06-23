use crate::rendering::*;

use super::support::*;

fn derive_plan(
    retained_dirty_state: &RenderDirtyState,
    pending_work: &PendingRenderWork,
    retained_style_artifacts: RetainedStyleArtifactState,
) -> RenderWorkPlan {
    RenderWorkPlan::derive(RenderWorkPlanInput {
        has_dom: true,
        retained_style_artifacts,
        retained_dirty_state,
        pending_work,
    })
}

#[test]
fn no_op_dirty_state_plans_minimal_work() {
    let retained_dirty_state = RenderDirtyState::new();
    let pending_work = PendingRenderWork::default();

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(plan.entry_points, Vec::new());
    assert!(plan.dirty_state.is_empty());
    assert_eq!(
        plan.restyle.decision,
        RenderWorkDecision::ReuseRetainedStyle
    );
    assert_eq!(plan.restyle.scope, DirtyScope::None);
    assert_eq!(plan.relayout.decision, RenderWorkDecision::None);
    assert_eq!(plan.repaint.decision, RenderWorkDecision::None);
    assert_eq!(plan.conservative_fallback, None);
}

#[test]
fn viewport_dirty_state_plans_relayout_and_repaint_without_restyle() {
    let retained_dirty_state = RenderDirtyState::new();
    let mut pending_work = PendingRenderWork::default();
    pending_work.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ViewportChanged,
    ));

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(
        plan.entry_points,
        vec![RenderInvalidationEntryPoint::ViewportChanged]
    );
    assert!(!plan.dirty_state.is_phase_dirty(DirtyPhase::Style));
    assert_eq!(
        plan.restyle.decision,
        RenderWorkDecision::ReuseRetainedStyle
    );
    assert_eq!(plan.relayout.decision, RenderWorkDecision::Relayout);
    assert_eq!(plan.relayout.scope, DirtyScope::Viewport);
    assert_eq!(plan.repaint.decision, RenderWorkDecision::Repaint);
    assert_eq!(plan.repaint.scope, DirtyScope::Viewport);
    assert_eq!(plan.conservative_fallback, None);
}

#[test]
fn style_dirty_state_plans_restyle_relayout_and_repaint() {
    let mut retained_dirty_state = RenderDirtyState::new();
    retained_dirty_state.extend(
        dirty_request_for_entry_point(RenderInvalidationEntryPoint::DomAttributesChanged).entries,
    );
    let pending_work = PendingRenderWork::default();

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Stale,
    );

    assert_eq!(plan.restyle.decision, RenderWorkDecision::Restyle);
    assert_eq!(plan.restyle.scope, DirtyScope::Document);
    assert_eq!(plan.relayout.decision, RenderWorkDecision::Relayout);
    assert_eq!(plan.relayout.scope, DirtyScope::Document);
    assert_eq!(plan.repaint.decision, RenderWorkDecision::Repaint);
    assert_eq!(plan.repaint.scope, DirtyScope::Document);
}

#[test]
fn layout_dirty_state_plans_relayout_and_repaint_without_restyle() {
    let mut retained_dirty_state = RenderDirtyState::new();
    retained_dirty_state.extend(
        dirty_request_for_entry_point(RenderInvalidationEntryPoint::DomTextChanged).entries,
    );
    let pending_work = PendingRenderWork::default();

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(
        plan.restyle.decision,
        RenderWorkDecision::ReuseRetainedStyle
    );
    assert_eq!(plan.relayout.decision, RenderWorkDecision::Relayout);
    assert_eq!(plan.relayout.scope, DirtyScope::Document);
    assert_eq!(plan.repaint.decision, RenderWorkDecision::Repaint);
    assert_eq!(plan.repaint.scope, DirtyScope::Document);
}

#[test]
fn paint_dirty_state_plans_repaint_without_relayout() {
    let mut retained_dirty_state = RenderDirtyState::new();
    retained_dirty_state.extend(
        dirty_request_for_entry_point(RenderInvalidationEntryPoint::InputStateChanged).entries,
    );
    let pending_work = PendingRenderWork::default();

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(
        plan.restyle.decision,
        RenderWorkDecision::ReuseRetainedStyle
    );
    assert_eq!(plan.relayout.decision, RenderWorkDecision::None);
    assert_eq!(plan.repaint.decision, RenderWorkDecision::Repaint);
    assert_eq!(plan.repaint.scope, DirtyScope::Viewport);
}

#[test]
fn unknown_dirty_state_plans_visible_conservative_fallback() {
    let retained_dirty_state = RenderDirtyState::conservative_unknown();
    let pending_work = PendingRenderWork::default();

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(
        plan.conservative_fallback,
        Some(RenderWorkFallbackReason::ConservativeUnknownImpact {
            scope: DirtyScope::Document,
        })
    );
    assert_eq!(
        plan.restyle.decision,
        RenderWorkDecision::ConservativeFallback
    );
    assert_eq!(
        plan.relayout.decision,
        RenderWorkDecision::ConservativeFallback
    );
    assert_eq!(
        plan.repaint.decision,
        RenderWorkDecision::ConservativeFallback
    );
}

#[test]
fn planner_normalizes_retained_dirty_state_then_pending_work_deterministically() {
    let mut retained_dirty_state = RenderDirtyState::new();
    retained_dirty_state.push(DirtyEntry::new(
        DirtyPhase::Paint,
        DirtyReason::RuntimeInputState,
        DirtyScope::Viewport,
    ));
    retained_dirty_state.push(DirtyEntry::new(
        DirtyPhase::Layout,
        DirtyReason::ViewportChanged,
        DirtyScope::Viewport,
    ));

    let mut pending_work = PendingRenderWork::default();
    pending_work.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ViewportChanged,
    ));
    pending_work.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(
        plan.dirty_state.entries(),
        &[
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::ViewportChanged,
                DirtyScope::Viewport,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Viewport,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::RuntimeInputState,
                DirtyScope::Viewport,
            ),
        ]
    );
}

#[test]
fn render_work_plan_debug_snapshot_is_exact_for_viewport_update() {
    let retained_dirty_state = RenderDirtyState::new();
    let mut pending_work = PendingRenderWork::default();
    pending_work.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ViewportChanged,
    ));

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(
        plan.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "render-work-plan\n",
            "entry-points: 1\n",
            "  - viewport-changed\n",
            "canonical-dirty-state:\n",
            "  entries: 2\n",
            "    entry[0]: phase=layout reason=viewport-changed scope=viewport\n",
            "    entry[1]: phase=paint reason=cascaded-from-layout scope=viewport\n",
            "restyle: decision=reuse-retained-style scope=none\n",
            "  reasons: 2\n",
            "    - clean-dirty-state\n",
            "    - retained-style-artifact(computed-document-style)=fresh\n",
            "relayout: decision=relayout scope=viewport\n",
            "  reasons: 1\n",
            "    - dirty(viewport-changed)\n",
            "repaint: decision=repaint scope=viewport\n",
            "  reasons: 1\n",
            "    - dirty(cascaded-from-layout)\n",
            "conservative-fallback: none\n",
        )
    );
}

#[test]
fn render_work_plan_debug_snapshot_is_exact_for_conservative_unknown() {
    let retained_dirty_state = RenderDirtyState::conservative_unknown();
    let pending_work = PendingRenderWork::default();

    let plan = derive_plan(
        &retained_dirty_state,
        &pending_work,
        RetainedStyleArtifactState::Fresh,
    );

    assert_eq!(
        plan.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "render-work-plan\n",
            "entry-points: 0\n",
            "canonical-dirty-state:\n",
            "  entries: 3\n",
            "    entry[0]: phase=style reason=conservative-unknown-impact scope=document\n",
            "    entry[1]: phase=layout reason=conservative-unknown-impact scope=document\n",
            "    entry[2]: phase=paint reason=conservative-unknown-impact scope=document\n",
            "restyle: decision=conservative-fallback scope=document\n",
            "  reasons: 1\n",
            "    - dirty(conservative-unknown-impact)\n",
            "relayout: decision=conservative-fallback scope=document\n",
            "  reasons: 1\n",
            "    - dirty(conservative-unknown-impact)\n",
            "repaint: decision=conservative-fallback scope=document\n",
            "  reasons: 1\n",
            "    - dirty(conservative-unknown-impact)\n",
            "conservative-fallback: reason=conservative-unknown-impact scope=document\n",
        )
    );
}

#[test]
fn prepare_page_frame_carries_derived_work_plan_before_execution() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let warm_style = style_output_for_test(&mut page);
    drop(warm_style);
    page.clear_all_dirty_for_tests();

    let mut pending_work = PendingRenderWork::default();
    pending_work.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ViewportChanged,
    ));

    let prepared = prepare_page_frame(&mut page, pending_work)
        .expect("frame preparation should succeed")
        .expect("document should produce a prepared frame");
    let plan = &prepared.work_plan;

    assert_eq!(
        plan.entry_points,
        vec![RenderInvalidationEntryPoint::ViewportChanged]
    );
    assert!(!plan.dirty_state.is_phase_dirty(DirtyPhase::Style));
    assert_eq!(
        plan.restyle.decision,
        RenderWorkDecision::ReuseRetainedStyle
    );
    assert_eq!(plan.restyle.scope, DirtyScope::None);
    assert_eq!(plan.relayout.decision, RenderWorkDecision::Relayout);
    assert_eq!(plan.relayout.scope, DirtyScope::Viewport);
    assert_eq!(plan.repaint.decision, RenderWorkDecision::Repaint);
    assert_eq!(plan.repaint.scope, DirtyScope::Viewport);
    assert_eq!(plan.conservative_fallback, None);
}
