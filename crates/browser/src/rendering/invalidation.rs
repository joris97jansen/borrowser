//! Runtime invalidation entry points and pending render work.

use super::types::{
    PaintInvalidationReason, PaintInvalidationRequest, PaintInvalidationScope,
    PaintInvalidationTrigger, RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase,
    RenderingSubsystem, RepaintExecutionPlan, RepaintExecutionScope,
};

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

impl RenderInvalidationRequest {
    pub fn paint_invalidation(self) -> Option<PaintInvalidationRequest> {
        match self.work.paint {
            PhaseRerunSource::None => None,
            PhaseRerunSource::Direct(_) | PhaseRerunSource::CascadedFrom(_) => {
                Some(paint_invalidation_request(self.entry_point))
            }
        }
    }
}

pub(crate) const ALL_INVALIDATION_ENTRY_POINTS: &[RenderInvalidationEntryPoint] = &[
    RenderInvalidationEntryPoint::DocumentReplaced,
    RenderInvalidationEntryPoint::DomStructureChanged,
    RenderInvalidationEntryPoint::DomAttributesChanged,
    RenderInvalidationEntryPoint::DomTextChanged,
    RenderInvalidationEntryPoint::StylesheetSetChanged,
    RenderInvalidationEntryPoint::ViewportChanged,
    RenderInvalidationEntryPoint::ResourceStateChanged,
    RenderInvalidationEntryPoint::InputStateChanged,
];
pub(crate) const STYLE_LAYOUT_INVALIDATION_ENTRY_POINTS: &[RenderInvalidationEntryPoint] = &[
    RenderInvalidationEntryPoint::DocumentReplaced,
    RenderInvalidationEntryPoint::DomStructureChanged,
    RenderInvalidationEntryPoint::DomAttributesChanged,
    RenderInvalidationEntryPoint::DomTextChanged,
    RenderInvalidationEntryPoint::StylesheetSetChanged,
    RenderInvalidationEntryPoint::ViewportChanged,
    RenderInvalidationEntryPoint::ResourceStateChanged,
];
pub(crate) const LAYOUT_PAINT_INVALIDATION_ENTRY_POINTS: &[RenderInvalidationEntryPoint] = &[
    RenderInvalidationEntryPoint::DocumentReplaced,
    RenderInvalidationEntryPoint::DomStructureChanged,
    RenderInvalidationEntryPoint::DomAttributesChanged,
    RenderInvalidationEntryPoint::DomTextChanged,
    RenderInvalidationEntryPoint::StylesheetSetChanged,
    RenderInvalidationEntryPoint::ViewportChanged,
    RenderInvalidationEntryPoint::ResourceStateChanged,
    RenderInvalidationEntryPoint::InputStateChanged,
];

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

static PAINT_INVALIDATION_REQUEST_CONTRACTS: [PaintInvalidationRequest; 8] = [
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

/// Stable paint-invalidation contract table.
///
/// Each entry explains why paint is dirty and which conservative repaint scope
/// is affected when the corresponding runtime invalidation entry point requests
/// a paint rerun. The scope is a scheduling/invalidation contract, not a
/// retained scene key or backend partial-raster command.
pub fn paint_invalidation_request_contracts() -> &'static [PaintInvalidationRequest] {
    &PAINT_INVALIDATION_REQUEST_CONTRACTS
}

pub fn paint_invalidation_request(
    entry_point: RenderInvalidationEntryPoint,
) -> PaintInvalidationRequest {
    *paint_invalidation_request_contracts()
        .iter()
        .find(|contract| contract.entry_point == entry_point)
        .expect("paint invalidation contract must exist for every paint entry point")
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

    pub fn paint_invalidations(&self) -> PendingPaintInvalidations {
        let mut pending = PendingPaintInvalidations::default();
        for request in &self.requests {
            if let Some(paint_invalidation) = request.paint_invalidation() {
                pending.push(paint_invalidation);
            }
        }
        pending
    }
}

/// Derived, deterministic view of pending paint invalidations.
///
/// This is intentionally derived from `PendingRenderWork` instead of retained
/// separately. AB5 introduces structured paint invalidation, not a retained
/// paint scene, display list, compositor, or backend partial-raster scheduler.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingPaintInvalidations {
    requests: Vec<PaintInvalidationRequest>,
}

impl PendingPaintInvalidations {
    pub fn push(&mut self, request: PaintInvalidationRequest) {
        if !self.requests.contains(&request) {
            self.requests.push(request);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn requests(&self) -> &[PaintInvalidationRequest] {
        &self.requests
    }

    pub fn effective_scope(&self) -> Option<PaintInvalidationScope> {
        self.requests.iter().map(|request| request.scope).max()
    }
}

impl RepaintExecutionPlan {
    pub const fn document() -> Self {
        Self {
            scope: RepaintExecutionScope::Document,
        }
    }

    pub const fn viewport() -> Self {
        Self {
            scope: RepaintExecutionScope::Viewport,
        }
    }

    pub fn from_pending_render_work(pending: &PendingRenderWork) -> Self {
        Self::from_paint_invalidations(&pending.paint_invalidations())
    }

    pub fn from_frame_inputs(pending: &PendingRenderWork, viewport_changed: bool) -> Self {
        let paint = pending.paint_invalidations();
        match paint.effective_scope() {
            Some(PaintInvalidationScope::Document) => Self::document(),
            Some(PaintInvalidationScope::Viewport) => Self::viewport(),
            None if viewport_changed => Self::viewport(),
            None => Self::document(),
        }
    }

    pub fn from_paint_invalidations(pending: &PendingPaintInvalidations) -> Self {
        match pending.effective_scope() {
            Some(PaintInvalidationScope::Viewport) => Self::viewport(),
            Some(PaintInvalidationScope::Document) | None => Self::document(),
        }
    }
}
