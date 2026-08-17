//! Runtime invalidation entry points and pending render work.

use super::types::{
    DirtyEntry, DirtyPhase, DirtyPropagationResult, DirtyReason, DirtyScope,
    PaintInvalidationReason, PaintInvalidationRequest, PaintInvalidationScope,
    PaintInvalidationTrigger, RenderDirtyRequest, RenderDirtyState, RenderInvalidationEntryPoint,
    RenderRebuildTrigger, RenderingPhase, RenderingSubsystem, RepaintExecutionPlan,
    RepaintExecutionScope,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseRerunSource {
    None,
    Direct(RenderRebuildTrigger),
    CascadedFrom(RenderingPhase),
}

/// Read-only phase work produced by the rendering invalidation factories.
///
/// Construction stays inside this module so phase relationships cannot be
/// assembled independently of the rendering contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderInvalidationWorkPlan {
    style: PhaseRerunSource,
    layout: PhaseRerunSource,
    paint: PhaseRerunSource,
    frame_orchestration: PhaseRerunSource,
}

impl RenderInvalidationWorkPlan {
    pub const fn style(self) -> PhaseRerunSource {
        self.style
    }

    pub const fn layout(self) -> PhaseRerunSource {
        self.layout
    }

    pub const fn paint(self) -> PhaseRerunSource {
        self.paint
    }

    pub const fn frame_orchestration(self) -> PhaseRerunSource {
        self.frame_orchestration
    }

    pub const fn requests_redraw(self) -> bool {
        !matches!(self.frame_orchestration, PhaseRerunSource::None)
    }
}

/// A validated runtime invalidation request.
///
/// Consumers may inspect this value, but production construction is owned by
/// the intrinsic request and typed CSS Style composition factories below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderInvalidationRequest {
    entry_point: RenderInvalidationEntryPoint,
    requested_by: RenderingSubsystem,
    requested_work: RenderInvalidationWorkPlan,
}

impl RenderInvalidationRequest {
    pub const fn entry_point(self) -> RenderInvalidationEntryPoint {
        self.entry_point
    }

    pub const fn requested_by(self) -> RenderingSubsystem {
        self.requested_by
    }

    pub const fn requested_work(self) -> RenderInvalidationWorkPlan {
        self.requested_work
    }

    pub const fn requests_style_work(self) -> bool {
        !matches!(self.requested_work.style, PhaseRerunSource::None)
    }

    pub fn paint_invalidation(self) -> Option<PaintInvalidationRequest> {
        let reason = match self.requested_work.paint {
            PhaseRerunSource::None => return None,
            PhaseRerunSource::CascadedFrom(RenderingPhase::Style) => {
                PaintInvalidationReason::CascadedFromStyle
            }
            PhaseRerunSource::CascadedFrom(RenderingPhase::Layout) => {
                PaintInvalidationReason::CascadedFromLayout
            }
            PhaseRerunSource::CascadedFrom(RenderingPhase::Paint)
            | PhaseRerunSource::CascadedFrom(RenderingPhase::FrameOrchestration) => {
                PaintInvalidationReason::ConservativeUnknownImpact
            }
            PhaseRerunSource::Direct(_) => paint_invalidation_request(self.entry_point).reason,
        };
        Some(PaintInvalidationRequest {
            reason,
            ..paint_invalidation_request(self.entry_point)
        })
    }

    pub fn dirty_request(self) -> RenderDirtyRequest {
        dirty_request_for_render_request(self)
    }
}

/// Browser-runtime sources for which CSS may authorize direct Style work.
///
/// Keeping this domain narrower than `RenderInvalidationEntryPoint` prevents
/// viewport, resource, and input events from manufacturing Style-phase
/// triggers that the rendering phase contract does not admit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CssStyleInvalidationSource {
    DocumentReplaced,
    DomStructureChanged,
    DomAttributesChanged,
    DomTextChanged,
    StylesheetSetChanged,
}

