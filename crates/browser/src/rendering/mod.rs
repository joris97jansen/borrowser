//! Rendering pipeline contracts and debug surfaces.
//!
//! Milestone V formalizes the rendering pipeline without prematurely adding
//! retained layout or paint caches. This module records the current ownership
//! boundaries, phase I/O, rebuild triggers, and runtime-visible retained vs.
//! rebuilt state so later rendering work can evolve against explicit contracts.
//! It also pins the deferred extension hooks that later milestones are allowed
//! to extend without reinterpreting the current ownership model.

mod contracts;
mod debug;
mod frame;
mod invalidation;
mod lifecycle;
mod page_background;
mod types;

pub use contracts::{
    RenderArtifactLifetime, RenderArtifactOwnershipContract, RenderExtensionHook,
    RenderExtensionHookContract, RenderingPhaseContract, render_artifact_ownership_contracts,
    render_extension_hook_contracts, render_phase_contracts,
};
pub use debug::{
    RenderFrameExecutionTrace, RenderPhaseBoundaryDebugSnapshot, RenderPhaseExecutionKind,
    RenderPhaseExecutionTrace, render_phase_boundary_debug_snapshot,
};
#[cfg(test)]
pub(crate) use frame::build_render_frame_execution_trace;
pub(crate) use frame::{OrchestratedFrameOutcome, execute_prepared_page_frame, prepare_page_frame};
pub use invalidation::{
    PendingRenderWork, PhaseRerunSource, RenderInvalidationRequest, RenderWorkPlan,
    render_invalidation_request, render_invalidation_request_contracts,
};
pub use lifecycle::{RenderArtifactState, RenderPipelineDebugSnapshot, StyleInvalidationState};
pub use types::{
    RenderArtifact, RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase,
    RenderingSubsystem,
};

#[cfg(test)]
mod tests;
