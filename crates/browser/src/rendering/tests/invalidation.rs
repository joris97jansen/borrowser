use crate::page::{PageState, RestyleHint};
use crate::rendering::*;
use html::{HtmlParseOptions, parse_document};

use super::support::*;

#[test]
fn render_invalidation_request_contracts_pin_runtime_entry_points() {
    let contracts = render_invalidation_request_contracts();
    assert_eq!(contracts.len(), 8);

    let attrs = render_invalidation_request(RenderInvalidationEntryPoint::DomAttributesChanged);
    assert_eq!(attrs.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        attrs.work.style,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomAttributesChanged)
    );
    assert_eq!(
        attrs.work.layout,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
    assert_eq!(
        attrs.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );

    let text = render_invalidation_request(RenderInvalidationEntryPoint::DomTextChanged);
    assert_eq!(text.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(text.work.style, PhaseRerunSource::None);
    assert_eq!(
        text.work.layout,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );
    assert_eq!(
        text.work.frame_orchestration,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );

    let input = render_invalidation_request(RenderInvalidationEntryPoint::InputStateChanged);
    assert_eq!(input.requested_by, RenderingSubsystem::BrowserView);
    assert_eq!(input.work.style, PhaseRerunSource::None);
    assert_eq!(input.work.layout, PhaseRerunSource::None);
    assert_eq!(
        input.work.paint,
        PhaseRerunSource::Direct(RenderRebuildTrigger::InputStateChanged)
    );

    let resource = render_invalidation_request(RenderInvalidationEntryPoint::ResourceStateChanged);
    assert_eq!(resource.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        resource.work.layout,
        PhaseRerunSource::Direct(RenderRebuildTrigger::ResourceStateChanged)
    );
    assert_eq!(
        resource.work.paint,
        PhaseRerunSource::Direct(RenderRebuildTrigger::ResourceStateChanged)
    );
}

#[test]
fn render_invalidation_request_contracts_cover_each_entry_point_once() {
    let contracts = render_invalidation_request_contracts();
    let expected = [
        RenderInvalidationEntryPoint::DocumentReplaced,
        RenderInvalidationEntryPoint::DomStructureChanged,
        RenderInvalidationEntryPoint::DomAttributesChanged,
        RenderInvalidationEntryPoint::DomTextChanged,
        RenderInvalidationEntryPoint::StylesheetSetChanged,
        RenderInvalidationEntryPoint::ViewportChanged,
        RenderInvalidationEntryPoint::ResourceStateChanged,
        RenderInvalidationEntryPoint::InputStateChanged,
    ];

    for entry_point in expected {
        let count = contracts
            .iter()
            .filter(|contract| contract.entry_point == entry_point)
            .count();
        assert_eq!(
            count, 1,
            "entry point must have exactly one invalidation contract: {entry_point:?}"
        );
    }

    assert_eq!(contracts.len(), expected.len());
}

#[test]
fn paint_invalidation_request_contracts_pin_explicit_repaint_scope_and_reason() {
    let expected = [
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::DocumentReplaced,
            trigger: PaintInvalidationTrigger::DocumentReplaced,
            reason: PaintInvalidationReason::ConservativeUnknownImpact,
            scope: PaintInvalidationScope::Document,
        },
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::DomStructureChanged,
            trigger: PaintInvalidationTrigger::DomStructureChanged,
            reason: PaintInvalidationReason::CascadedFromStyle,
            scope: PaintInvalidationScope::Document,
        },
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::DomAttributesChanged,
            trigger: PaintInvalidationTrigger::DomAttributesChanged,
            reason: PaintInvalidationReason::CascadedFromStyle,
            scope: PaintInvalidationScope::Document,
        },
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::DomTextChanged,
            trigger: PaintInvalidationTrigger::DomTextChanged,
            reason: PaintInvalidationReason::CascadedFromLayout,
            scope: PaintInvalidationScope::Document,
        },
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::StylesheetSetChanged,
            trigger: PaintInvalidationTrigger::StylesheetSetChanged,
            reason: PaintInvalidationReason::CascadedFromStyle,
            scope: PaintInvalidationScope::Document,
        },
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::ViewportChanged,
            trigger: PaintInvalidationTrigger::ViewportChanged,
            reason: PaintInvalidationReason::CascadedFromLayout,
            scope: PaintInvalidationScope::Viewport,
        },
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::ResourceStateChanged,
            trigger: PaintInvalidationTrigger::ResourceStateChanged,
            reason: PaintInvalidationReason::DirectPaintDependency,
            scope: PaintInvalidationScope::Document,
        },
        PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::InputStateChanged,
            trigger: PaintInvalidationTrigger::InputStateChanged,
            reason: PaintInvalidationReason::RuntimeInputState,
            scope: PaintInvalidationScope::Viewport,
        },
    ];

    assert_eq!(paint_invalidation_request_contracts(), expected);
    for request in expected {
        assert_eq!(paint_invalidation_request(request.entry_point), request);
    }
}

