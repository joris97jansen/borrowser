//! Frame preparation and execution orchestration.

use crate::form_controls::FormControlIndex;
use crate::input_state::DocumentInputState;
use crate::page::PageState;
use css::{ComputedStyleResolutionError, StylePhaseOutput};
use egui::Ui;
use gfx::input::PageAction;
use gfx::paint::{ImageProvider, PaintArtifact};
use gfx::viewport::{
    ViewportCtx, ViewportPaintArtifactAction, ViewportRepaintPolicy, ViewportRepaintScope,
    ViewportResourceInputs, ViewportRetainedLayout, ViewportRetainedPaint, execute_viewport_frame,
};
use layout::{RetainedLayoutArtifact, RetainedLayoutFrameResult, RetainedLayoutKeySeed};

use super::debug::{
    RenderFrameExecutionTrace, RenderPhaseExecutionKind, RenderPhaseExecutionTrace,
};
use super::invalidation::{
    PendingRenderWork, PhaseRerunSource, RenderInvalidationRequest, render_invalidation_request,
};
use super::page_background::find_page_background_color;
use super::types::{PaintInvalidationScope, RepaintExecutionPlan, RepaintExecutionScope};
use super::types::{RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase};
use super::work_plan::{RenderWorkPlan, RepaintExecution};
use super::{RetainedPaintArtifactKeySeed, RetainedPaintFrameAction, RetainedPaintFrameResult};

pub(crate) struct OrchestratedFrameOutcome {
    pub(crate) action: Option<PageAction>,
    pub(crate) followup_render_request: Option<RenderInvalidationRequest>,
    pub(crate) trace: RenderFrameExecutionTrace,
    pub(crate) retained_layout_result: Option<RetainedLayoutFrameResult>,
    pub(crate) retained_paint_result: Option<RetainedPaintFrameResult>,
}

pub(crate) struct PreparedPageFrame<'a> {
    pub(crate) style_output: StylePhaseOutput<'a>,
    pub(crate) page_background: Option<(u8, u8, u8, u8)>,
    pub(crate) work_plan: RenderWorkPlan,
    retained_layout_key_seed: RetainedLayoutKeySeed,
    retained_layout_artifact: Option<RetainedLayoutArtifact>,
    retained_paint_key_seed: RetainedPaintArtifactKeySeed,
    retained_paint_artifact: Option<PaintArtifact>,
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
    let style_dirty_before_frame = page.style_dirty_for_rendering();
    let base_url = page.base_url.clone();
    let form_controls = page.form_controls.clone();

    let prepared_style = match page.prepare_style_phase_for_frame(&pending_work)? {
        Some(prepared_style) => prepared_style,
        None => return Ok(None),
    };
    let page_background = find_page_background_color(&prepared_style.style_output);

    Ok(Some(PreparedPageFrame {
        style_output: prepared_style.style_output,
        page_background,
        work_plan: prepared_style.work_plan,
        retained_layout_key_seed: prepared_style.retained_layout_key_seed,
        retained_layout_artifact: prepared_style.retained_layout_artifact,
        retained_paint_key_seed: prepared_style.retained_paint_key_seed,
        retained_paint_artifact: prepared_style.retained_paint_artifact,
        pending_work,
        style_dirty_before_frame,
        base_url,
        form_controls,
    }))
}

struct BrowserReplacedElementInfo<'a, R> {
    base_url: Option<&'a str>,
    resources: &'a R,
}

impl<R: ImageProvider> layout::ReplacedElementInfoProvider for BrowserReplacedElementInfo<'_, R> {
    fn resolve_image_source(&self, source: &str) -> Option<String> {
        crate::resources::resolve_image_source(self.base_url, source)
    }

    fn intrinsic_for_img(
        &self,
        image: &layout::ImagePresentation,
    ) -> Option<layout::replaced::intrinsic::IntrinsicSize> {
        let url = image.resolved_source()?;
        let (width, height) = self.resources.image_intrinsic_size_px(url)?;
        Some(layout::replaced::intrinsic::IntrinsicSize::from_w_h(
            Some(width as f32),
            Some(height as f32),
        ))
    }
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
        work_plan,
        retained_layout_key_seed,
        retained_layout_artifact,
        retained_paint_key_seed,
        retained_paint_artifact,
        pending_work,
        style_dirty_before_frame,
        base_url,
        form_controls,
    } = prepared;
    let replaced_info = BrowserReplacedElementInfo {
        base_url: base_url.as_deref(),
        resources,
    };
    let repaint_policy = viewport_repaint_policy(&pending_work);
    let viewport_result = execute_viewport_frame(
        ViewportCtx::new(
            ui,
            &style_output,
            base_url.as_deref(),
            ViewportResourceInputs::new(resources, &replaced_info),
            &mut input_state.input_values,
            &form_controls,
            &mut input_state.interaction,
        )
        .with_repaint_policy(repaint_policy)
        .with_retained_layout(ViewportRetainedLayout {
            key_seed: retained_layout_key_seed,
            retained: retained_layout_artifact.as_ref(),
            reuse_allowed: matches!(
                work_plan.relayout_execution,
                super::work_plan::RelayoutExecution::ReuseRetained
            ),
            conservative_dirty_fallback: matches!(
                work_plan.relayout_execution,
                super::work_plan::RelayoutExecution::ConservativeDocumentFallback { .. }
            ),
        })
        .with_retained_paint(ViewportRetainedPaint {
            retained: retained_paint_artifact.as_ref(),
            reuse_allowed: matches!(work_plan.repaint_execution, RepaintExecution::ReuseRetained),
        }),
    );
    let retained_paint_result = match (
        viewport_result.retained_paint_result,
        viewport_result.retained_layout_result.as_ref(),
    ) {
        (Some(paint_result), Some(layout_result)) => {
            let key = retained_paint_key_seed.for_layout_key(layout_result.key);
            let action = match paint_result.action {
                ViewportPaintArtifactAction::Reused => {
                    debug_assert!(
                        matches!(
                            layout_result.action,
                            layout::RetainedLayoutFrameAction::Reused
                        ),
                        "retained paint reuse requires actual retained layout reuse"
                    );
                    RetainedPaintFrameAction::Reused
                }
                ViewportPaintArtifactAction::Recomputed => {
                    retained_paint_action_for_recompute(&work_plan)
                }
            };
            Some(RetainedPaintFrameResult {
                key,
                action,
                artifact: paint_result.artifact,
            })
        }
        _ => None,
    };

    let trace = build_render_frame_execution_trace(
        &pending_work,
        style_dirty_before_frame,
        viewport_result.viewport_changed,
        viewport_result
            .retained_layout_result
            .as_ref()
            .is_some_and(|result| {
                matches!(result.action, layout::RetainedLayoutFrameAction::Reused)
            }),
        retained_paint_result
            .as_ref()
            .is_some_and(|result| matches!(result.action, RetainedPaintFrameAction::Reused)),
    );
    debug_assert_eq!(
        trace.repaint_execution.scope,
        repaint_execution_scope_from_viewport(viewport_result.repaint_scope)
    );
    let followup_render_request = viewport_result
        .requested_followup_render
        .then(|| render_invalidation_request(RenderInvalidationEntryPoint::InputStateChanged));

    OrchestratedFrameOutcome {
        action: viewport_result.action,
        followup_render_request,
        trace,
        retained_layout_result: viewport_result.retained_layout_result,
        retained_paint_result,
    }
}

