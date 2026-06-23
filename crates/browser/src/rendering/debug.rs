//! Deterministic rendering debug snapshots.

use crate::page::PageState;
use css::ComputedStyleResolutionError;
use gfx::paint::PaintPhaseInput;
use layout::{LayoutPhaseInput, ReplacedElementInfoProvider, TextMeasurer, layout_document};
use std::fmt::Write;

use super::frame::build_render_frame_execution_trace;
use super::invalidation::PendingRenderWork;
use super::types::{
    PaintInvalidationReason, PaintInvalidationScope, PaintInvalidationTrigger,
    RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase, RepaintExecutionPlan,
    RepaintExecutionScope,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderPhaseExecutionKind {
    Requested,
    MaterializedFromRetainedArtifacts,
    RequiredForCurrentFrame,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderPhaseExecutionTrace {
    pub phase: RenderingPhase,
    pub kind: RenderPhaseExecutionKind,
    pub direct_triggers: Vec<RenderRebuildTrigger>,
    pub cascaded_from: Vec<RenderingPhase>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepaintExecutionTrace {
    pub scope: RepaintExecutionScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderFrameExecutionTrace {
    /// Invalidation entry points that actively shaped this frame.
    ///
    /// This includes queued runtime requests consumed at frame start plus the
    /// in-frame `ViewportChanged` trigger when viewport metrics differ from the
    /// previous frame.
    pub triggered_entry_points: Vec<RenderInvalidationEntryPoint>,
    pub style: RenderPhaseExecutionTrace,
    pub layout: RenderPhaseExecutionTrace,
    pub paint: RenderPhaseExecutionTrace,
    pub frame_orchestration: RenderPhaseExecutionTrace,
    pub repaint_execution: RepaintExecutionTrace,
    pub semantic_phase_order: Vec<RenderingPhase>,
}

impl RenderFrameExecutionTrace {
    /// Stable debug snapshot for runtime orchestration decisions.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "render-frame-execution-trace").expect("write snapshot");
        writeln!(
            &mut out,
            "triggered-entry-points: {}",
            self.triggered_entry_points.len()
        )
        .expect("write snapshot");
        for entry_point in &self.triggered_entry_points {
            writeln!(&mut out, "  - {}", entry_point_debug_label(*entry_point))
                .expect("write snapshot");
        }
        append_phase_trace_snapshot(&mut out, "style", &self.style);
        append_phase_trace_snapshot(&mut out, "layout", &self.layout);
        append_phase_trace_snapshot(&mut out, "paint", &self.paint);
        append_phase_trace_snapshot(&mut out, "frame-orchestration", &self.frame_orchestration);
        append_repaint_execution_snapshot(&mut out, &self.repaint_execution);
        writeln!(
            &mut out,
            "semantic-phase-order: {}",
            self.semantic_phase_order
                .iter()
                .map(|phase| rendering_phase_debug_label(*phase))
                .collect::<Vec<_>>()
                .join(" -> ")
        )
        .expect("write snapshot");
        out
    }
}

/// Stable runtime-owned paint invalidation and repaint-planning snapshot.
///
/// This debug surface is derived from pending render work through the AB5/AB6
/// invalidation and repaint-plan APIs. It is not a retained paint scene,
/// compositor layer tree, dirty-region graph, backend command stream, or paint
/// ordering surface.
pub fn paint_invalidation_debug_snapshot(pending: &PendingRenderWork) -> String {
    let paint = pending.paint_invalidations();
    let repaint = RepaintExecutionPlan::from_paint_invalidations(&paint);
    let mut out = String::new();
    writeln!(&mut out, "version: 1").expect("write paint invalidation snapshot");
    writeln!(&mut out, "paint-invalidation-snapshot").expect("write paint invalidation snapshot");
    writeln!(
        &mut out,
        "pending-render-work: {}",
        pending.requests().len()
    )
    .expect("write paint invalidation snapshot");
    writeln!(&mut out, "paint-invalidations: {}", paint.requests().len())
        .expect("write paint invalidation snapshot");
    for (index, request) in paint.requests().iter().enumerate() {
        writeln!(
            &mut out,
            "  request[{index}]: entry-point={} trigger={} reason={} scope={}",
            entry_point_debug_label(request.entry_point),
            paint_invalidation_trigger_debug_label(request.trigger),
            paint_invalidation_reason_debug_label(request.reason),
            paint_invalidation_scope_debug_label(request.scope)
        )
        .expect("write paint invalidation snapshot");
    }
    writeln!(
        &mut out,
        "effective-scope: {}",
        optional_paint_invalidation_scope_debug_label(paint.effective_scope())
    )
    .expect("write paint invalidation snapshot");
    writeln!(
        &mut out,
        "repaint-execution-plan: scope={}",
        repaint_execution_scope_debug_label(repaint.scope)
    )
    .expect("write paint invalidation snapshot");
    out
}

fn append_repaint_execution_snapshot(out: &mut String, trace: &RepaintExecutionTrace) {
    writeln!(
        out,
        "repaint-execution: scope={}",
        repaint_execution_scope_debug_label(trace.scope)
    )
    .expect("write snapshot");
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderPhaseBoundaryDebugSnapshot {
    pub style_output: String,
    pub layout_input: String,
    pub layout_output: String,
    pub paint_input: String,
    pub orchestration: String,
}

impl RenderPhaseBoundaryDebugSnapshot {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "render-phase-boundaries").expect("write snapshot");
        append_nested_snapshot(&mut out, "style-output", &self.style_output);
        append_nested_snapshot(&mut out, "layout-input", &self.layout_input);
        append_nested_snapshot(&mut out, "layout-output", &self.layout_output);
        append_nested_snapshot(&mut out, "paint-input", &self.paint_input);
        append_nested_snapshot(&mut out, "orchestration", &self.orchestration);
        out
    }
}

/// Stable debug surface for one deterministic render pipeline handoff flow.
///
/// This surface intentionally captures phase-boundary objects and the runtime
/// orchestration decision trace without depending on egui paint execution.
pub fn render_phase_boundary_debug_snapshot(
    page: &mut PageState,
    pending_work: PendingRenderWork,
    available_width: f32,
    measurer: &dyn TextMeasurer,
    replaced_info: Option<&dyn ReplacedElementInfoProvider>,
    viewport_changed: bool,
) -> Result<Option<RenderPhaseBoundaryDebugSnapshot>, ComputedStyleResolutionError> {
    let style_dirty_before_frame = page.render_pipeline_debug_snapshot().style_dirty;
    let style_output = match page.build_style_phase_output()? {
        Some(style_output) => style_output,
        None => return Ok(None),
    };
    let style_output_snapshot = style_output.to_debug_snapshot();
    let layout_input = LayoutPhaseInput::from_style_output(
        &style_output,
        available_width,
        measurer,
        replaced_info,
    );
    let layout_input_snapshot = layout_input.to_debug_snapshot();
    let layout_output = layout_document(layout_input);
    let paint_input = PaintPhaseInput::new(&layout_output);
    let orchestration = build_render_frame_execution_trace(
        &pending_work,
        style_dirty_before_frame,
        viewport_changed,
        false,
        false,
    );

    Ok(Some(RenderPhaseBoundaryDebugSnapshot {
        style_output: style_output_snapshot,
        layout_input: layout_input_snapshot,
        layout_output: layout_output.to_debug_snapshot(),
        paint_input: paint_input.to_debug_snapshot(),
        orchestration: orchestration.to_debug_snapshot(),
    }))
}

fn append_phase_trace_snapshot(out: &mut String, label: &str, trace: &RenderPhaseExecutionTrace) {
    writeln!(
        out,
        "{label}: phase={} kind={}",
        rendering_phase_debug_label(trace.phase),
        execution_kind_debug_label(trace.kind)
    )
    .expect("write snapshot");

    writeln!(out, "  direct-triggers: {}", trace.direct_triggers.len()).expect("write snapshot");
    for trigger in &trace.direct_triggers {
        writeln!(out, "    - {}", rebuild_trigger_debug_label(*trigger)).expect("write snapshot");
    }

    writeln!(out, "  cascaded-from: {}", trace.cascaded_from.len()).expect("write snapshot");
    for phase in &trace.cascaded_from {
        writeln!(out, "    - {}", rendering_phase_debug_label(*phase)).expect("write snapshot");
    }
}

fn append_nested_snapshot(out: &mut String, label: &str, snapshot: &str) {
    writeln!(out, "{label}:").expect("write snapshot");
    for line in snapshot.lines() {
        writeln!(out, "  {line}").expect("write snapshot");
    }
}

fn rendering_phase_debug_label(phase: RenderingPhase) -> &'static str {
    match phase {
        RenderingPhase::Style => "style",
        RenderingPhase::Layout => "layout",
        RenderingPhase::Paint => "paint",
        RenderingPhase::FrameOrchestration => "frame-orchestration",
    }
}

fn entry_point_debug_label(entry_point: RenderInvalidationEntryPoint) -> &'static str {
    match entry_point {
        RenderInvalidationEntryPoint::DocumentReplaced => "document-replaced",
        RenderInvalidationEntryPoint::DomStructureChanged => "dom-structure-changed",
        RenderInvalidationEntryPoint::DomAttributesChanged => "dom-attributes-changed",
        RenderInvalidationEntryPoint::DomTextChanged => "dom-text-changed",
        RenderInvalidationEntryPoint::StylesheetSetChanged => "stylesheet-set-changed",
        RenderInvalidationEntryPoint::ViewportChanged => "viewport-changed",
        RenderInvalidationEntryPoint::ResourceStateChanged => "resource-state-changed",
        RenderInvalidationEntryPoint::InputStateChanged => "input-state-changed",
    }
}