#[test]
fn paint_invalidation_contracts_cover_each_paint_rerunning_entry_point_once() {
    let paint_contracts = paint_invalidation_request_contracts();

    for render_request in render_invalidation_request_contracts() {
        let count = paint_contracts
            .iter()
            .filter(|contract| contract.entry_point == render_request.entry_point)
            .count();

        if render_request.paint_invalidation().is_some() {
            assert_eq!(
                count, 1,
                "paint rerun entry point must have exactly one paint invalidation contract: {:?}",
                render_request.entry_point
            );
        } else {
            assert_eq!(
                count, 0,
                "non-paint entry point must not have a paint invalidation contract: {:?}",
                render_request.entry_point
            );
        }
    }

    assert_eq!(
        paint_contracts.len(),
        render_invalidation_request_contracts()
            .iter()
            .filter(|request| request.paint_invalidation().is_some())
            .count()
    );
}

#[test]
fn render_invalidation_request_derives_paint_invalidation_from_paint_work() {
    let input = render_invalidation_request(RenderInvalidationEntryPoint::InputStateChanged);
    assert_eq!(
        input.paint_invalidation(),
        Some(PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::InputStateChanged,
            trigger: PaintInvalidationTrigger::InputStateChanged,
            reason: PaintInvalidationReason::RuntimeInputState,
            scope: PaintInvalidationScope::Viewport,
        })
    );

    let dom = render_invalidation_request(RenderInvalidationEntryPoint::DomStructureChanged);
    assert_eq!(
        dom.paint_invalidation(),
        Some(PaintInvalidationRequest {
            entry_point: RenderInvalidationEntryPoint::DomStructureChanged,
            trigger: PaintInvalidationTrigger::DomStructureChanged,
            reason: PaintInvalidationReason::CascadedFromStyle,
            scope: PaintInvalidationScope::Document,
        })
    );
}

#[test]
fn direct_invalidation_phase_sources_align_with_phase_rebuild_triggers() {
    let phase_contracts = render_phase_contracts();

    for request in render_invalidation_request_contracts() {
        assert!(
            request.work.requests_redraw(),
            "every shipped invalidation entry point should request a frame: {:?}",
            request.entry_point
        );

        for (phase, source) in [
            (RenderingPhase::Style, request.work.style),
            (RenderingPhase::Layout, request.work.layout),
            (RenderingPhase::Paint, request.work.paint),
            (
                RenderingPhase::FrameOrchestration,
                request.work.frame_orchestration,
            ),
        ] {
            if let PhaseRerunSource::Direct(trigger) = source {
                let contract = phase_contracts
                    .iter()
                    .find(|contract| contract.phase == phase)
                    .expect("phase contract should exist");
                assert!(
                    contract.rebuild_triggers.contains(&trigger),
                    "direct invalidation trigger {trigger:?} must be listed on {phase:?}"
                );
            }
        }
    }
}

