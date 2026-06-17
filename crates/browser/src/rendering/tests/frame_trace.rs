use crate::rendering::*;

#[test]
fn frame_execution_trace_distinguishes_requested_work_from_frame_prerequisites() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));

    let trace = build_render_frame_execution_trace(&pending, false, false);
    assert_eq!(
        trace.triggered_entry_points,
        vec![RenderInvalidationEntryPoint::InputStateChanged]
    );
    assert_eq!(
        trace.style.kind,
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    );
    assert!(trace.style.direct_triggers.is_empty());
    assert_eq!(
        trace.layout.kind,
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    );
    assert!(trace.layout.direct_triggers.is_empty());
    assert_eq!(trace.paint.kind, RenderPhaseExecutionKind::Requested);
    assert_eq!(
        trace.paint.direct_triggers,
        vec![RenderRebuildTrigger::InputStateChanged]
    );
    assert_eq!(
        trace.repaint_execution.scope,
        RepaintExecutionScope::Viewport
    );
    assert_eq!(
        trace.frame_orchestration.kind,
        RenderPhaseExecutionKind::Requested
    );
    assert_eq!(
        trace.semantic_phase_order,
        vec![
            RenderingPhase::Style,
            RenderingPhase::Layout,
            RenderingPhase::Paint,
        ]
    );
}

#[test]
fn frame_execution_trace_adds_viewport_change_as_direct_runtime_trigger() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));

    let trace = build_render_frame_execution_trace(&pending, true, true);
    assert_eq!(
        trace.triggered_entry_points,
        vec![
            RenderInvalidationEntryPoint::DocumentReplaced,
            RenderInvalidationEntryPoint::ViewportChanged,
        ]
    );
    assert_eq!(trace.style.kind, RenderPhaseExecutionKind::Requested);
    assert_eq!(trace.layout.kind, RenderPhaseExecutionKind::Requested);
    assert_eq!(
        trace.layout.direct_triggers,
        vec![RenderRebuildTrigger::ViewportChanged]
    );
    assert_eq!(trace.layout.cascaded_from, vec![RenderingPhase::Style]);
    assert_eq!(trace.paint.kind, RenderPhaseExecutionKind::Requested);
    assert_eq!(trace.paint.cascaded_from, vec![RenderingPhase::Layout]);
    assert_eq!(
        trace.frame_orchestration.direct_triggers,
        vec![RenderRebuildTrigger::ViewportChanged]
    );
    assert_eq!(
        trace.repaint_execution.scope,
        RepaintExecutionScope::Document
    );
}

#[test]
fn frame_execution_trace_records_viewport_repaint_for_synthesized_viewport_change() {
    let pending = PendingRenderWork::default();

    let trace = build_render_frame_execution_trace(&pending, false, true);
    assert_eq!(
        trace.triggered_entry_points,
        vec![RenderInvalidationEntryPoint::ViewportChanged]
    );
    assert_eq!(
        trace.repaint_execution.scope,
        RepaintExecutionScope::Viewport
    );
    assert!(
        trace
            .to_debug_snapshot()
            .contains("repaint-execution: scope=viewport")
    );
}

#[test]
fn frame_execution_trace_records_document_repaint_for_mixed_invalidations() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));

    let trace = build_render_frame_execution_trace(&pending, false, false);
    assert_eq!(
        trace.repaint_execution.scope,
        RepaintExecutionScope::Document
    );
    assert!(
        trace
            .to_debug_snapshot()
            .contains("repaint-execution: scope=document")
    );
}
