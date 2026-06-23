//! Rendering pipeline contracts and debug surfaces.
//!
//! Milestone V formalized the rendering pipeline; Milestone AC extends it with
//! retained runtime state, retained style artifacts, and retained layout
//! artifacts. This module records the current ownership boundaries, phase I/O,
//! rebuild triggers, and runtime-visible retained vs. rebuilt state so later
//! rendering work can evolve against explicit contracts.
//! It also pins the deferred extension hooks that later milestones are allowed
//! to extend without reinterpreting the current ownership model.

mod contracts;
mod debug;
mod frame;
mod identity;
mod invalidation;
mod lifecycle;
mod page_background;
mod types;
mod work_plan;

pub use contracts::{
    RenderArtifactLifetime, RenderArtifactOwnershipContract, RenderExtensionHook,
    RenderExtensionHookContract, RenderingPhaseContract, render_artifact_ownership_contracts,
    render_extension_hook_contracts, render_phase_contracts,
};
pub use debug::{
    RenderFrameExecutionTrace, RenderPhaseBoundaryDebugSnapshot, RenderPhaseExecutionKind,
    RenderPhaseExecutionTrace, RepaintExecutionTrace, paint_invalidation_debug_snapshot,
    render_phase_boundary_debug_snapshot,
};
#[cfg(test)]
pub(crate) use frame::build_render_frame_execution_trace;
pub(crate) use frame::{OrchestratedFrameOutcome, execute_prepared_page_frame, prepare_page_frame};
pub use identity::{
    RetainedRenderAnchor, RetainedRenderArtifactKind, RetainedRenderId, RetainedRenderIdentity,
    RetainedRenderIdentityDomain,
};
pub(crate) use identity::{
    RetainedRenderIdentityMap, retained_render_anchor_debug_label,
    retained_render_artifact_kind_debug_label,
};
pub use invalidation::{
    PendingPaintInvalidations, PendingRenderWork, PhaseRerunSource, RenderInvalidationRequest,
    RenderInvalidationWorkPlan, dirty_propagation_for_entry_point, dirty_request_for_entry_point,
    paint_invalidation_request, paint_invalidation_request_contracts, render_invalidation_request,
    render_invalidation_request_contracts,
};
pub use lifecycle::{
    DirtyStateDebugSnapshot, FrameLocalIdentityState, RenderArtifactState, RenderEpoch,
    RenderPipelineDebugSnapshot, RetainedLayoutArtifactAction, RetainedLayoutArtifactDebugSnapshot,
    RetainedLayoutArtifactStats, RetainedPaintArtifactAction, RetainedPaintArtifactDebugSnapshot,
    RetainedPaintArtifactKey, RetainedPaintArtifactKeySeed, RetainedPaintArtifactStats,
    RetainedPaintFrameAction, RetainedPaintFrameResult, RetainedRenderStateDebugSnapshot,
    RetainedStyleArtifactAction, RetainedStyleArtifactDebugSnapshot, RetainedStyleArtifactKey,
    RetainedStyleArtifactStats, StyleInvalidationState,
};
pub use types::{
    DirtyEntry, DirtyPhase, DirtyPropagationResult, DirtyReason, DirtyScope, DirtyScopeDebugLabel,
    PaintInvalidationReason, PaintInvalidationRequest, PaintInvalidationScope,
    PaintInvalidationTrigger, RenderArtifact, RenderDirtyRequest, RenderDirtyState,
    RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase, RenderingSubsystem,
    RepaintExecutionPlan, RepaintExecutionScope,
};
pub use work_plan::{
    PlannedRenderWork, RelayoutExecution, RenderWorkDecision, RenderWorkFallbackReason,
    RenderWorkPlan, RenderWorkPlanInput, RenderWorkPlanReason, RepaintExecution,
    RetainedLayoutArtifactState, RetainedPaintArtifactState, RetainedStyleArtifactState,
};

#[cfg(test)]
mod tests;