#[test]
fn pending_render_work_deduplicates_and_preserves_request_order() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));

    assert_eq!(
        pending
            .requests()
            .iter()
            .map(|request| request.entry_point)
            .collect::<Vec<_>>(),
        vec![
            RenderInvalidationEntryPoint::DocumentReplaced,
            RenderInvalidationEntryPoint::ResourceStateChanged,
        ]
    );
}

#[test]
fn pending_render_work_derives_ordered_deduplicated_paint_invalidations() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));

    let paint = pending.paint_invalidations();
    assert_eq!(
        paint
            .requests()
            .iter()
            .map(|request| request.entry_point)
            .collect::<Vec<_>>(),
        vec![
            RenderInvalidationEntryPoint::InputStateChanged,
            RenderInvalidationEntryPoint::ResourceStateChanged,
        ]
    );
    assert_eq!(
        paint
            .requests()
            .iter()
            .map(|request| request.scope)
            .collect::<Vec<_>>(),
        vec![
            PaintInvalidationScope::Viewport,
            PaintInvalidationScope::Document,
        ]
    );
}

#[test]
fn dirty_request_for_viewport_change_marks_layout_and_paint_not_style() {
    let request = dirty_request_for_entry_point(RenderInvalidationEntryPoint::ViewportChanged);

    assert_eq!(
        request.entries,
        vec![
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
        ]
    );

    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ViewportChanged,
    ));
    let dirty = pending.dirty_state();
    assert!(!dirty.is_phase_dirty(DirtyPhase::Style));
    assert!(dirty.is_phase_dirty(DirtyPhase::Layout));
    assert!(dirty.is_phase_dirty(DirtyPhase::Paint));
    assert_eq!(
        dirty.effective_scope(DirtyPhase::Layout),
        DirtyScope::Viewport
    );
    assert_eq!(
        dirty.effective_scope(DirtyPhase::Paint),
        DirtyScope::Viewport
    );
}

#[test]
fn dirty_request_for_input_state_is_paint_only_when_runtime_can_classify_it() {
    let request = dirty_request_for_entry_point(RenderInvalidationEntryPoint::InputStateChanged);

    assert_eq!(
        request.entries,
        vec![DirtyEntry::new(
            DirtyPhase::Paint,
            DirtyReason::RuntimeInputState,
            DirtyScope::Viewport,
        )]
    );

    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    let dirty = pending.dirty_state();
    assert!(!dirty.is_phase_dirty(DirtyPhase::Style));
    assert!(!dirty.is_phase_dirty(DirtyPhase::Layout));
    assert!(dirty.is_phase_dirty(DirtyPhase::Paint));
    assert_eq!(
        dirty.effective_scope(DirtyPhase::Paint),
        DirtyScope::Viewport
    );
}