fn paint_invalidation_trigger_debug_label(trigger: PaintInvalidationTrigger) -> &'static str {
    match trigger {
        PaintInvalidationTrigger::DocumentReplaced => "document-replaced",
        PaintInvalidationTrigger::DomStructureChanged => "dom-structure-changed",
        PaintInvalidationTrigger::DomAttributesChanged => "dom-attributes-changed",
        PaintInvalidationTrigger::DomTextChanged => "dom-text-changed",
        PaintInvalidationTrigger::StylesheetSetChanged => "stylesheet-set-changed",
        PaintInvalidationTrigger::ViewportChanged => "viewport-changed",
        PaintInvalidationTrigger::ResourceStateChanged => "resource-state-changed",
        PaintInvalidationTrigger::InputStateChanged => "input-state-changed",
    }
}

fn paint_invalidation_reason_debug_label(reason: PaintInvalidationReason) -> &'static str {
    match reason {
        PaintInvalidationReason::ConservativeUnknownImpact => "conservative-unknown-impact",
        PaintInvalidationReason::CascadedFromStyle => "cascaded-from-style",
        PaintInvalidationReason::CascadedFromLayout => "cascaded-from-layout",
        PaintInvalidationReason::DirectPaintDependency => "direct-paint-dependency",
        PaintInvalidationReason::RuntimeInputState => "runtime-input-state",
    }
}