fn retained_paint_action_for_recompute(work_plan: &RenderWorkPlan) -> RetainedPaintFrameAction {
    match work_plan.repaint_execution {
        RepaintExecution::ConservativeDocumentFallback { .. } => {
            RetainedPaintFrameAction::ConservativeDocumentFallback
        }
        RepaintExecution::ConservativeViewportFallback { .. } | RepaintExecution::Viewport => {
            RetainedPaintFrameAction::ConservativeViewportFallback
        }
        RepaintExecution::None
        | RepaintExecution::ReuseRetained
        | RepaintExecution::FullDocument => RetainedPaintFrameAction::Recomputed,
    }
}

pub(crate) fn build_render_frame_execution_trace(
    pending_work: &PendingRenderWork,
    style_dirty_before_frame: bool,
    viewport_changed: bool,
    layout_reused_from_retained_artifacts: bool,
    paint_reused_from_retained_artifacts: bool,
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
        style.record(request.requested_work.style);
        layout.record(request.requested_work.layout);
        paint.record(request.requested_work.paint);
        frame_orchestration.record(request.requested_work.frame_orchestration);
    }

    if viewport_changed {
        let request = render_invalidation_request(RenderInvalidationEntryPoint::ViewportChanged);
        push_unique(&mut triggered_entry_points, request.entry_point);
        style.record(request.requested_work.style);
        layout.record(request.requested_work.layout);
        paint.record(request.requested_work.paint);
        frame_orchestration.record(request.requested_work.frame_orchestration);
    }

    let style_fallback = if style_dirty_before_frame {
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    } else {
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    };
    let layout_fallback = if layout_reused_from_retained_artifacts {
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    } else {
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    };
    let paint_fallback = if paint_reused_from_retained_artifacts {
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    } else {
        RenderPhaseExecutionKind::RequiredForCurrentFrame
    };

    RenderFrameExecutionTrace {
        triggered_entry_points,
        style: phase_trace(RenderingPhase::Style, style, style_fallback),
        layout: phase_trace(RenderingPhase::Layout, layout, layout_fallback),
        paint: phase_trace(RenderingPhase::Paint, paint, paint_fallback),
        frame_orchestration: phase_trace(
            RenderingPhase::FrameOrchestration,
            frame_orchestration,
            RenderPhaseExecutionKind::RequiredForCurrentFrame,
        ),
        repaint_execution: super::debug::RepaintExecutionTrace {
            scope: RepaintExecutionPlan::from_frame_inputs(pending_work, viewport_changed).scope,
        },
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

fn viewport_repaint_policy(pending_work: &PendingRenderWork) -> ViewportRepaintPolicy {
    let paint = pending_work.paint_invalidations();
    let pending_scope = paint
        .effective_scope()
        .map(viewport_repaint_scope_from_paint);
    ViewportRepaintPolicy::from_pending_scope(pending_scope)
}

fn viewport_repaint_scope_from_paint(scope: PaintInvalidationScope) -> ViewportRepaintScope {
    match scope {
        PaintInvalidationScope::Viewport => ViewportRepaintScope::Viewport,
        PaintInvalidationScope::Document => ViewportRepaintScope::Document,
    }
}

fn repaint_execution_scope_from_viewport(scope: ViewportRepaintScope) -> RepaintExecutionScope {
    match scope {
        ViewportRepaintScope::Viewport => RepaintExecutionScope::Viewport,
        ViewportRepaintScope::Document => RepaintExecutionScope::Document,
    }
}