impl CssStyleInvalidationSource {
    #[cfg(test)]
    pub(crate) const fn from_entry_point(
        entry_point: RenderInvalidationEntryPoint,
    ) -> Option<Self> {
        match entry_point {
            RenderInvalidationEntryPoint::DocumentReplaced => Some(Self::DocumentReplaced),
            RenderInvalidationEntryPoint::DomStructureChanged => Some(Self::DomStructureChanged),
            RenderInvalidationEntryPoint::DomAttributesChanged => Some(Self::DomAttributesChanged),
            RenderInvalidationEntryPoint::DomTextChanged => Some(Self::DomTextChanged),
            RenderInvalidationEntryPoint::StylesheetSetChanged => Some(Self::StylesheetSetChanged),
            RenderInvalidationEntryPoint::ViewportChanged
            | RenderInvalidationEntryPoint::ResourceStateChanged
            | RenderInvalidationEntryPoint::InputStateChanged => None,
        }
    }

    pub(crate) const fn entry_point(self) -> RenderInvalidationEntryPoint {
        match self {
            Self::DocumentReplaced => RenderInvalidationEntryPoint::DocumentReplaced,
            Self::DomStructureChanged => RenderInvalidationEntryPoint::DomStructureChanged,
            Self::DomAttributesChanged => RenderInvalidationEntryPoint::DomAttributesChanged,
            Self::DomTextChanged => RenderInvalidationEntryPoint::DomTextChanged,
            Self::StylesheetSetChanged => RenderInvalidationEntryPoint::StylesheetSetChanged,
        }
    }

    pub(crate) const fn rebuild_trigger(self) -> RenderRebuildTrigger {
        match self {
            Self::DocumentReplaced => RenderRebuildTrigger::DomReplaced,
            Self::DomStructureChanged => RenderRebuildTrigger::DomStructureChanged,
            Self::DomAttributesChanged => RenderRebuildTrigger::DomAttributesChanged,
            Self::DomTextChanged => RenderRebuildTrigger::DomTextChanged,
            Self::StylesheetSetChanged => RenderRebuildTrigger::StylesheetSetChanged,
        }
    }
}

pub(crate) const CSS_STYLE_INVALIDATION_SOURCES: [CssStyleInvalidationSource; 5] = [
    CssStyleInvalidationSource::DocumentReplaced,
    CssStyleInvalidationSource::DomStructureChanged,
    CssStyleInvalidationSource::DomAttributesChanged,
    CssStyleInvalidationSource::DomTextChanged,
    CssStyleInvalidationSource::StylesheetSetChanged,
];

const fn css_style_rebuild_triggers<const N: usize>(
    sources: [CssStyleInvalidationSource; N],
) -> [RenderRebuildTrigger; N] {
    let mut triggers = [RenderRebuildTrigger::DomReplaced; N];
    let mut index = 0;
    while index < N {
        triggers[index] = sources[index].rebuild_trigger();
        index += 1;
    }
    triggers
}

pub(crate) const CSS_STYLE_REBUILD_TRIGGERS: [RenderRebuildTrigger; 5] =
    css_style_rebuild_triggers(CSS_STYLE_INVALIDATION_SOURCES);

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

