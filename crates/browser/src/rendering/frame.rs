//! Frame preparation and execution orchestration.

use crate::form_controls::FormControlIndex;
use crate::input_state::DocumentInputState;
use crate::page::PageState;
use css::{ComputedStyleResolutionError, StylePhaseOutput};
use egui::Ui;
use gfx::input::PageAction;
use gfx::paint::ImageProvider;
use gfx::viewport::{ViewportCtx, execute_viewport_frame};

use super::debug::{
    RenderFrameExecutionTrace, RenderPhaseExecutionKind, RenderPhaseExecutionTrace,
};
use super::invalidation::{
    PendingRenderWork, PhaseRerunSource, RenderInvalidationRequest, render_invalidation_request,
};
use super::page_background::find_page_background_color;
use super::types::{RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase};

pub(crate) struct OrchestratedFrameOutcome {
    pub(crate) action: Option<PageAction>,
    pub(crate) followup_render_request: Option<RenderInvalidationRequest>,
    pub(crate) trace: RenderFrameExecutionTrace,
}

pub(crate) struct PreparedPageFrame<'a> {
    pub(crate) style_output: StylePhaseOutput<'a>,
    pub(crate) page_background: Option<(u8, u8, u8, u8)>,
    pending_work: PendingRenderWork,
    style_dirty_before_frame: bool,
    base_url: Option<String>,
    form_controls: FormControlIndex,
}

#[derive(Default)]
struct PhaseReasonAccumulator {
    direct_triggers: Vec<RenderRebuildTrigger>,
    cascaded_from: Vec<RenderingPhase>,
}

impl PhaseReasonAccumulator {
    fn record(&mut self, source: PhaseRerunSource) {
        match source {
            PhaseRerunSource::None => {}
            PhaseRerunSource::Direct(trigger) => push_unique(&mut self.direct_triggers, trigger),
            PhaseRerunSource::CascadedFrom(phase) => push_unique(&mut self.cascaded_from, phase),
        }
    }

    fn has_requested_work(&self) -> bool {
        !self.direct_triggers.is_empty() || !self.cascaded_from.is_empty()
    }
}

pub(crate) fn prepare_page_frame(
    page: &mut PageState,
    pending_work: PendingRenderWork,
) -> Result<Option<PreparedPageFrame<'_>>, ComputedStyleResolutionError> {
    let style_snapshot = page.render_pipeline_debug_snapshot();
    let base_url = page.base_url.clone();
    let form_controls = page.form_controls.clone();

    let style_output = match page.build_style_phase_output()? {
        Some(style_output) => style_output,
        None => return Ok(None),
    };
    let page_background = find_page_background_color(&style_output);

    Ok(Some(PreparedPageFrame {
        style_output,
        page_background,
        pending_work,
        style_dirty_before_frame: style_snapshot.style_dirty,
        base_url,
        form_controls,
    }))
}

pub(crate) fn execute_prepared_page_frame<R: ImageProvider>(
    ui: &mut Ui,
    prepared: PreparedPageFrame<'_>,
    input_state: &mut DocumentInputState,
    resources: &R,
) -> OrchestratedFrameOutcome {
    let PreparedPageFrame {
        style_output,
        page_background: _page_background,
        pending_work,
        style_dirty_before_frame,
        base_url,
        form_controls,
    } = prepared;
    let viewport_result = execute_viewport_frame(ViewportCtx::new(
        ui,
        &style_output,
        base_url.as_deref(),
        resources,
        &mut input_state.input_values,
        &form_controls,
        &mut input_state.interaction,
    ));

    let trace = build_render_frame_execution_trace(
        &pending_work,
        style_dirty_before_frame,
        viewport_result.viewport_changed,
    );
    let followup_render_request = viewport_result
        .requested_followup_render
        .then(|| render_invalidation_request(RenderInvalidationEntryPoint::InputStateChanged));

    OrchestratedFrameOutcome {
        action: viewport_result.action,
        followup_render_request,
        trace,
    }
}

pub(crate) fn build_render_frame_execution_trace(
    pending_work: &PendingRenderWork,
    style_dirty_before_frame: bool,
    viewport_changed: bool,
) -> RenderFrameExecutionTrace {
    let mut triggered_entry_points = pending_work
        .requests()
        .iter()
        .map(|request| request.entry_point)
        .collect::<Vec<_>>();
    let mut style = PhaseReasonAccumulator::default();
    let mut layout = PhaseReasonAccumulator::default();
    let mut paint = PhaseReasonAccumulator::default();
    let mut frame_orchestration = PhaseReasonAccumulator::default();

    for request in pending_work.requests() {
        style.record(request.work.style);
        layout.record(request.work.layout);
        paint.record(request.work.paint);
        frame_orchestration.record(request.work.frame_orchestration);
    }

    if viewport_changed {
        let request = render_invalidation_request(RenderInvalidationEntryPoint::ViewportChanged);
        push_unique(&mut triggered_entry_points, request.entry_point);
        style.record(request.work.style);
        layout.record(request.work.layout);
        paint.record(request.work.paint);
        frame_orchestration.record(request.work.frame_orchestration);
    }

    let style_fallback = if style_dirty_before_frame {
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    } else {
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    };

    RenderFrameExecutionTrace {
        triggered_entry_points,
        style: phase_trace(RenderingPhase::Style, style, style_fallback),
        layout: phase_trace(
            RenderingPhase::Layout,
            layout,
            RenderPhaseExecutionKind::RequiredForCurrentFrame,
        ),
        paint: phase_trace(
            RenderingPhase::Paint,
            paint,
            RenderPhaseExecutionKind::RequiredForCurrentFrame,
        ),
        frame_orchestration: phase_trace(
            RenderingPhase::FrameOrchestration,
            frame_orchestration,
            RenderPhaseExecutionKind::RequiredForCurrentFrame,
        ),
        semantic_phase_order: vec![
            RenderingPhase::Style,
            RenderingPhase::Layout,
            RenderingPhase::Paint,
        ],
    }
}

fn phase_trace(
    phase: RenderingPhase,
    reasons: PhaseReasonAccumulator,
    fallback_kind: RenderPhaseExecutionKind,
) -> RenderPhaseExecutionTrace {
    let kind = if reasons.has_requested_work() {
        RenderPhaseExecutionKind::Requested
    } else {
        fallback_kind
    };
    RenderPhaseExecutionTrace {
        phase,
        kind,
        direct_triggers: reasons.direct_triggers,
        cascaded_from: reasons.cascaded_from,
    }
}

fn push_unique<T: Copy + PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}
