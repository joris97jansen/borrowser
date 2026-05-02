//! Rendering pipeline contracts and debug surfaces.
//!
//! Milestone V formalizes the rendering pipeline without prematurely adding
//! retained layout or paint caches. This module records the current ownership
//! boundaries, phase I/O, rebuild triggers, and runtime-visible retained vs.
//! rebuilt state so later rendering work can evolve against explicit contracts.
//! It also pins the deferred extension hooks that later milestones are allowed
//! to extend without reinterpreting the current ownership model.

use crate::form_controls::FormControlIndex;
use crate::input_state::DocumentInputState;
use crate::page::PageState;
use css::{ComputedStyleResolutionError, StylePhaseOutput, StyledNode};
use egui::Ui;
use gfx::input::PageAction;
use gfx::paint::{ImageProvider, PaintPhaseInput};
use gfx::viewport::{ViewportCtx, execute_viewport_frame};
use html::Node;
use layout::{LayoutPhaseInput, ReplacedElementInfoProvider, TextMeasurer, layout_document};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingPhase {
    Style,
    Layout,
    Paint,
    FrameOrchestration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingSubsystem {
    BrowserRuntime,
    BrowserView,
    CssEngine,
    GfxViewport,
    LayoutEngine,
    PaintEngine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderArtifact {
    Dom,
    StylesheetSet,
    ResolvedDocumentStyle,
    ComputedDocumentStyle,
    StyledTree,
    ViewportMetrics,
    TextMeasurement,
    ReplacedElementMetadata,
    LayoutTree,
    ResourceState,
    InputState,
    PaintCommands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderRebuildTrigger {
    DomReplaced,
    DomStructureChanged,
    DomAttributesChanged,
    DomTextChanged,
    StylesheetSetChanged,
    StyleOutputsChanged,
    ViewportChanged,
    ResourceStateChanged,
    InputStateChanged,
    LayoutOutputsChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderingPhaseContract {
    pub phase: RenderingPhase,
    pub coordinator: RenderingSubsystem,
    pub engine_owner: RenderingSubsystem,
    pub consumes: &'static [RenderArtifact],
    pub produces: &'static [RenderArtifact],
    pub retained_outputs: &'static [RenderArtifact],
    pub rebuilt_outputs: &'static [RenderArtifact],
    pub rebuild_triggers: &'static [RenderRebuildTrigger],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderInvalidationEntryPoint {
    DocumentReplaced,
    DomStructureChanged,
    DomAttributesChanged,
    DomTextChanged,
    StylesheetSetChanged,
    ViewportChanged,
    ResourceStateChanged,
    InputStateChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseRerunSource {
    None,
    Direct(RenderRebuildTrigger),
    CascadedFrom(RenderingPhase),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderWorkPlan {
    pub style: PhaseRerunSource,
    pub layout: PhaseRerunSource,
    pub paint: PhaseRerunSource,
    pub frame_orchestration: PhaseRerunSource,
}

impl RenderWorkPlan {
    pub const fn requests_redraw(self) -> bool {
        !matches!(self.frame_orchestration, PhaseRerunSource::None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderInvalidationRequest {
    pub entry_point: RenderInvalidationEntryPoint,
    pub requested_by: RenderingSubsystem,
    pub work: RenderWorkPlan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderArtifactLifetime {
    RetainedAcrossUpdates,
    BorrowBackedRebuiltOnDemand,
    FrameLocalRebuiltPerFrame,
    ImmediateFrameOutput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderArtifactOwnershipContract {
    pub artifact: RenderArtifact,
    pub semantic_owner: RenderingSubsystem,
    /// Runtime subsystem that retains this artifact across updates.
    ///
    /// `None` means the artifact is intentionally rebuilt or emitted on demand
    /// and is not retained as a long-lived rendering object.
    pub retention_owner: Option<RenderingSubsystem>,
    pub lifetime: RenderArtifactLifetime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderExtensionHook {
    BoxTreeFormalization,
    ConstraintSizingAndIntrinsicLayout,
    PaintPrimitiveAndDisplayListExpansion,
    IncrementalInvalidationAndDependencyTracking,
    RetainedLayoutState,
    RetainedPaintSceneState,
    RuntimeFrameSchedulingIncrementality,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderExtensionHookContract {
    /// The deferred rendering milestone hook being reserved explicitly.
    pub hook: RenderExtensionHook,
    /// Subsystem that owns integrating this hook into the current pipeline.
    pub integration_owner: RenderingSubsystem,
    /// Rendering phases whose contracts the hook is allowed to extend.
    pub phases: &'static [RenderingPhase],
    /// Pipeline artifacts the hook is allowed to reinterpret or replace.
    pub artifacts: &'static [RenderArtifact],
    /// Runtime invalidation entry points the hook must preserve or refine.
    pub invalidation_entry_points: &'static [RenderInvalidationEntryPoint],
}

const FRAME_ORCHESTRATION_CONSUMES: &[RenderArtifact] = &[
    RenderArtifact::StyledTree,
    RenderArtifact::ViewportMetrics,
    RenderArtifact::ResourceState,
    RenderArtifact::InputState,
];
const FRAME_ORCHESTRATION_PRODUCES: &[RenderArtifact] =
    &[RenderArtifact::LayoutTree, RenderArtifact::PaintCommands];
const FRAME_ORCHESTRATION_REBUILDS: &[RenderArtifact] =
    &[RenderArtifact::LayoutTree, RenderArtifact::PaintCommands];
const FRAME_ORCHESTRATION_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::StyleOutputsChanged,
    RenderRebuildTrigger::DomTextChanged,
    RenderRebuildTrigger::ViewportChanged,
    RenderRebuildTrigger::ResourceStateChanged,
    RenderRebuildTrigger::InputStateChanged,
];

const STYLE_CONSUMES: &[RenderArtifact] = &[RenderArtifact::Dom, RenderArtifact::StylesheetSet];
const STYLE_PRODUCES: &[RenderArtifact] = &[
    RenderArtifact::ResolvedDocumentStyle,
    RenderArtifact::ComputedDocumentStyle,
    RenderArtifact::StyledTree,
];
const STYLE_RETAINED: &[RenderArtifact] = &[
    RenderArtifact::ResolvedDocumentStyle,
    RenderArtifact::ComputedDocumentStyle,
];
const STYLE_REBUILDS: &[RenderArtifact] = &[RenderArtifact::StyledTree];
const STYLE_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::DomReplaced,
    RenderRebuildTrigger::DomStructureChanged,
    RenderRebuildTrigger::DomAttributesChanged,
    RenderRebuildTrigger::StylesheetSetChanged,
];

const LAYOUT_CONSUMES: &[RenderArtifact] = &[
    RenderArtifact::StyledTree,
    RenderArtifact::ViewportMetrics,
    RenderArtifact::TextMeasurement,
    RenderArtifact::ReplacedElementMetadata,
];
const LAYOUT_PRODUCES: &[RenderArtifact] = &[RenderArtifact::LayoutTree];
const LAYOUT_REBUILDS: &[RenderArtifact] = &[RenderArtifact::LayoutTree];
const LAYOUT_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::StyleOutputsChanged,
    RenderRebuildTrigger::DomTextChanged,
    RenderRebuildTrigger::ViewportChanged,
    RenderRebuildTrigger::ResourceStateChanged,
];

const PAINT_CONSUMES: &[RenderArtifact] = &[
    RenderArtifact::LayoutTree,
    RenderArtifact::ResourceState,
    RenderArtifact::InputState,
];
const PAINT_PRODUCES: &[RenderArtifact] = &[RenderArtifact::PaintCommands];
const PAINT_REBUILDS: &[RenderArtifact] = &[RenderArtifact::PaintCommands];
const PAINT_TRIGGERS: &[RenderRebuildTrigger] = &[
    RenderRebuildTrigger::LayoutOutputsChanged,
    RenderRebuildTrigger::ResourceStateChanged,
    RenderRebuildTrigger::InputStateChanged,
];

const ALL_RENDERING_PHASES: &[RenderingPhase] = &[
    RenderingPhase::Style,
    RenderingPhase::Layout,
    RenderingPhase::Paint,
    RenderingPhase::FrameOrchestration,
];

const STYLE_LAYOUT_PHASES: &[RenderingPhase] = &[RenderingPhase::Style, RenderingPhase::Layout];
const LAYOUT_PAINT_PHASES: &[RenderingPhase] = &[RenderingPhase::Layout, RenderingPhase::Paint];
const PAINT_ORCHESTRATION_PHASES: &[RenderingPhase] =
    &[RenderingPhase::Paint, RenderingPhase::FrameOrchestration];
const LAYOUT_PAINT_ORCHESTRATION_PHASES: &[RenderingPhase] = &[
    RenderingPhase::Layout,
    RenderingPhase::Paint,
    RenderingPhase::FrameOrchestration,
];

const BOX_TREE_ARTIFACTS: &[RenderArtifact] =
    &[RenderArtifact::StyledTree, RenderArtifact::LayoutTree];
const CONSTRAINT_SIZING_ARTIFACTS: &[RenderArtifact] = &[
    RenderArtifact::StyledTree,
    RenderArtifact::ViewportMetrics,
    RenderArtifact::TextMeasurement,
    RenderArtifact::ReplacedElementMetadata,
    RenderArtifact::LayoutTree,
];
const PAINT_EXPANSION_ARTIFACTS: &[RenderArtifact] = &[
    RenderArtifact::LayoutTree,
    RenderArtifact::ResourceState,
    RenderArtifact::InputState,
    RenderArtifact::PaintCommands,
];
const INCREMENTAL_INVALIDATION_ARTIFACTS: &[RenderArtifact] = &[
    RenderArtifact::ResolvedDocumentStyle,
    RenderArtifact::ComputedDocumentStyle,
    RenderArtifact::StyledTree,
    RenderArtifact::LayoutTree,
    RenderArtifact::PaintCommands,
];
const RETAINED_LAYOUT_ARTIFACTS: &[RenderArtifact] = &[
    RenderArtifact::ViewportMetrics,
    RenderArtifact::TextMeasurement,
    RenderArtifact::ReplacedElementMetadata,
    RenderArtifact::LayoutTree,
];
const RETAINED_PAINT_ARTIFACTS: &[RenderArtifact] = &[
    RenderArtifact::LayoutTree,
    RenderArtifact::ResourceState,
    RenderArtifact::InputState,
    RenderArtifact::PaintCommands,
];
const RUNTIME_INCREMENTALITY_ARTIFACTS: &[RenderArtifact] = &[
    RenderArtifact::ViewportMetrics,
    RenderArtifact::ResourceState,
    RenderArtifact::InputState,
    RenderArtifact::LayoutTree,
    RenderArtifact::PaintCommands,
];

const ALL_INVALIDATION_ENTRY_POINTS: &[RenderInvalidationEntryPoint] = &[
    RenderInvalidationEntryPoint::DocumentReplaced,
    RenderInvalidationEntryPoint::DomStructureChanged,
    RenderInvalidationEntryPoint::DomAttributesChanged,
    RenderInvalidationEntryPoint::DomTextChanged,
    RenderInvalidationEntryPoint::StylesheetSetChanged,
    RenderInvalidationEntryPoint::ViewportChanged,
    RenderInvalidationEntryPoint::ResourceStateChanged,
    RenderInvalidationEntryPoint::InputStateChanged,
];
const STYLE_LAYOUT_INVALIDATION_ENTRY_POINTS: &[RenderInvalidationEntryPoint] = &[
    RenderInvalidationEntryPoint::DocumentReplaced,
    RenderInvalidationEntryPoint::DomStructureChanged,
    RenderInvalidationEntryPoint::DomAttributesChanged,
    RenderInvalidationEntryPoint::DomTextChanged,
    RenderInvalidationEntryPoint::StylesheetSetChanged,
    RenderInvalidationEntryPoint::ViewportChanged,
    RenderInvalidationEntryPoint::ResourceStateChanged,
];
const LAYOUT_PAINT_INVALIDATION_ENTRY_POINTS: &[RenderInvalidationEntryPoint] = &[
    RenderInvalidationEntryPoint::DocumentReplaced,
    RenderInvalidationEntryPoint::DomStructureChanged,
    RenderInvalidationEntryPoint::DomAttributesChanged,
    RenderInvalidationEntryPoint::DomTextChanged,
    RenderInvalidationEntryPoint::StylesheetSetChanged,
    RenderInvalidationEntryPoint::ViewportChanged,
    RenderInvalidationEntryPoint::ResourceStateChanged,
    RenderInvalidationEntryPoint::InputStateChanged,
];

static RENDER_PHASE_CONTRACTS: [RenderingPhaseContract; 4] = [
    RenderingPhaseContract {
        phase: RenderingPhase::Style,
        coordinator: RenderingSubsystem::BrowserRuntime,
        engine_owner: RenderingSubsystem::CssEngine,
        consumes: STYLE_CONSUMES,
        produces: STYLE_PRODUCES,
        retained_outputs: STYLE_RETAINED,
        rebuilt_outputs: STYLE_REBUILDS,
        rebuild_triggers: STYLE_TRIGGERS,
    },
    RenderingPhaseContract {
        phase: RenderingPhase::Layout,
        coordinator: RenderingSubsystem::GfxViewport,
        engine_owner: RenderingSubsystem::LayoutEngine,
        consumes: LAYOUT_CONSUMES,
        produces: LAYOUT_PRODUCES,
        retained_outputs: &[],
        rebuilt_outputs: LAYOUT_REBUILDS,
        rebuild_triggers: LAYOUT_TRIGGERS,
    },
    RenderingPhaseContract {
        phase: RenderingPhase::Paint,
        coordinator: RenderingSubsystem::GfxViewport,
        engine_owner: RenderingSubsystem::PaintEngine,
        consumes: PAINT_CONSUMES,
        produces: PAINT_PRODUCES,
        retained_outputs: &[],
        rebuilt_outputs: PAINT_REBUILDS,
        rebuild_triggers: PAINT_TRIGGERS,
    },
    RenderingPhaseContract {
        phase: RenderingPhase::FrameOrchestration,
        coordinator: RenderingSubsystem::BrowserView,
        engine_owner: RenderingSubsystem::GfxViewport,
        consumes: FRAME_ORCHESTRATION_CONSUMES,
        produces: FRAME_ORCHESTRATION_PRODUCES,
        retained_outputs: &[],
        rebuilt_outputs: FRAME_ORCHESTRATION_REBUILDS,
        rebuild_triggers: FRAME_ORCHESTRATION_TRIGGERS,
    },
];

/// Stable rendering phase contract table.
///
/// `FrameOrchestration` is intentionally a runtime coordination phase, not a
/// semantic rendering engine phase like style, layout, or paint.
pub fn render_phase_contracts() -> &'static [RenderingPhaseContract] {
    &RENDER_PHASE_CONTRACTS
}

static RENDER_INVALIDATION_REQUEST_CONTRACTS: [RenderInvalidationRequest; 8] = [
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DocumentReplaced,
        requested_by: RenderingSubsystem::BrowserRuntime,
        work: RenderWorkPlan {
            style: PhaseRerunSource::Direct(RenderRebuildTrigger::DomReplaced),
            layout: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomStructureChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        work: RenderWorkPlan {
            style: PhaseRerunSource::Direct(RenderRebuildTrigger::DomStructureChanged),
            layout: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomAttributesChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        work: RenderWorkPlan {
            style: PhaseRerunSource::Direct(RenderRebuildTrigger::DomAttributesChanged),
            layout: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomTextChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        work: RenderWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::StylesheetSetChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        work: RenderWorkPlan {
            style: PhaseRerunSource::Direct(RenderRebuildTrigger::StylesheetSetChanged),
            layout: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::CascadedFrom(RenderingPhase::Style),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::ViewportChanged,
        requested_by: RenderingSubsystem::BrowserView,
        work: RenderWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::Direct(RenderRebuildTrigger::ViewportChanged),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::Direct(RenderRebuildTrigger::ViewportChanged),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::ResourceStateChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        work: RenderWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::Direct(RenderRebuildTrigger::ResourceStateChanged),
            paint: PhaseRerunSource::Direct(RenderRebuildTrigger::ResourceStateChanged),
            frame_orchestration: PhaseRerunSource::Direct(
                RenderRebuildTrigger::ResourceStateChanged,
            ),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::InputStateChanged,
        requested_by: RenderingSubsystem::BrowserView,
        work: RenderWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::None,
            paint: PhaseRerunSource::Direct(RenderRebuildTrigger::InputStateChanged),
            frame_orchestration: PhaseRerunSource::Direct(RenderRebuildTrigger::InputStateChanged),
        },
    },
];

/// Stable invalidation-entry-point contract table.
///
/// Each entry records who may request pipeline work for a runtime trigger and
/// which phases rerun directly versus as a downstream consequence.
pub fn render_invalidation_request_contracts() -> &'static [RenderInvalidationRequest] {
    &RENDER_INVALIDATION_REQUEST_CONTRACTS
}

pub fn render_invalidation_request(
    entry_point: RenderInvalidationEntryPoint,
) -> RenderInvalidationRequest {
    *render_invalidation_request_contracts()
        .iter()
        .find(|contract| contract.entry_point == entry_point)
        .expect("render invalidation contract must exist for every entry point")
}

static RENDER_ARTIFACT_OWNERSHIP_CONTRACTS: [RenderArtifactOwnershipContract; 12] = [
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::Dom,
        semantic_owner: RenderingSubsystem::BrowserRuntime,
        retention_owner: Some(RenderingSubsystem::BrowserRuntime),
        lifetime: RenderArtifactLifetime::RetainedAcrossUpdates,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::StylesheetSet,
        semantic_owner: RenderingSubsystem::BrowserRuntime,
        retention_owner: Some(RenderingSubsystem::BrowserRuntime),
        lifetime: RenderArtifactLifetime::RetainedAcrossUpdates,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::ResolvedDocumentStyle,
        semantic_owner: RenderingSubsystem::CssEngine,
        retention_owner: Some(RenderingSubsystem::BrowserRuntime),
        lifetime: RenderArtifactLifetime::RetainedAcrossUpdates,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::ComputedDocumentStyle,
        semantic_owner: RenderingSubsystem::CssEngine,
        retention_owner: Some(RenderingSubsystem::BrowserRuntime),
        lifetime: RenderArtifactLifetime::RetainedAcrossUpdates,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::StyledTree,
        semantic_owner: RenderingSubsystem::CssEngine,
        retention_owner: None,
        lifetime: RenderArtifactLifetime::BorrowBackedRebuiltOnDemand,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::ViewportMetrics,
        semantic_owner: RenderingSubsystem::BrowserView,
        retention_owner: None,
        lifetime: RenderArtifactLifetime::FrameLocalRebuiltPerFrame,
    },
    RenderArtifactOwnershipContract {
        // Layout owns the measurement contract it consumes, even though the
        // concrete measurer may be provided by the viewport/backend runtime.
        artifact: RenderArtifact::TextMeasurement,
        semantic_owner: RenderingSubsystem::LayoutEngine,
        retention_owner: None,
        lifetime: RenderArtifactLifetime::FrameLocalRebuiltPerFrame,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::ReplacedElementMetadata,
        semantic_owner: RenderingSubsystem::LayoutEngine,
        retention_owner: None,
        lifetime: RenderArtifactLifetime::FrameLocalRebuiltPerFrame,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::LayoutTree,
        semantic_owner: RenderingSubsystem::LayoutEngine,
        retention_owner: None,
        lifetime: RenderArtifactLifetime::FrameLocalRebuiltPerFrame,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::ResourceState,
        semantic_owner: RenderingSubsystem::BrowserRuntime,
        retention_owner: Some(RenderingSubsystem::BrowserRuntime),
        lifetime: RenderArtifactLifetime::RetainedAcrossUpdates,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::InputState,
        semantic_owner: RenderingSubsystem::BrowserRuntime,
        retention_owner: Some(RenderingSubsystem::BrowserRuntime),
        lifetime: RenderArtifactLifetime::RetainedAcrossUpdates,
    },
    RenderArtifactOwnershipContract {
        artifact: RenderArtifact::PaintCommands,
        semantic_owner: RenderingSubsystem::PaintEngine,
        retention_owner: None,
        lifetime: RenderArtifactLifetime::ImmediateFrameOutput,
    },
];

/// Stable artifact lifetime and retention-owner table.
///
/// This complements `render_phase_contracts()` by recording where each
/// pipeline artifact lives across updates and which artifacts are intentionally
/// rebuilt rather than retained.
pub fn render_artifact_ownership_contracts() -> &'static [RenderArtifactOwnershipContract] {
    &RENDER_ARTIFACT_OWNERSHIP_CONTRACTS
}

static RENDER_EXTENSION_HOOK_CONTRACTS: [RenderExtensionHookContract; 7] = [
    RenderExtensionHookContract {
        hook: RenderExtensionHook::BoxTreeFormalization,
        integration_owner: RenderingSubsystem::LayoutEngine,
        phases: LAYOUT_PAINT_PHASES,
        artifacts: BOX_TREE_ARTIFACTS,
        invalidation_entry_points: STYLE_LAYOUT_INVALIDATION_ENTRY_POINTS,
    },
    RenderExtensionHookContract {
        hook: RenderExtensionHook::ConstraintSizingAndIntrinsicLayout,
        integration_owner: RenderingSubsystem::LayoutEngine,
        phases: STYLE_LAYOUT_PHASES,
        artifacts: CONSTRAINT_SIZING_ARTIFACTS,
        invalidation_entry_points: STYLE_LAYOUT_INVALIDATION_ENTRY_POINTS,
    },
    RenderExtensionHookContract {
        hook: RenderExtensionHook::PaintPrimitiveAndDisplayListExpansion,
        integration_owner: RenderingSubsystem::PaintEngine,
        phases: PAINT_ORCHESTRATION_PHASES,
        artifacts: PAINT_EXPANSION_ARTIFACTS,
        invalidation_entry_points: LAYOUT_PAINT_INVALIDATION_ENTRY_POINTS,
    },
    RenderExtensionHookContract {
        hook: RenderExtensionHook::IncrementalInvalidationAndDependencyTracking,
        integration_owner: RenderingSubsystem::BrowserRuntime,
        phases: ALL_RENDERING_PHASES,
        artifacts: INCREMENTAL_INVALIDATION_ARTIFACTS,
        invalidation_entry_points: ALL_INVALIDATION_ENTRY_POINTS,
    },
    RenderExtensionHookContract {
        hook: RenderExtensionHook::RetainedLayoutState,
        integration_owner: RenderingSubsystem::BrowserRuntime,
        phases: LAYOUT_PAINT_ORCHESTRATION_PHASES,
        artifacts: RETAINED_LAYOUT_ARTIFACTS,
        invalidation_entry_points: STYLE_LAYOUT_INVALIDATION_ENTRY_POINTS,
    },
    RenderExtensionHookContract {
        hook: RenderExtensionHook::RetainedPaintSceneState,
        integration_owner: RenderingSubsystem::BrowserRuntime,
        phases: PAINT_ORCHESTRATION_PHASES,
        artifacts: RETAINED_PAINT_ARTIFACTS,
        invalidation_entry_points: LAYOUT_PAINT_INVALIDATION_ENTRY_POINTS,
    },
    RenderExtensionHookContract {
        hook: RenderExtensionHook::RuntimeFrameSchedulingIncrementality,
        integration_owner: RenderingSubsystem::BrowserRuntime,
        phases: LAYOUT_PAINT_ORCHESTRATION_PHASES,
        artifacts: RUNTIME_INCREMENTALITY_ARTIFACTS,
        invalidation_entry_points: ALL_INVALIDATION_ENTRY_POINTS,
    },
];

/// Stable table of deferred rendering extension hooks.
///
/// This is the normative V7 contract surface for future rendering milestones:
/// later work may extend these named hooks, but must not bypass the current
/// ownership, handoff, retained-state, invalidation, or debug contracts.
pub fn render_extension_hook_contracts() -> &'static [RenderExtensionHookContract] {
    &RENDER_EXTENSION_HOOK_CONTRACTS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderArtifactState {
    Absent,
    RetainedFresh,
    RetainedStale,
    BorrowBackedRebuiltOnDemand,
    /// Rebuilt during frame execution rather than retained in page state.
    FrameLocalRebuiltPerFrame,
    /// Emitted during paint for the current frame rather than retained.
    ImmediateFrameOutput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleInvalidationState {
    None,
    Full,
    AttributeSuffix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderPipelineDebugSnapshot {
    pub has_dom: bool,
    pub resolved_styles: RenderArtifactState,
    pub computed_styles: RenderArtifactState,
    pub styled_tree: RenderArtifactState,
    pub layout_tree: RenderArtifactState,
    pub paint_output: RenderArtifactState,
    pub style_dirty: bool,
    pub layout_dirty: bool,
    pub style_invalidation: StyleInvalidationState,
}

/// Runtime-owned queue of invalidation requests awaiting the next frame.
///
/// V4 introduced explicit invalidation entry points and work plans. V5 makes
/// those requests part of runtime orchestration by retaining them until the
/// next frame consumes the planned work through the render pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingRenderWork {
    requests: Vec<RenderInvalidationRequest>,
}

impl PendingRenderWork {
    pub fn push(&mut self, request: RenderInvalidationRequest) {
        if !self.requests.contains(&request) {
            self.requests.push(request);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn requests(&self) -> &[RenderInvalidationRequest] {
        &self.requests
    }
}

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
    );

    Ok(Some(RenderPhaseBoundaryDebugSnapshot {
        style_output: style_output_snapshot,
        layout_input: layout_input_snapshot,
        layout_output: layout_output.to_debug_snapshot(),
        paint_input: paint_input.to_debug_snapshot(),
        orchestration: orchestration.to_debug_snapshot(),
    }))
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

fn build_render_frame_execution_trace(
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

fn find_page_background_color(style_output: &StylePhaseOutput<'_>) -> Option<(u8, u8, u8, u8)> {
    let root = style_output.root();

    fn is_non_transparent_rgba(rgba: (u8, u8, u8, u8)) -> bool {
        let (_r, _g, _b, a) = rgba;
        a > 0
    }

    fn from_elem(node: &StyledNode<'_>, want: &str) -> Option<(u8, u8, u8, u8)> {
        match node.node {
            Node::Element { name, .. } if name.eq_ignore_ascii_case(want) => {
                let rgba = node.style.background_color();
                if is_non_transparent_rgba(rgba) {
                    Some(rgba)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    let mut html_bg = None;
    let mut body_bg = None;

    for child in &root.children {
        if html_bg.is_none() {
            html_bg = from_elem(child, "html");
        }

        for grandchild in &child.children {
            if body_bg.is_none() {
                body_bg = from_elem(grandchild, "body");
            }
        }
    }

    body_bg.or(html_bg)
}

#[cfg(test)]
mod tests {
    use super::{
        PendingRenderWork, PhaseRerunSource, RenderArtifact, RenderArtifactLifetime,
        RenderArtifactState, RenderExtensionHook, RenderInvalidationEntryPoint,
        RenderPhaseExecutionKind, RenderPipelineDebugSnapshot, RenderRebuildTrigger,
        RenderingPhase, RenderingSubsystem, StyleInvalidationState,
        build_render_frame_execution_trace, render_artifact_ownership_contracts,
        render_extension_hook_contracts, render_invalidation_request,
        render_invalidation_request_contracts, render_phase_boundary_debug_snapshot,
        render_phase_contracts,
    };
    use crate::page::{PageState, RestyleHint};
    use css::Display;
    use gfx::paint::PaintPhaseInput;
    use html::{HtmlParseOptions, Node, parse_document};
    use layout::replaced::intrinsic::IntrinsicSize;
    use layout::{
        LayoutBox, LayoutPhaseInput, ReplacedElementInfoProvider, TextMeasurer, layout_document,
    };
    use std::sync::Arc;

    #[test]
    fn render_phase_contracts_pin_expected_phase_boundaries() {
        let contracts = render_phase_contracts();
        assert_eq!(contracts.len(), 4);

        let orchestration = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::FrameOrchestration)
            .expect("frame orchestration contract");
        assert_eq!(orchestration.coordinator, RenderingSubsystem::BrowserView);
        assert_eq!(orchestration.engine_owner, RenderingSubsystem::GfxViewport);
        assert_eq!(
            orchestration.consumes,
            &[
                RenderArtifact::StyledTree,
                RenderArtifact::ViewportMetrics,
                RenderArtifact::ResourceState,
                RenderArtifact::InputState,
            ]
        );
        assert_eq!(
            orchestration.produces,
            &[RenderArtifact::LayoutTree, RenderArtifact::PaintCommands]
        );

        let style = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::Style)
            .expect("style contract");
        assert_eq!(style.coordinator, RenderingSubsystem::BrowserRuntime);
        assert_eq!(style.engine_owner, RenderingSubsystem::CssEngine);
        assert_eq!(
            style.consumes,
            &[RenderArtifact::Dom, RenderArtifact::StylesheetSet]
        );
        assert_eq!(
            style.produces,
            &[
                RenderArtifact::ResolvedDocumentStyle,
                RenderArtifact::ComputedDocumentStyle,
                RenderArtifact::StyledTree,
            ]
        );
        assert_eq!(
            style.retained_outputs,
            &[
                RenderArtifact::ResolvedDocumentStyle,
                RenderArtifact::ComputedDocumentStyle,
            ]
        );
        assert_eq!(style.rebuilt_outputs, &[RenderArtifact::StyledTree]);

        let layout = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::Layout)
            .expect("layout contract");
        assert_eq!(layout.coordinator, RenderingSubsystem::GfxViewport);
        assert_eq!(layout.engine_owner, RenderingSubsystem::LayoutEngine);
        assert_eq!(
            layout.consumes,
            &[
                RenderArtifact::StyledTree,
                RenderArtifact::ViewportMetrics,
                RenderArtifact::TextMeasurement,
                RenderArtifact::ReplacedElementMetadata,
            ]
        );
        assert_eq!(layout.produces, &[RenderArtifact::LayoutTree]);
        assert_eq!(
            layout.rebuild_triggers,
            &[
                RenderRebuildTrigger::StyleOutputsChanged,
                RenderRebuildTrigger::DomTextChanged,
                RenderRebuildTrigger::ViewportChanged,
                RenderRebuildTrigger::ResourceStateChanged,
            ]
        );

        let paint = contracts
            .iter()
            .find(|contract| contract.phase == RenderingPhase::Paint)
            .expect("paint contract");
        assert_eq!(paint.coordinator, RenderingSubsystem::GfxViewport);
        assert_eq!(paint.engine_owner, RenderingSubsystem::PaintEngine);
        assert_eq!(
            paint.consumes,
            &[
                RenderArtifact::LayoutTree,
                RenderArtifact::ResourceState,
                RenderArtifact::InputState,
            ]
        );
        assert_eq!(paint.produces, &[RenderArtifact::PaintCommands]);
        assert_eq!(
            paint.rebuild_triggers,
            &[
                RenderRebuildTrigger::LayoutOutputsChanged,
                RenderRebuildTrigger::ResourceStateChanged,
                RenderRebuildTrigger::InputStateChanged,
            ]
        );
    }

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

        let resource =
            render_invalidation_request(RenderInvalidationEntryPoint::ResourceStateChanged);
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
    }

    #[test]
    fn render_phase_boundary_debug_snapshot_is_stable_for_simple_text_flow() {
        let mut page = page_with_dom(
            "<!doctype html><html style=\"display: inline;\"><head><style>html { background-color: white; } p { color: red; }</style></head><body style=\"display: inline;\"><p style=\"display: inline;\">Hello</p></body></html>",
        );
        let mut pending = PendingRenderWork::default();
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::DocumentReplaced,
        ));
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::StylesheetSetChanged,
        ));

        let snapshot = render_phase_boundary_debug_snapshot(
            &mut page,
            pending,
            320.0,
            &FixedTextMeasurer,
            None,
            false,
        )
        .expect("snapshot should build")
        .expect("document should produce a pipeline snapshot");
        let first = snapshot.to_debug_snapshot();
        let second = render_phase_boundary_debug_snapshot(
            &mut page,
            pending_for_simple_text_flow(),
            320.0,
            &FixedTextMeasurer,
            None,
            false,
        )
        .expect("snapshot should rebuild deterministically")
        .expect("document should still produce a pipeline snapshot")
        .to_debug_snapshot();
        assert_eq!(first, second);
        assert_eq!(
            first,
            r#"version: 1
render-phase-boundaries
style-output:
  version: 1
  style-phase-output
  root-id: 0
  styled-nodes: 8
  node[0]: id=0 kind=document children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    node[1]: id=0 kind=element name="html" children=2 style=display=inline color=rgba(0,0,0,255) background=rgba(255,255,255,255) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[2]: id=0 kind=element name="head" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[3]: id=0 kind=element name="style" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          node[4]: id=0 kind=text text="html { background-color: white; } p { color: red; }" children=0 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[5]: id=0 kind=element name="body" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[6]: id=0 kind=element name="p" children=1 style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          node[7]: id=0 kind=text text="Hello" children=0 style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
layout-input:
  version: 1
  layout-phase-input
  available-width: 320.00
  style-root-id: 0
  style-root: document
  style-nodes: 8
  has-replaced-info: false
layout-output:
  version: 1
  layout-phase-output
  viewport-width: 320.00
  document-rect: x=0.00 y=0.00 w=320.00 h=0.00
  layout-boxes: 8
  box[0]: id=0 node=document kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    box[1]: id=0 node=element("html") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(255,255,255,255) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[2]: id=0 node=element("head") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[3]: id=0 node=element("style") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[4]: id=0 node=text("html { background-color: white; } p { color: red; }") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[5]: id=0 node=element("body") kind=inline rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[6]: id=0 node=element("p") kind=inline rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[7]: id=0 node=text("Hello") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
paint-input:
  version: 1
  paint-phase-input
  layout-root-id: 0
  viewport-width: 320.00
  document-rect: x=0.00 y=0.00 w=320.00 h=0.00
    layout-boxes: 8
    box[0]: id=0 node=document kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[1]: id=0 node=element("html") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(255,255,255,255) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[2]: id=0 node=element("head") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[3]: id=0 node=element("style") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
            box[4]: id=0 node=text("html { background-color: white; } p { color: red; }") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[5]: id=0 node=element("body") kind=inline rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[6]: id=0 node=element("p") kind=inline rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
            box[7]: id=0 node=text("Hello") kind=block rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
orchestration:
  version: 1
  render-frame-execution-trace
  triggered-entry-points: 2
    - document-replaced
    - stylesheet-set-changed
  style: phase=style kind=requested
    direct-triggers: 2
      - dom-replaced
      - stylesheet-set-changed
    cascaded-from: 0
  layout: phase=layout kind=requested
    direct-triggers: 0
    cascaded-from: 1
      - style
  paint: phase=paint kind=requested
    direct-triggers: 0
    cascaded-from: 1
      - layout
  frame-orchestration: phase=frame-orchestration kind=requested
    direct-triggers: 0
    cascaded-from: 1
      - style
  semantic-phase-order: style -> layout -> paint
"#
        );
    }

    #[test]
    fn render_phase_boundary_debug_snapshot_is_stable_for_replaced_element_flow() {
        let mut page = page_with_dom(
            "<!doctype html><html style=\"display: inline;\"><head><style>img { display: inline-block; }</style></head><body style=\"display: inline;\"><img src=\"hero.png\"></body></html>",
        );
        let warm_style = style_output_for_test(&mut page);
        drop(warm_style);

        let mut pending = PendingRenderWork::default();
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::ResourceStateChanged,
        ));
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::InputStateChanged,
        ));

        let snapshot = render_phase_boundary_debug_snapshot(
            &mut page,
            pending,
            240.0,
            &FixedTextMeasurer,
            Some(&FixedReplacedInfo),
            true,
        )
        .expect("snapshot should build")
        .expect("document should produce a pipeline snapshot");
        let first = snapshot.to_debug_snapshot();
        let second = render_phase_boundary_debug_snapshot(
            &mut page,
            pending_for_replaced_element_flow(),
            240.0,
            &FixedTextMeasurer,
            Some(&FixedReplacedInfo),
            true,
        )
        .expect("snapshot should rebuild deterministically")
        .expect("document should still produce a pipeline snapshot")
        .to_debug_snapshot();
        assert_eq!(first, second);
        assert_eq!(
            first,
            r#"version: 1
render-phase-boundaries
style-output:
  version: 1
  style-phase-output
  root-id: 0
  styled-nodes: 7
  node[0]: id=0 kind=document children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    node[1]: id=0 kind=element name="html" children=2 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[2]: id=0 kind=element name="head" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[3]: id=0 kind=element name="style" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          node[4]: id=0 kind=text text="img { display: inline-block; }" children=0 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[5]: id=0 kind=element name="body" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[6]: id=0 kind=element name="img" children=0 style=display=inline-block color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
layout-input:
  version: 1
  layout-phase-input
  available-width: 240.00
  style-root-id: 0
  style-root: document
  style-nodes: 7
  has-replaced-info: true
layout-output:
  version: 1
  layout-phase-output
  viewport-width: 240.00
  document-rect: x=0.00 y=0.00 w=240.00 h=0.00
  layout-boxes: 7
  box[0]: id=0 node=document kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    box[1]: id=0 node=element("html") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[2]: id=0 node=element("head") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[3]: id=0 node=element("style") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[4]: id=0 node=text("img { display: inline-block; }") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[5]: id=0 node=element("body") kind=inline rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[6]: id=0 node=element("img") kind=replaced-inline rect=x=0.00 y=0.00 w=64.00 h=32.00 children=0 marker=none replaced=img intrinsic=w=64.00px h=32.00px ratio=2.0000 style=display=inline-block color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
paint-input:
  version: 1
  paint-phase-input
  layout-root-id: 0
  viewport-width: 240.00
  document-rect: x=0.00 y=0.00 w=240.00 h=0.00
    layout-boxes: 7
    box[0]: id=0 node=document kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[1]: id=0 node=element("html") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[2]: id=0 node=element("head") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[3]: id=0 node=element("style") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
            box[4]: id=0 node=text("img { display: inline-block; }") kind=block rect=x=0.00 y=0.00 w=240.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[5]: id=0 node=element("body") kind=inline rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[6]: id=0 node=element("img") kind=replaced-inline rect=x=0.00 y=0.00 w=64.00 h=32.00 children=0 marker=none replaced=img intrinsic=w=64.00px h=32.00px ratio=2.0000 style=display=inline-block color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
orchestration:
  version: 1
  render-frame-execution-trace
  triggered-entry-points: 3
    - resource-state-changed
    - input-state-changed
    - viewport-changed
  style: phase=style kind=materialized-from-retained-artifacts
    direct-triggers: 0
    cascaded-from: 0
  layout: phase=layout kind=requested
    direct-triggers: 2
      - resource-state-changed
      - viewport-changed
    cascaded-from: 0
  paint: phase=paint kind=requested
    direct-triggers: 2
      - resource-state-changed
      - input-state-changed
    cascaded-from: 1
      - layout
  frame-orchestration: phase=frame-orchestration kind=requested
    direct-triggers: 3
      - resource-state-changed
      - input-state-changed
      - viewport-changed
    cascaded-from: 0
  semantic-phase-order: style -> layout -> paint
"#
        );
    }

    #[test]
    fn render_phase_boundary_debug_snapshot_preserves_non_zero_dom_identity_across_handoffs() {
        let mut page = page_with_node(doc_with_explicit_ids());

        let warm_style = style_output_for_test(&mut page);
        assert_eq!(warm_style.root().node_id, html::internal::Id(1));

        let paragraph = find_styled_node_id(warm_style.root(), html::internal::Id(4))
            .expect("paragraph styled node");
        assert!(matches!(paragraph.node, Node::Element { name, .. } if name.as_ref() == "p"));
        drop(warm_style);

        let style_output = style_output_for_test(&mut page);
        let layout_input =
            LayoutPhaseInput::from_style_output(&style_output, 360.0, &FixedTextMeasurer, None);
        assert_eq!(layout_input.style_root().node_id, html::internal::Id(1));

        let layout_output = layout_document(layout_input);
        assert_eq!(layout_output.root().node_id(), html::internal::Id(1));
        let paragraph_box = find_layout_box_by_id(layout_output.root(), html::internal::Id(4))
            .expect("paragraph layout box");
        assert_eq!(paragraph_box.node_id(), html::internal::Id(4));

        let paint_input = PaintPhaseInput::new(&layout_output);
        assert_eq!(paint_input.layout_root().node_id(), html::internal::Id(1));
        drop(style_output);

        let snapshot = render_phase_boundary_debug_snapshot(
            &mut page,
            PendingRenderWork::default(),
            360.0,
            &FixedTextMeasurer,
            None,
            false,
        )
        .expect("snapshot should build")
        .expect("document should produce a pipeline snapshot")
        .to_debug_snapshot();

        assert!(snapshot.contains("root-id: 1"));
        assert!(snapshot.contains("style-root-id: 1"));
        assert!(snapshot.contains("layout-root-id: 1"));
        assert!(snapshot.contains("id=4 kind=element name=\"p\""));
        assert!(snapshot.contains("id=4 node=element(\"p\")"));
    }

    #[test]
    fn render_artifact_ownership_contracts_pin_retained_vs_rebuilt_lifetimes() {
        let contracts = render_artifact_ownership_contracts();
        assert_eq!(contracts.len(), 12);

        let dom = artifact_contract(contracts, RenderArtifact::Dom);
        assert_eq!(dom.semantic_owner, RenderingSubsystem::BrowserRuntime);
        assert_eq!(
            dom.retention_owner,
            Some(RenderingSubsystem::BrowserRuntime)
        );
        assert_eq!(dom.lifetime, RenderArtifactLifetime::RetainedAcrossUpdates);

        let resolved = artifact_contract(contracts, RenderArtifact::ResolvedDocumentStyle);
        assert_eq!(resolved.semantic_owner, RenderingSubsystem::CssEngine);
        assert_eq!(
            resolved.retention_owner,
            Some(RenderingSubsystem::BrowserRuntime)
        );
        assert_eq!(
            resolved.lifetime,
            RenderArtifactLifetime::RetainedAcrossUpdates
        );

        let styled = artifact_contract(contracts, RenderArtifact::StyledTree);
        assert_eq!(styled.semantic_owner, RenderingSubsystem::CssEngine);
        assert_eq!(styled.retention_owner, None);
        assert_eq!(
            styled.lifetime,
            RenderArtifactLifetime::BorrowBackedRebuiltOnDemand
        );

        let layout = artifact_contract(contracts, RenderArtifact::LayoutTree);
        assert_eq!(layout.semantic_owner, RenderingSubsystem::LayoutEngine);
        assert_eq!(layout.retention_owner, None);
        assert_eq!(
            layout.lifetime,
            RenderArtifactLifetime::FrameLocalRebuiltPerFrame
        );

        let paint = artifact_contract(contracts, RenderArtifact::PaintCommands);
        assert_eq!(paint.semantic_owner, RenderingSubsystem::PaintEngine);
        assert_eq!(paint.retention_owner, None);
        assert_eq!(paint.lifetime, RenderArtifactLifetime::ImmediateFrameOutput);

        let input = artifact_contract(contracts, RenderArtifact::InputState);
        assert_eq!(input.semantic_owner, RenderingSubsystem::BrowserRuntime);
        assert_eq!(
            input.retention_owner,
            Some(RenderingSubsystem::BrowserRuntime)
        );
        assert_eq!(
            input.lifetime,
            RenderArtifactLifetime::RetainedAcrossUpdates
        );
    }

    #[test]
    fn render_artifact_ownership_contracts_cover_each_artifact_once() {
        let contracts = render_artifact_ownership_contracts();
        let expected = [
            RenderArtifact::Dom,
            RenderArtifact::StylesheetSet,
            RenderArtifact::ResolvedDocumentStyle,
            RenderArtifact::ComputedDocumentStyle,
            RenderArtifact::StyledTree,
            RenderArtifact::ViewportMetrics,
            RenderArtifact::TextMeasurement,
            RenderArtifact::ReplacedElementMetadata,
            RenderArtifact::LayoutTree,
            RenderArtifact::ResourceState,
            RenderArtifact::InputState,
            RenderArtifact::PaintCommands,
        ];

        for artifact in expected {
            let count = contracts
                .iter()
                .filter(|contract| contract.artifact == artifact)
                .count();
            assert_eq!(
                count, 1,
                "artifact must have exactly one ownership contract: {artifact:?}"
            );
        }

        assert_eq!(contracts.len(), expected.len());
    }

    #[test]
    fn render_extension_hook_contracts_cover_expected_future_work_once() {
        let contracts = render_extension_hook_contracts();
        let expected = [
            RenderExtensionHook::BoxTreeFormalization,
            RenderExtensionHook::ConstraintSizingAndIntrinsicLayout,
            RenderExtensionHook::PaintPrimitiveAndDisplayListExpansion,
            RenderExtensionHook::IncrementalInvalidationAndDependencyTracking,
            RenderExtensionHook::RetainedLayoutState,
            RenderExtensionHook::RetainedPaintSceneState,
            RenderExtensionHook::RuntimeFrameSchedulingIncrementality,
        ];

        for hook in expected {
            let count = contracts
                .iter()
                .filter(|contract| contract.hook == hook)
                .count();
            assert_eq!(
                count, 1,
                "extension hook must have exactly one contract: {hook:?}"
            );
        }

        assert_eq!(contracts.len(), expected.len());
    }

    #[test]
    fn render_extension_hook_contracts_anchor_deferred_work_to_current_pipeline() {
        let phase_contracts = render_phase_contracts();
        let artifact_contracts = render_artifact_ownership_contracts();
        let invalidation_contracts = render_invalidation_request_contracts();

        for hook in render_extension_hook_contracts() {
            assert!(
                !hook.phases.is_empty(),
                "extension hook must anchor to at least one phase: {:?}",
                hook.hook
            );
            assert!(
                !hook.artifacts.is_empty(),
                "extension hook must anchor to at least one artifact: {:?}",
                hook.hook
            );

            for phase in hook.phases {
                assert!(
                    phase_contracts
                        .iter()
                        .any(|contract| contract.phase == *phase),
                    "extension hook references unknown phase: {:?} -> {:?}",
                    hook.hook,
                    phase
                );
            }

            for artifact in hook.artifacts {
                assert!(
                    artifact_contracts
                        .iter()
                        .any(|contract| contract.artifact == *artifact),
                    "extension hook references unknown artifact: {:?} -> {:?}",
                    hook.hook,
                    artifact
                );
            }

            for entry_point in hook.invalidation_entry_points {
                assert!(
                    invalidation_contracts
                        .iter()
                        .any(|contract| contract.entry_point == *entry_point),
                    "extension hook references unknown invalidation entry point: {:?} -> {:?}",
                    hook.hook,
                    entry_point
                );
            }
        }

        let retained_layout = render_extension_hook_contracts()
            .iter()
            .find(|contract| contract.hook == RenderExtensionHook::RetainedLayoutState)
            .expect("retained layout hook");
        assert_eq!(
            retained_layout.integration_owner,
            RenderingSubsystem::BrowserRuntime
        );
        assert!(
            retained_layout
                .artifacts
                .contains(&RenderArtifact::LayoutTree)
        );

        let retained_paint = render_extension_hook_contracts()
            .iter()
            .find(|contract| contract.hook == RenderExtensionHook::RetainedPaintSceneState)
            .expect("retained paint hook");
        assert_eq!(
            retained_paint.integration_owner,
            RenderingSubsystem::BrowserRuntime
        );
        assert!(
            retained_paint
                .artifacts
                .contains(&RenderArtifact::PaintCommands)
        );

        let invalidation = render_extension_hook_contracts()
            .iter()
            .find(|contract| {
                contract.hook == RenderExtensionHook::IncrementalInvalidationAndDependencyTracking
            })
            .expect("incremental invalidation hook");
        let expected_entry_points = render_invalidation_request_contracts()
            .iter()
            .map(|contract| contract.entry_point)
            .collect::<Vec<_>>();
        assert_eq!(
            invalidation.integration_owner,
            RenderingSubsystem::BrowserRuntime
        );
        assert_eq!(
            invalidation.invalidation_entry_points,
            expected_entry_points.as_slice()
        );
    }

    #[test]
    fn phase_contract_outputs_align_with_artifact_lifetimes() {
        let ownership = render_artifact_ownership_contracts();

        for phase in render_phase_contracts() {
            for artifact in phase.retained_outputs {
                let contract = artifact_contract(ownership, *artifact);
                assert_eq!(
                    contract.lifetime,
                    RenderArtifactLifetime::RetainedAcrossUpdates,
                    "phase retained output must have retained artifact lifetime: {artifact:?}"
                );
                assert!(
                    contract.retention_owner.is_some(),
                    "retained artifact must have a retention owner: {artifact:?}"
                );
            }

            for artifact in phase.rebuilt_outputs {
                let contract = artifact_contract(ownership, *artifact);
                assert_ne!(
                    contract.lifetime,
                    RenderArtifactLifetime::RetainedAcrossUpdates,
                    "rebuilt phase output must not be retained: {artifact:?}"
                );
                assert_eq!(
                    contract.retention_owner, None,
                    "rebuilt artifact must not have a retention owner: {artifact:?}"
                );
            }
        }
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
        assert_eq!(
            snapshot,
            RenderPipelineDebugSnapshot {
                has_dom: true,
                resolved_styles: RenderArtifactState::RetainedFresh,
                computed_styles: RenderArtifactState::RetainedFresh,
                styled_tree: RenderArtifactState::BorrowBackedRebuiltOnDemand,
                layout_tree: RenderArtifactState::FrameLocalRebuiltPerFrame,
                paint_output: RenderArtifactState::ImmediateFrameOutput,
                style_dirty: false,
                layout_dirty: true,
                style_invalidation: StyleInvalidationState::None,
            }
        );
    }

    #[test]
    fn style_to_layout_handoff_uses_explicit_phase_output_models() {
        let mut page = page_with_dom(
            "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        );
        let style_output = style_output_for_test(&mut page);
        let paragraph = find_styled_element(style_output.root(), "p").expect("paragraph");
        let measurer = FixedTextMeasurer;

        let layout_input =
            LayoutPhaseInput::from_style_output(&style_output, 320.0, &measurer, None);
        assert!(std::ptr::eq(layout_input.style_root(), style_output.root()));
        assert_eq!(layout_input.available_width(), 320.0);

        let layout_output = layout_document(layout_input);
        let layout_root = layout_output.root();
        let paragraph_box =
            find_layout_box_by_id(layout_root, paragraph.node_id).expect("paragraph layout box");
        assert_eq!(layout_output.document_rect(), layout_root.rect);
        assert_eq!(layout_output.viewport_width(), 320.0);
        assert_eq!(layout_root.node_id(), style_output.root().node_id);
        assert_eq!(paragraph_box.node_id(), paragraph.node_id);
    }

    #[test]
    fn runtime_style_phase_applies_minimal_ua_display_defaults() {
        let mut page = page_with_dom(
            "<!doctype html><html><head></head><body><p>Hello <span>world</span></p><ul><li>One</li></ul><button>Go</button></body></html>",
        );
        let style_output = style_output_for_test(&mut page);

        assert_eq!(
            styled_element_display(style_output.root(), "html"),
            Display::Block
        );
        assert_eq!(
            styled_element_display(style_output.root(), "body"),
            Display::Block
        );
        assert_eq!(
            styled_element_display(style_output.root(), "p"),
            Display::Block
        );
        assert_eq!(
            styled_element_display(style_output.root(), "span"),
            Display::Inline
        );
        assert_eq!(
            styled_element_display(style_output.root(), "li"),
            Display::ListItem
        );
        assert_eq!(
            styled_element_display(style_output.root(), "button"),
            Display::InlineBlock
        );

        let measurer = FixedTextMeasurer;
        let layout_output = layout_document(LayoutPhaseInput::from_style_output(
            &style_output,
            320.0,
            &measurer,
            None,
        ));

        assert!(
            layout_output.content_height() > 0.0,
            "minimal UA display defaults should let ordinary body text produce visible layout"
        );
    }

    #[test]
    fn layout_to_paint_handoff_wraps_layout_phase_output_without_reinterpretation() {
        let mut page = page_with_dom(
            "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        );
        let style_output = style_output_for_test(&mut page);
        let measurer = FixedTextMeasurer;
        let layout_output = layout_document(LayoutPhaseInput::from_style_output(
            &style_output,
            480.0,
            &measurer,
            None,
        ));

        let paint_input = PaintPhaseInput::new(&layout_output);
        assert!(std::ptr::eq(paint_input.layout(), &layout_output));
        assert!(std::ptr::eq(
            paint_input.layout_root(),
            layout_output.root()
        ));
        assert_eq!(
            paint_input.layout().document_rect(),
            layout_output.document_rect()
        );
        assert_eq!(
            paint_input.layout_root().node_id(),
            layout_output.root().node_id()
        );
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
        assert!(stale.style_dirty);
        assert!(stale.layout_dirty);

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
                style_dirty: true,
                layout_dirty: true,
                style_invalidation: StyleInvalidationState::Full,
            }
        );
    }

    fn page_with_dom(input: &str) -> PageState {
        let output = parse_document(input, HtmlParseOptions::default()).expect("parse should work");
        page_with_node(output.document)
    }

    fn page_with_node(dom: Node) -> PageState {
        let mut page = PageState::new();
        page.start_nav("https://example.com/index.html");
        let _ = page.replace_dom(Box::new(dom), RestyleHint::document_replaced());
        let _ = page.reconcile_document_stylesheets();
        page
    }

    fn artifact_contract(
        contracts: &[super::RenderArtifactOwnershipContract],
        artifact: RenderArtifact,
    ) -> &super::RenderArtifactOwnershipContract {
        contracts
            .iter()
            .find(|contract| contract.artifact == artifact)
            .expect("artifact contract should exist")
    }

    fn style_output_for_test(page: &mut PageState) -> css::StylePhaseOutput<'_> {
        page.build_style_phase_output()
            .expect("style phase output should build")
            .expect("document should be styled")
    }

    fn styled_element_color(node: &css::StyledNode<'_>, want_name: &str) -> (u8, u8, u8, u8) {
        find_styled_element(node, want_name)
            .map(|node| node.style.color())
            .expect("styled element should exist")
    }

    fn styled_element_display(node: &css::StyledNode<'_>, want_name: &str) -> Display {
        find_styled_element(node, want_name)
            .map(|node| node.style.display())
            .expect("styled element should exist")
    }

    fn find_styled_element<'a>(
        node: &'a css::StyledNode<'a>,
        want_name: &str,
    ) -> Option<&'a css::StyledNode<'a>> {
        if let Node::Element { name, .. } = node.node
            && name.as_ref() == want_name
        {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_styled_element(child, want_name))
    }

    fn find_styled_node_id<'a>(
        node: &'a css::StyledNode<'a>,
        want: html::internal::Id,
    ) -> Option<&'a css::StyledNode<'a>> {
        if node.node_id == want {
            return Some(node);
        }

        node.children
            .iter()
            .find_map(|child| find_styled_node_id(child, want))
    }

    fn find_layout_box_by_id<'layout, 'dom>(
        layout: &'layout LayoutBox<'layout, 'dom>,
        want: html::internal::Id,
    ) -> Option<&'layout LayoutBox<'layout, 'dom>> {
        if layout.node_id() == want {
            return Some(layout);
        }

        layout
            .children
            .iter()
            .find_map(|child| find_layout_box_by_id(child, want))
    }

    fn set_first_element_attr(
        node: &mut Node,
        want_name: &str,
        attr_name: &str,
        value: Option<String>,
    ) -> html::internal::Id {
        match node {
            Node::Document { children, .. } => children
                .iter_mut()
                .find_map(|child| {
                    set_first_element_attr_optional(child, want_name, attr_name, value.clone())
                })
                .expect("target element should exist"),
            Node::Element {
                id,
                name,
                attributes,
                children,
                ..
            } => {
                if name.as_ref() == want_name {
                    if let Some(existing) = attributes
                        .iter_mut()
                        .find(|(name, _)| name.eq_ignore_ascii_case(attr_name))
                    {
                        existing.1 = value;
                    } else {
                        attributes.push((Arc::from(attr_name), value));
                    }
                    *id
                } else {
                    children
                        .iter_mut()
                        .find_map(|child| {
                            set_first_element_attr_optional(
                                child,
                                want_name,
                                attr_name,
                                value.clone(),
                            )
                        })
                        .expect("target element should exist")
                }
            }
            Node::Text { .. } | Node::Comment { .. } => panic!("target element should exist"),
        }
    }

    fn set_first_element_attr_optional(
        node: &mut Node,
        want_name: &str,
        attr_name: &str,
        value: Option<String>,
    ) -> Option<html::internal::Id> {
        match node {
            Node::Document { children, .. } => children.iter_mut().find_map(|child| {
                set_first_element_attr_optional(child, want_name, attr_name, value.clone())
            }),
            Node::Element {
                id,
                name,
                attributes,
                children,
                ..
            } => {
                if name.as_ref() == want_name {
                    if let Some(existing) = attributes
                        .iter_mut()
                        .find(|(name, _)| name.eq_ignore_ascii_case(attr_name))
                    {
                        existing.1 = value;
                    } else {
                        attributes.push((Arc::from(attr_name), value));
                    }
                    Some(*id)
                } else {
                    children.iter_mut().find_map(|child| {
                        set_first_element_attr_optional(child, want_name, attr_name, value.clone())
                    })
                }
            }
            Node::Text { .. } | Node::Comment { .. } => None,
        }
    }

    fn replace_first_text(node: &mut Node, before: &str, after: &str) -> html::internal::Id {
        replace_first_text_optional(node, before, after).expect("target text should exist")
    }

    fn replace_first_text_optional(
        node: &mut Node,
        before: &str,
        after: &str,
    ) -> Option<html::internal::Id> {
        match node {
            Node::Document { children, .. } | Node::Element { children, .. } => children
                .iter_mut()
                .find_map(|child| replace_first_text_optional(child, before, after)),
            Node::Text { id, text } if text == before => {
                *text = after.to_string();
                Some(*id)
            }
            Node::Text { .. } | Node::Comment { .. } => None,
        }
    }

    struct FixedTextMeasurer;

    impl TextMeasurer for FixedTextMeasurer {
        fn measure(&self, text: &str, _style: &css::ComputedStyle) -> f32 {
            text.chars().count() as f32 * 8.0
        }

        fn line_height(&self, _style: &css::ComputedStyle) -> f32 {
            16.0
        }
    }

    fn pending_for_simple_text_flow() -> PendingRenderWork {
        let mut pending = PendingRenderWork::default();
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::DocumentReplaced,
        ));
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::StylesheetSetChanged,
        ));
        pending
    }

    fn pending_for_replaced_element_flow() -> PendingRenderWork {
        let mut pending = PendingRenderWork::default();
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::ResourceStateChanged,
        ));
        pending.push(render_invalidation_request(
            RenderInvalidationEntryPoint::InputStateChanged,
        ));
        pending
    }

    struct FixedReplacedInfo;

    impl ReplacedElementInfoProvider for FixedReplacedInfo {
        fn intrinsic_for_img(&self, _node: &html::Node) -> Option<IntrinsicSize> {
            Some(IntrinsicSize::from_w_h(Some(64.0), Some(32.0)))
        }
    }

    fn doc_with_explicit_ids() -> Node {
        Node::Document {
            id: html::internal::Id(1),
            doctype: None,
            children: vec![Node::Element {
                id: html::internal::Id(2),
                name: Arc::from("html"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: vec![Node::Element {
                    id: html::internal::Id(3),
                    name: Arc::from("body"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: vec![Node::Element {
                        id: html::internal::Id(4),
                        name: Arc::from("p"),
                        attributes: Vec::new(),
                        style: Vec::new(),
                        children: vec![Node::Text {
                            id: html::internal::Id(5),
                            text: "Hello".to_string(),
                        }],
                    }],
                }],
            }],
        }
    }
}