/// Intrinsic rendering dependencies only. CSS-authorized Style work is
/// composed after CSS classification and is never fabricated by this table.
static RENDER_INVALIDATION_REQUEST_CONTRACTS: [RenderInvalidationRequest; 8] = [
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DocumentReplaced,
        requested_by: RenderingSubsystem::BrowserRuntime,
        requested_work: RenderInvalidationWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::Direct(RenderRebuildTrigger::DomReplaced),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::Direct(RenderRebuildTrigger::DomReplaced),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomStructureChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        requested_work: RenderInvalidationWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::Direct(RenderRebuildTrigger::DomStructureChanged),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::Direct(
                RenderRebuildTrigger::DomStructureChanged,
            ),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomAttributesChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        requested_work: RenderInvalidationWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::None,
            paint: PhaseRerunSource::None,
            frame_orchestration: PhaseRerunSource::None,
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomTextChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        requested_work: RenderInvalidationWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::StylesheetSetChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        requested_work: RenderInvalidationWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::None,
            paint: PhaseRerunSource::None,
            frame_orchestration: PhaseRerunSource::None,
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::ViewportChanged,
        requested_by: RenderingSubsystem::BrowserView,
        requested_work: RenderInvalidationWorkPlan {
            style: PhaseRerunSource::None,
            layout: PhaseRerunSource::Direct(RenderRebuildTrigger::ViewportChanged),
            paint: PhaseRerunSource::CascadedFrom(RenderingPhase::Layout),
            frame_orchestration: PhaseRerunSource::Direct(RenderRebuildTrigger::ViewportChanged),
        },
    },
    RenderInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::ResourceStateChanged,
        requested_by: RenderingSubsystem::BrowserRuntime,
        requested_work: RenderInvalidationWorkPlan {
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
        requested_work: RenderInvalidationWorkPlan {
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
        reason: PaintInvalidationReason::CascadedFromLayout,
        scope: PaintInvalidationScope::Document,
    },
    PaintInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomStructureChanged,
        trigger: PaintInvalidationTrigger::DomStructureChanged,
        reason: PaintInvalidationReason::CascadedFromLayout,
        scope: PaintInvalidationScope::Document,
    },
    PaintInvalidationRequest {
        entry_point: RenderInvalidationEntryPoint::DomAttributesChanged,
        trigger: PaintInvalidationTrigger::DomAttributesChanged,
        reason: PaintInvalidationReason::CascadedFromLayout,
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
        reason: PaintInvalidationReason::CascadedFromLayout,
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

/// Composes CSS-authorized Style work with the intrinsic dependencies of a
/// typed CSS style-input source. CSS plan scope remains opaque to Browser.
pub(crate) fn render_css_style_invalidation_request(
    source: CssStyleInvalidationSource,
    requested: bool,
) -> RenderInvalidationRequest {
    let mut request = render_invalidation_request(source.entry_point());
    if !requested {
        return request;
    }

    request.requested_work.style = PhaseRerunSource::Direct(source.rebuild_trigger());
    if matches!(request.requested_work.layout, PhaseRerunSource::None) {
        request.requested_work.layout = PhaseRerunSource::CascadedFrom(RenderingPhase::Style);
    }
    if matches!(request.requested_work.paint, PhaseRerunSource::None) {
        request.requested_work.paint = PhaseRerunSource::CascadedFrom(RenderingPhase::Layout);
    }
    if matches!(
        request.requested_work.frame_orchestration,
        PhaseRerunSource::None
    ) {
        request.requested_work.frame_orchestration =
            PhaseRerunSource::CascadedFrom(RenderingPhase::Style);
    }
    request
}

/// Stable paint-invalidation contract table.
///
/// Each entry provides the stable trigger and conservative scope metadata used
/// when a composed runtime request includes Paint work. The composed work plan,
/// not this metadata table, determines whether Paint is actually requested.
/// The scope is a scheduling/invalidation contract, not a retained scene key or
/// backend partial-raster command.
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

pub fn dirty_request_for_entry_point(
    entry_point: RenderInvalidationEntryPoint,
) -> RenderDirtyRequest {
    render_invalidation_request(entry_point).dirty_request()
}

fn intrinsic_dirty_request_for_entry_point(
    entry_point: RenderInvalidationEntryPoint,
) -> RenderDirtyRequest {
    let (direct, propagated): (Vec<DirtyEntry>, Vec<DirtyEntry>) = match entry_point {
        RenderInvalidationEntryPoint::DocumentReplaced => (
            vec![DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::DocumentReplaced,
                DirtyScope::Document,
            )],
            vec![DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            )],
        ),
        RenderInvalidationEntryPoint::DomStructureChanged => (
            vec![DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::DomContentChanged,
                DirtyScope::Document,
            )],
            vec![DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            )],
        ),
        RenderInvalidationEntryPoint::DomAttributesChanged
        | RenderInvalidationEntryPoint::StylesheetSetChanged => (vec![], vec![]),
        RenderInvalidationEntryPoint::DomTextChanged => (
            vec![DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::TextContentChanged,
                DirtyScope::Document,
            )],
            vec![DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Document,
            )],
        ),
        RenderInvalidationEntryPoint::ViewportChanged => (
            vec![DirtyEntry::new(
                DirtyPhase::Layout,
                DirtyReason::ViewportChanged,
                DirtyScope::Viewport,
            )],
            vec![DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::CascadedFromLayout,
                DirtyScope::Viewport,
            )],
        ),
        RenderInvalidationEntryPoint::ResourceStateChanged => (
            vec![
                DirtyEntry::new(
                    DirtyPhase::Layout,
                    DirtyReason::ResourceStateChanged,
                    DirtyScope::Document,
                ),
                DirtyEntry::new(
                    DirtyPhase::Paint,
                    DirtyReason::ResourceStateChanged,
                    DirtyScope::Document,
                ),
            ],
            vec![],
        ),
        RenderInvalidationEntryPoint::InputStateChanged => (
            vec![DirtyEntry::new(
                DirtyPhase::Paint,
                DirtyReason::RuntimeInputState,
                DirtyScope::Viewport,
            )],
            vec![],
        ),
    };

    let mut entries = Vec::with_capacity(direct.len() + propagated.len());
    entries.extend(direct);
    entries.extend(propagated);
    let mut state = RenderDirtyState::new();
    state.extend(entries);

    RenderDirtyRequest {
        entry_point,
        entries: state.entries().to_vec(),
    }
}