#[test]
fn dirty_request_for_document_replacement_propagates_style_to_layout_to_paint() {
    let propagation =
        dirty_propagation_for_entry_point(RenderInvalidationEntryPoint::DocumentReplaced);

    assert_eq!(
        propagation.direct,
        vec![DirtyEntry::new(
            DirtyPhase::Style,
            DirtyReason::DocumentReplaced,
            DirtyScope::Document,
        )]
    );
    assert_eq!(
        propagation.propagated,
        vec![
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
    assert!(propagation.state.is_phase_dirty(DirtyPhase::Style));
    assert!(propagation.state.is_phase_dirty(DirtyPhase::Layout));
    assert!(propagation.state.is_phase_dirty(DirtyPhase::Paint));
}

#[test]
fn dirty_state_merging_is_deterministic_deduplicated_and_conservative() {
    let mut dirty = RenderDirtyState::new();
    dirty.push(DirtyEntry::new(
        DirtyPhase::Paint,
        DirtyReason::RuntimeInputState,
        DirtyScope::Viewport,
    ));
    dirty.push(DirtyEntry::new(
        DirtyPhase::Style,
        DirtyReason::StyleInputChanged,
        DirtyScope::Document,
    ));
    dirty.push(DirtyEntry::new(
        DirtyPhase::Paint,
        DirtyReason::RuntimeInputState,
        DirtyScope::Viewport,
    ));
    dirty.push(DirtyEntry::new(
        DirtyPhase::Paint,
        DirtyReason::RuntimeInputState,
        DirtyScope::Document,
    ));

    assert_eq!(
        dirty.entries(),
        &[
            DirtyEntry::new(
                DirtyPhase::Style,
                DirtyReason::StyleInputChanged,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::RuntimeInputState,
                DirtyScope::Document,
            ),
        ]
    );
    assert_eq!(
        dirty.effective_scope(DirtyPhase::Paint),
        DirtyScope::Document
    );
}

#[test]
fn unknown_dirty_state_falls_back_to_visible_document_scope() {
    let dirty = RenderDirtyState::conservative_unknown();

    assert_eq!(
        dirty.entries(),
        &[
            DirtyEntry::new(
                DirtyPhase::Style,
                DirtyReason::ConservativeUnknownImpact,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::ConservativeUnknownImpact,
                DirtyScope::Document,
            ),
            DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::ConservativeUnknownImpact,
                DirtyScope::Document,
            ),
        ]
    );
}

#[test]
fn pending_paint_invalidations_compute_conservative_effective_scope() {
    let mut pending = PendingPaintInvalidations::default();
    assert_eq!(pending.effective_scope(), None);
    assert!(pending.is_empty());

    pending.push(paint_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    assert_eq!(
        pending.effective_scope(),
        Some(PaintInvalidationScope::Viewport)
    );

    pending.push(paint_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));
    assert_eq!(
        pending.effective_scope(),
        Some(PaintInvalidationScope::Document)
    );

    pending.push(paint_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    assert_eq!(pending.requests().len(), 2);
}

#[test]
fn paint_invalidation_debug_snapshot_is_exact_for_mixed_repaint_scopes() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));

    assert_eq!(
        paint_invalidation_debug_snapshot(&pending),
        concat!(
            "version: 1\n",
            "paint-invalidation-snapshot\n",
            "pending-render-work: 2\n",
            "paint-invalidations: 2\n",
            "  request[0]: entry-point=input-state-changed trigger=input-state-changed reason=runtime-input-state scope=viewport\n",
            "  request[1]: entry-point=resource-state-changed trigger=resource-state-changed reason=direct-paint-dependency scope=document\n",
            "effective-scope: document\n",
            "repaint-execution-plan: scope=document\n",
        )
    );
}

#[test]
fn paint_invalidation_debug_snapshot_is_exact_for_empty_pending_work() {
    let pending = PendingRenderWork::default();

    assert_eq!(
        paint_invalidation_debug_snapshot(&pending),
        concat!(
            "version: 1\n",
            "paint-invalidation-snapshot\n",
            "pending-render-work: 0\n",
            "paint-invalidations: 0\n",
            "effective-scope: none\n",
            "repaint-execution-plan: scope=document\n",
        )
    );
}

#[test]
fn repaint_execution_plan_uses_viewport_for_viewport_scoped_invalidations() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));

    assert_eq!(
        RepaintExecutionPlan::from_pending_render_work(&pending),
        RepaintExecutionPlan {
            scope: RepaintExecutionScope::Viewport,
        }
    );

    let mut viewport_pending = PendingRenderWork::default();
    viewport_pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ViewportChanged,
    ));
    assert_eq!(
        RepaintExecutionPlan::from_pending_render_work(&viewport_pending).scope,
        RepaintExecutionScope::Viewport
    );
}

