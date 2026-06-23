//! Deterministic browser/runtime render work planning.
//!
//! The planner derives intended style, layout, and paint work from retained
//! runtime state and dirty inputs before the engine executes those phases. It
//! does not perform CSS, layout, or paint semantics.

use std::fmt::Write;

use super::invalidation::PendingRenderWork;
use super::types::{
    DirtyPhase, DirtyReason, DirtyScope, DirtyScopeDebugLabel, RenderArtifact, RenderDirtyState,
    RenderInvalidationEntryPoint,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedStyleArtifactState {
    Absent,
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedLayoutArtifactState {
    Absent,
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderWorkDecision {
    None,
    ReuseRetainedStyle,
    ReuseRetainedLayout,
    Restyle,
    Relayout,
    Repaint,
    ConservativeFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderWorkPlanReason {
    NoDocument,
    CleanDirtyState,
    Dirty(DirtyReason),
    RetainedStyleArtifactAbsent,
    RetainedStyleArtifactFresh,
    RetainedStyleArtifactStale,
    RetainedLayoutArtifactAbsent,
    RetainedLayoutArtifactFresh,
    RetainedLayoutArtifactStale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderWorkFallbackReason {
    ConservativeUnknownImpact { scope: DirtyScope },
    TargetedRelayoutNotExecutable { scope: DirtyScope },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedRenderWork {
    pub decision: RenderWorkDecision,
    pub scope: DirtyScope,
    pub reasons: Vec<RenderWorkPlanReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayoutExecution {
    None,
    ReuseRetained,
    FullDocument,
    ConservativeDocumentFallback {
        requested_scope: DirtyScope,
        reason: RenderWorkFallbackReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderWorkPlan {
    pub entry_points: Vec<RenderInvalidationEntryPoint>,
    pub dirty_state: RenderDirtyState,
    pub restyle: PlannedRenderWork,
    pub relayout: PlannedRenderWork,
    pub relayout_execution: RelayoutExecution,
    pub repaint: PlannedRenderWork,
    pub conservative_fallback: Option<RenderWorkFallbackReason>,
}

pub struct RenderWorkPlanInput<'a> {
    pub has_dom: bool,
    pub retained_style_artifacts: RetainedStyleArtifactState,
    pub retained_layout_artifacts: RetainedLayoutArtifactState,
    pub retained_dirty_state: &'a RenderDirtyState,
    pub pending_work: &'a PendingRenderWork,
}

impl RenderWorkPlan {
    pub fn derive(input: RenderWorkPlanInput<'_>) -> Self {
        let dirty_state = canonical_dirty_state(input.retained_dirty_state, input.pending_work);
        let conservative_fallback = conservative_fallback(&dirty_state);
        let entry_points = input
            .pending_work
            .requests()
            .iter()
            .map(|request| request.entry_point)
            .collect::<Vec<_>>();

        Self {
            entry_points,
            restyle: plan_restyle(
                input.has_dom,
                input.retained_style_artifacts,
                &dirty_state,
                conservative_fallback,
            ),
            relayout: plan_relayout(
                input.has_dom,
                input.retained_layout_artifacts,
                &dirty_state,
                conservative_fallback,
            ),
            relayout_execution: plan_relayout_execution(
                input.has_dom,
                input.retained_layout_artifacts,
                &dirty_state,
                conservative_fallback,
            ),
            repaint: plan_phase(
                input.has_dom,
                DirtyPhase::Paint,
                RenderWorkDecision::Repaint,
                &dirty_state,
                conservative_fallback,
            ),
            dirty_state,
            conservative_fallback,
        }
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write render work plan snapshot");
        writeln!(&mut out, "render-work-plan").expect("write render work plan snapshot");
        writeln!(&mut out, "entry-points: {}", self.entry_points.len())
            .expect("write render work plan snapshot");
        for entry_point in &self.entry_points {
            writeln!(&mut out, "  - {}", entry_point_debug_label(*entry_point))
                .expect("write render work plan snapshot");
        }
        append_dirty_state_snapshot(&mut out, &self.dirty_state);
        append_planned_work_snapshot(&mut out, "restyle", &self.restyle);
        append_planned_work_snapshot(&mut out, "relayout", &self.relayout);
        append_relayout_execution_snapshot(&mut out, self.relayout_execution);
        append_planned_work_snapshot(&mut out, "repaint", &self.repaint);
        append_fallback_snapshot(&mut out, self.conservative_fallback);
        out
    }
}

fn canonical_dirty_state(
    retained_dirty_state: &RenderDirtyState,
    pending_work: &PendingRenderWork,
) -> RenderDirtyState {
    let mut state = retained_dirty_state.clone();
    state.merge(&pending_work.dirty_state());
    state
}

fn conservative_fallback(dirty_state: &RenderDirtyState) -> Option<RenderWorkFallbackReason> {
    let scope = dirty_state
        .entries()
        .iter()
        .filter(|entry| entry.reason == DirtyReason::ConservativeUnknownImpact)
        .map(|entry| entry.scope)
        .fold(DirtyScope::None, DirtyScope::conservative_merge);

    (scope != DirtyScope::None)
        .then_some(RenderWorkFallbackReason::ConservativeUnknownImpact { scope })
}

fn plan_restyle(
    has_dom: bool,
    style_artifacts: RetainedStyleArtifactState,
    dirty_state: &RenderDirtyState,
    conservative_fallback: Option<RenderWorkFallbackReason>,
) -> PlannedRenderWork {
    if !has_dom {
        return planned_no_document();
    }

    let scope = dirty_state.effective_scope(DirtyPhase::Style);
    let mut reasons = dirty_reasons(dirty_state, DirtyPhase::Style);
    let style_is_dirty = scope != DirtyScope::None;

    match (style_is_dirty, style_artifacts, conservative_fallback) {
        (true, RetainedStyleArtifactState::Absent, Some(_))
        | (true, RetainedStyleArtifactState::Fresh, Some(_))
        | (true, RetainedStyleArtifactState::Stale, Some(_)) => PlannedRenderWork {
            decision: RenderWorkDecision::ConservativeFallback,
            scope,
            reasons,
        },
        (true, RetainedStyleArtifactState::Absent, None) => {
            reasons.push(RenderWorkPlanReason::RetainedStyleArtifactAbsent);
            PlannedRenderWork {
                decision: RenderWorkDecision::Restyle,
                scope,
                reasons,
            }
        }
        (true, RetainedStyleArtifactState::Fresh, None) => PlannedRenderWork {
            decision: RenderWorkDecision::Restyle,
            scope,
            reasons,
        },
        (true, RetainedStyleArtifactState::Stale, None) => {
            reasons.push(RenderWorkPlanReason::RetainedStyleArtifactStale);
            PlannedRenderWork {
                decision: RenderWorkDecision::Restyle,
                scope,
                reasons,
            }
        }
        (false, RetainedStyleArtifactState::Absent, _) => PlannedRenderWork {
            decision: RenderWorkDecision::Restyle,
            scope: DirtyScope::Document,
            reasons: vec![RenderWorkPlanReason::RetainedStyleArtifactAbsent],
        },
        (false, RetainedStyleArtifactState::Fresh, _) => PlannedRenderWork {
            decision: RenderWorkDecision::ReuseRetainedStyle,
            scope: DirtyScope::None,
            reasons: vec![
                RenderWorkPlanReason::CleanDirtyState,
                RenderWorkPlanReason::RetainedStyleArtifactFresh,
            ],
        },
        (false, RetainedStyleArtifactState::Stale, _) => PlannedRenderWork {
            decision: RenderWorkDecision::Restyle,
            scope: DirtyScope::Document,
            reasons: vec![RenderWorkPlanReason::RetainedStyleArtifactStale],
        },
    }
}

fn plan_relayout(
    has_dom: bool,
    layout_artifacts: RetainedLayoutArtifactState,
    dirty_state: &RenderDirtyState,
    conservative_fallback: Option<RenderWorkFallbackReason>,
) -> PlannedRenderWork {
    if !has_dom {
        return planned_no_document();
    }

    let scope = dirty_state.effective_scope(DirtyPhase::Layout);
    if scope == DirtyScope::None {
        return match layout_artifacts {
            RetainedLayoutArtifactState::Fresh => PlannedRenderWork {
                decision: RenderWorkDecision::ReuseRetainedLayout,
                scope,
                reasons: vec![
                    RenderWorkPlanReason::CleanDirtyState,
                    RenderWorkPlanReason::RetainedLayoutArtifactFresh,
                ],
            },
            RetainedLayoutArtifactState::Absent => PlannedRenderWork {
                decision: RenderWorkDecision::Relayout,
                scope: DirtyScope::Document,
                reasons: vec![RenderWorkPlanReason::RetainedLayoutArtifactAbsent],
            },
            RetainedLayoutArtifactState::Stale => PlannedRenderWork {
                decision: RenderWorkDecision::Relayout,
                scope: DirtyScope::Document,
                reasons: vec![RenderWorkPlanReason::RetainedLayoutArtifactStale],
            },
        };
    }

    PlannedRenderWork {
        decision: if conservative_fallback.is_some() || scope != DirtyScope::Document {
            RenderWorkDecision::ConservativeFallback
        } else {
            RenderWorkDecision::Relayout
        },
        scope,
        reasons: dirty_reasons(dirty_state, DirtyPhase::Layout),
    }
}

fn plan_relayout_execution(
    has_dom: bool,
    layout_artifacts: RetainedLayoutArtifactState,
    dirty_state: &RenderDirtyState,
    conservative_fallback: Option<RenderWorkFallbackReason>,
) -> RelayoutExecution {
    if !has_dom {
        return RelayoutExecution::None;
    }

    let scope = dirty_state.effective_scope(DirtyPhase::Layout);
    if scope == DirtyScope::None {
        return match layout_artifacts {
            RetainedLayoutArtifactState::Fresh => RelayoutExecution::ReuseRetained,
            RetainedLayoutArtifactState::Absent | RetainedLayoutArtifactState::Stale => {
                RelayoutExecution::FullDocument
            }
        };
    }

    if let Some(reason) = conservative_fallback {
        return RelayoutExecution::ConservativeDocumentFallback {
            requested_scope: scope,
            reason,
        };
    }

    if scope != DirtyScope::Document {
        return RelayoutExecution::ConservativeDocumentFallback {
            requested_scope: scope,
            reason: RenderWorkFallbackReason::TargetedRelayoutNotExecutable { scope },
        };
    }

    RelayoutExecution::FullDocument
}

fn plan_phase(
    has_dom: bool,
    phase: DirtyPhase,
    dirty_decision: RenderWorkDecision,
    dirty_state: &RenderDirtyState,
    conservative_fallback: Option<RenderWorkFallbackReason>,
) -> PlannedRenderWork {
    if !has_dom {
        return planned_no_document();
    }

    let scope = dirty_state.effective_scope(phase);
    if scope == DirtyScope::None {
        return PlannedRenderWork {
            decision: RenderWorkDecision::None,
            scope,
            reasons: vec![RenderWorkPlanReason::CleanDirtyState],
        };
    }

    PlannedRenderWork {
        decision: if conservative_fallback.is_some() {
            RenderWorkDecision::ConservativeFallback
        } else {
            dirty_decision
        },
        scope,
        reasons: dirty_reasons(dirty_state, phase),
    }
}

fn planned_no_document() -> PlannedRenderWork {
    PlannedRenderWork {
        decision: RenderWorkDecision::None,
        scope: DirtyScope::None,
        reasons: vec![RenderWorkPlanReason::NoDocument],
    }
}

fn dirty_reasons(dirty_state: &RenderDirtyState, phase: DirtyPhase) -> Vec<RenderWorkPlanReason> {
    dirty_state
        .entries()
        .iter()
        .filter(|entry| entry.phase == phase)
        .map(|entry| RenderWorkPlanReason::Dirty(entry.reason))
        .collect()
}

fn append_dirty_state_snapshot(out: &mut String, dirty_state: &RenderDirtyState) {
    writeln!(out, "canonical-dirty-state:").expect("write render work plan dirty-state snapshot");
    writeln!(out, "  entries: {}", dirty_state.entries().len())
        .expect("write render work plan dirty-state snapshot");
    for (index, entry) in dirty_state.entries().iter().enumerate() {
        writeln!(
            out,
            "    entry[{index}]: phase={} reason={} scope={}",
            entry.phase.debug_label(),
            entry.reason.debug_label(),
            dirty_scope_debug_label(entry.scope.debug_label())
        )
        .expect("write render work plan dirty-state snapshot");
    }
}

fn append_planned_work_snapshot(out: &mut String, label: &str, work: &PlannedRenderWork) {
    writeln!(
        out,
        "{label}: decision={} scope={}",
        work_decision_debug_label(work.decision),
        dirty_scope_debug_label(work.scope.debug_label())
    )
    .expect("write render work plan phase snapshot");
    writeln!(out, "  reasons: {}", work.reasons.len())
        .expect("write render work plan phase snapshot");
    for reason in &work.reasons {
        writeln!(out, "    - {}", work_reason_debug_label(*reason))
            .expect("write render work plan phase snapshot");
    }
}

fn append_relayout_execution_snapshot(out: &mut String, execution: RelayoutExecution) {
    match execution {
        RelayoutExecution::None => writeln!(out, "relayout-execution: strategy=none"),
        RelayoutExecution::ReuseRetained => {
            writeln!(out, "relayout-execution: strategy=reuse-retained")
        }
        RelayoutExecution::FullDocument => {
            writeln!(out, "relayout-execution: strategy=full-document")
        }
        RelayoutExecution::ConservativeDocumentFallback {
            requested_scope,
            reason,
        } => writeln!(
            out,
            "relayout-execution: strategy=conservative-document-fallback requested-scope={} reason={}",
            dirty_scope_debug_label(requested_scope.debug_label()),
            fallback_reason_debug_label(reason)
        ),
    }
    .expect("write render work plan relayout execution snapshot");
}

fn append_fallback_snapshot(out: &mut String, fallback: Option<RenderWorkFallbackReason>) {
    match fallback {
        Some(RenderWorkFallbackReason::ConservativeUnknownImpact { scope }) => writeln!(
            out,
            "conservative-fallback: reason=conservative-unknown-impact scope={}",
            dirty_scope_debug_label(scope.debug_label())
        ),
        Some(RenderWorkFallbackReason::TargetedRelayoutNotExecutable { scope }) => writeln!(
            out,
            "conservative-fallback: reason=targeted-relayout-not-executable scope={}",
            dirty_scope_debug_label(scope.debug_label())
        ),
        None => writeln!(out, "conservative-fallback: none"),
    }
    .expect("write render work plan fallback snapshot");
}

fn work_decision_debug_label(decision: RenderWorkDecision) -> &'static str {
    match decision {
        RenderWorkDecision::None => "none",
        RenderWorkDecision::ReuseRetainedStyle => "reuse-retained-style",
        RenderWorkDecision::ReuseRetainedLayout => "reuse-retained-layout",
        RenderWorkDecision::Restyle => "restyle",
        RenderWorkDecision::Relayout => "relayout",
        RenderWorkDecision::Repaint => "repaint",
        RenderWorkDecision::ConservativeFallback => "conservative-fallback",
    }
}

fn work_reason_debug_label(reason: RenderWorkPlanReason) -> String {
    match reason {
        RenderWorkPlanReason::NoDocument => "no-document".to_string(),
        RenderWorkPlanReason::CleanDirtyState => "clean-dirty-state".to_string(),
        RenderWorkPlanReason::Dirty(reason) => format!("dirty({})", reason.debug_label()),
        RenderWorkPlanReason::RetainedStyleArtifactAbsent => {
            retained_style_artifact_reason("absent")
        }
        RenderWorkPlanReason::RetainedStyleArtifactFresh => retained_style_artifact_reason("fresh"),
        RenderWorkPlanReason::RetainedStyleArtifactStale => retained_style_artifact_reason("stale"),
        RenderWorkPlanReason::RetainedLayoutArtifactAbsent => {
            retained_layout_artifact_reason("absent")
        }
        RenderWorkPlanReason::RetainedLayoutArtifactFresh => {
            retained_layout_artifact_reason("fresh")
        }
        RenderWorkPlanReason::RetainedLayoutArtifactStale => {
            retained_layout_artifact_reason("stale")
        }
    }
}

fn retained_style_artifact_reason(state: &str) -> String {
    format!(
        "retained-style-artifact({})={state}",
        render_artifact_debug_label(RenderArtifact::ComputedDocumentStyle)
    )
}

fn retained_layout_artifact_reason(state: &str) -> String {
    format!(
        "retained-layout-artifact({})={state}",
        render_artifact_debug_label(RenderArtifact::LayoutTree)
    )
}

fn fallback_reason_debug_label(reason: RenderWorkFallbackReason) -> String {
    match reason {
        RenderWorkFallbackReason::ConservativeUnknownImpact { scope } => format!(
            "conservative-unknown-impact({})",
            dirty_scope_debug_label(scope.debug_label())
        ),
        RenderWorkFallbackReason::TargetedRelayoutNotExecutable { scope } => format!(
            "targeted-relayout-not-executable({})",
            dirty_scope_debug_label(scope.debug_label())
        ),
    }
}

fn dirty_scope_debug_label(label: DirtyScopeDebugLabel) -> String {
    match label {
        DirtyScopeDebugLabel::Static(label) => label.to_string(),
        DirtyScopeDebugLabel::RetainedId { prefix, id } => {
            format!("{prefix}(retained-render-id={})", id.value())
        }
    }
}

fn render_artifact_debug_label(artifact: RenderArtifact) -> &'static str {
    match artifact {
        RenderArtifact::Dom => "dom",
        RenderArtifact::StylesheetSet => "stylesheet-set",
        RenderArtifact::ResolvedDocumentStyle => "resolved-document-style",
        RenderArtifact::ComputedDocumentStyle => "computed-document-style",
        RenderArtifact::StyledTree => "styled-tree",
        RenderArtifact::ViewportMetrics => "viewport-metrics",
        RenderArtifact::TextMeasurement => "text-measurement",
        RenderArtifact::ReplacedElementMetadata => "replaced-element-metadata",
        RenderArtifact::LayoutTree => "layout-tree",
        RenderArtifact::ResourceState => "resource-state",
        RenderArtifact::InputState => "input-state",
        RenderArtifact::PaintCommands => "paint-commands",
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