fn css_style_invalidation_dirty_entries(
    entry_point: RenderInvalidationEntryPoint,
) -> [DirtyEntry; 3] {
    let style_reason = match entry_point {
        RenderInvalidationEntryPoint::DocumentReplaced => DirtyReason::DocumentReplaced,
        RenderInvalidationEntryPoint::DomStructureChanged => DirtyReason::DomContentChanged,
        RenderInvalidationEntryPoint::StylesheetSetChanged => DirtyReason::StylesheetChanged,
        RenderInvalidationEntryPoint::DomAttributesChanged
        | RenderInvalidationEntryPoint::DomTextChanged
        | RenderInvalidationEntryPoint::ViewportChanged
        | RenderInvalidationEntryPoint::ResourceStateChanged
        | RenderInvalidationEntryPoint::InputStateChanged => DirtyReason::StyleInputChanged,
    };
    [
        DirtyEntry::new(DirtyPhase::Style, style_reason, DirtyScope::Document),
        DirtyEntry::new(
            DirtyPhase::Layout,
            DirtyReason::CascadedFromStyle,
            DirtyScope::Document,
        ),
        DirtyEntry::new(
            DirtyPhase::Paint,
            DirtyReason::CascadedFromLayout,
            DirtyScope::Document,
        ),
    ]
}

fn dirty_request_for_render_request(request: RenderInvalidationRequest) -> RenderDirtyRequest {
    let mut state = RenderDirtyState::new();
    state.extend(intrinsic_dirty_request_for_entry_point(request.entry_point).entries);
    if request.requests_style_work() {
        state.extend(css_style_invalidation_dirty_entries(request.entry_point));
        debug_assert!(!matches!(
            request.requested_work.layout,
            PhaseRerunSource::None
        ));
        debug_assert!(!matches!(
            request.requested_work.paint,
            PhaseRerunSource::None
        ));
        debug_assert!(request.requested_work.requests_redraw());
    }

    RenderDirtyRequest {
        entry_point: request.entry_point,
        entries: state.entries().to_vec(),
    }
}

pub fn dirty_propagation_for_entry_point(
    entry_point: RenderInvalidationEntryPoint,
) -> DirtyPropagationResult {
    let request = dirty_request_for_entry_point(entry_point);
    let mut direct = Vec::new();
    let mut propagated = Vec::new();

    for entry in &request.entries {
        match entry.reason {
            DirtyReason::CascadedFromStyle | DirtyReason::CascadedFromLayout => {
                propagated.push(*entry)
            }
            _ => direct.push(*entry),
        }
    }

    let mut state = RenderDirtyState::new();
    state.extend(request.entries);
    DirtyPropagationResult {
        direct,
        propagated,
        state,
    }
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
    pub fn push(&mut self, request: RenderInvalidationRequest) -> bool {
        if !request.requested_work.requests_redraw() {
            debug_assert!(request.dirty_request().entries.is_empty());
            debug_assert!(request.paint_invalidation().is_none());
            return false;
        }
        if !self.requests.contains(&request) {
            self.requests.push(request);
        }
        true
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

    pub fn dirty_state(&self) -> RenderDirtyState {
        let mut state = RenderDirtyState::new();
        for request in &self.requests {
            state.extend(request.dirty_request().entries);
        }
        state
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