#[test]
fn repaint_execution_plan_uses_document_for_document_scoped_invalidations() {
    for entry_point in [
        RenderInvalidationEntryPoint::DocumentReplaced,
        RenderInvalidationEntryPoint::DomStructureChanged,
        RenderInvalidationEntryPoint::DomAttributesChanged,
        RenderInvalidationEntryPoint::DomTextChanged,
        RenderInvalidationEntryPoint::StylesheetSetChanged,
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ] {
        let mut pending = PendingRenderWork::default();
        pending.push(render_invalidation_request(entry_point));

        assert_eq!(
            RepaintExecutionPlan::from_pending_render_work(&pending).scope,
            RepaintExecutionScope::Document,
            "{entry_point:?} should conservatively repaint the document"
        );
    }
}

#[test]
fn repaint_execution_plan_uses_document_for_mixed_viewport_and_document_invalidations() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));

    assert_eq!(
        RepaintExecutionPlan::from_frame_inputs(&pending, true).scope,
        RepaintExecutionScope::Document
    );
}

#[test]
fn repaint_execution_plan_treats_synthesized_viewport_change_as_viewport_repaint() {
    let pending = PendingRenderWork::default();

    assert_eq!(
        RepaintExecutionPlan::from_frame_inputs(&pending, true).scope,
        RepaintExecutionScope::Viewport
    );
    assert_eq!(
        RepaintExecutionPlan::from_frame_inputs(&pending, false).scope,
        RepaintExecutionScope::Document
    );
}

#[test]
fn document_replacement_returns_explicit_full_pipeline_work_request() {
    let output = parse_document(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should work");
    let mut page = PageState::new();
    page.start_nav("https://example.com/index.html");

    let request = page.replace_dom(Box::new(output.document), RestyleHint::document_replaced());
    assert_eq!(
        request.entry_point,
        RenderInvalidationEntryPoint::DocumentReplaced
    );
    assert_eq!(request.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        request.work.style,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomReplaced)
    );
    assert_eq!(
        request.work.layout,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
    assert_eq!(
        request.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );
    assert_eq!(
        request.work.frame_orchestration,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
}

#[test]
fn dom_text_mutation_returns_explicit_layout_and_paint_work_request() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    page.clear_layout_dirty_for_tests();

    let request = page.mark_dom_changed(RestyleHint::text_mutated());
    assert_eq!(
        request.entry_point,
        RenderInvalidationEntryPoint::DomTextChanged
    );
    assert_eq!(request.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(request.work.style, PhaseRerunSource::None);
    assert_eq!(
        request.work.layout,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );
    assert_eq!(
        request.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );
    assert_eq!(
        request.work.frame_orchestration,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );
}

#[test]
fn stylesheet_reconcile_returns_explicit_style_invalidation_request() {
    let output = parse_document(
        "<!doctype html><html><head><link rel=\"stylesheet\" href=\"https://example.com/site.css\"></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should work");
    let mut page = PageState::new();
    page.start_nav("https://example.com/index.html");
    let _ = page.replace_dom(Box::new(output.document), RestyleHint::document_replaced());
    page.clear_all_dirty_for_tests();

    let outcome = page.reconcile_document_stylesheets();
    let request = outcome
        .render_invalidation
        .expect("stylesheet discovery should invalidate style inputs");
    assert_eq!(
        request.entry_point,
        RenderInvalidationEntryPoint::StylesheetSetChanged
    );
    assert_eq!(request.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        request.work.style,
        PhaseRerunSource::Direct(RenderRebuildTrigger::StylesheetSetChanged)
    );
    assert_eq!(
        request.work.layout,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
    assert_eq!(
        request.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );
    assert_eq!(outcome.fetches.len(), 1);

    let snapshot = page.retained_render_state_debug_snapshot();
    assert_eq!(
        snapshot.dirty_state.entries,
        vec![
            DirtyEntry::new(
                DirtyPhase::Style,
                DirtyReason::StylesheetChanged,
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
}
