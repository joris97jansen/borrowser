//! Runtime invalidation entry points and pending render work.

use super::types::{
    RenderInvalidationEntryPoint, RenderRebuildTrigger, RenderingPhase, RenderingSubsystem,
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