fn optional_paint_invalidation_scope_debug_label(
    scope: Option<PaintInvalidationScope>,
) -> &'static str {
    match scope {
        Some(scope) => paint_invalidation_scope_debug_label(scope),
        None => "none",
    }
}

fn paint_invalidation_scope_debug_label(scope: PaintInvalidationScope) -> &'static str {
    match scope {
        PaintInvalidationScope::Viewport => "viewport",
        PaintInvalidationScope::Document => "document",
    }
}

fn execution_kind_debug_label(kind: RenderPhaseExecutionKind) -> &'static str {
    match kind {
        RenderPhaseExecutionKind::Requested => "requested",
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts => {
            "materialized-from-retained-artifacts"
        }
        RenderPhaseExecutionKind::RequiredForCurrentFrame => "required-for-current-frame",
    }
}

fn rebuild_trigger_debug_label(trigger: RenderRebuildTrigger) -> &'static str {
    match trigger {
        RenderRebuildTrigger::DomReplaced => "dom-replaced",
        RenderRebuildTrigger::DomStructureChanged => "dom-structure-changed",
        RenderRebuildTrigger::DomAttributesChanged => "dom-attributes-changed",
        RenderRebuildTrigger::DomTextChanged => "dom-text-changed",
        RenderRebuildTrigger::StylesheetSetChanged => "stylesheet-set-changed",
        RenderRebuildTrigger::StyleOutputsChanged => "style-outputs-changed",
        RenderRebuildTrigger::ViewportChanged => "viewport-changed",
        RenderRebuildTrigger::ResourceStateChanged => "resource-state-changed",
        RenderRebuildTrigger::InputStateChanged => "input-state-changed",
        RenderRebuildTrigger::LayoutOutputsChanged => "layout-outputs-changed",
    }
}

fn repaint_execution_scope_debug_label(scope: RepaintExecutionScope) -> &'static str {
    match scope {
        RepaintExecutionScope::Viewport => "viewport",
        RepaintExecutionScope::Document => "document",
    }
}
