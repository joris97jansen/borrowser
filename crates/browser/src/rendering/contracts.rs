//! Stable rendering phase, artifact, and extension contracts.

use super::invalidation::{
    ALL_INVALIDATION_ENTRY_POINTS, LAYOUT_PAINT_INVALIDATION_ENTRY_POINTS,
    STYLE_LAYOUT_INVALIDATION_ENTRY_POINTS,
};
use super::types::{
    RenderArtifact, RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase,
    RenderingSubsystem,
};

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
const FRAME_ORCHESTRATION_REBUILDS: &[RenderArtifact] = &[RenderArtifact::PaintCommands];
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
const LAYOUT_RETAINED: &[RenderArtifact] = &[RenderArtifact::LayoutTree];
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
        retained_outputs: LAYOUT_RETAINED,
        rebuilt_outputs: &[],
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
        retention_owner: Some(RenderingSubsystem::BrowserRuntime),
        lifetime: RenderArtifactLifetime::RetainedAcrossUpdates,
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
