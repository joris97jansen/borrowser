//! Core rendering pipeline vocabulary.

use super::RetainedRenderId;

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
pub enum PaintInvalidationTrigger {
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
pub enum PaintInvalidationReason {
    CascadedFromStyle,
    CascadedFromLayout,
    DirectPaintDependency,
    RuntimeInputState,
    ConservativeUnknownImpact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaintInvalidationScope {
    Viewport,
    Document,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaintInvalidationRequest {
    pub entry_point: RenderInvalidationEntryPoint,
    pub trigger: PaintInvalidationTrigger,
    pub reason: PaintInvalidationReason,
    pub scope: PaintInvalidationScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DirtyPhase {
    Style,
    Layout,
    Paint,
}

impl DirtyPhase {
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::Style => "style",
            Self::Layout => "layout",
            Self::Paint => "paint",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DirtyReason {
    NoOp,
    DocumentReplaced,
    DomContentChanged,
    TextContentChanged,
    StyleInputChanged,
    StylesheetChanged,
    ViewportChanged,
    PaintOnlyStyleChanged,
    LayoutAffectingStyleChanged,
    CascadedFromStyle,
    CascadedFromLayout,
    RuntimeInputState,
    ResourceStateChanged,
    ConservativeUnknownImpact,
}

impl DirtyReason {
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::NoOp => "no-op",
            Self::DocumentReplaced => "document-replaced",
            Self::DomContentChanged => "dom-content-changed",
            Self::TextContentChanged => "text-content-changed",
            Self::StyleInputChanged => "style-input-changed",
            Self::StylesheetChanged => "stylesheet-changed",
            Self::ViewportChanged => "viewport-changed",
            Self::PaintOnlyStyleChanged => "paint-only-style-changed",
            Self::LayoutAffectingStyleChanged => "layout-affecting-style-changed",
            Self::CascadedFromStyle => "cascaded-from-style",
            Self::CascadedFromLayout => "cascaded-from-layout",
            Self::RuntimeInputState => "runtime-input-state",
            Self::ResourceStateChanged => "resource-state-changed",
            Self::ConservativeUnknownImpact => "conservative-unknown-impact",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DirtyScope {
    None,
    Node(RetainedRenderId),
    Artifact(RetainedRenderId),
    Subtree(RetainedRenderId),
    Viewport,
    Document,
}

impl DirtyScope {
    pub const fn debug_label(self) -> DirtyScopeDebugLabel {
        match self {
            Self::None => DirtyScopeDebugLabel::Static("none"),
            Self::Node(id) => DirtyScopeDebugLabel::RetainedId { prefix: "node", id },
            Self::Artifact(id) => DirtyScopeDebugLabel::RetainedId {
                prefix: "artifact",
                id,
            },
            Self::Subtree(id) => DirtyScopeDebugLabel::RetainedId {
                prefix: "subtree",
                id,
            },
            Self::Viewport => DirtyScopeDebugLabel::Static("viewport"),
            Self::Document => DirtyScopeDebugLabel::Static("document"),
        }
    }

    pub fn conservative_merge(self, next: Self) -> Self {
        if self == next {
            return self;
        }

        match (self, next) {
            (Self::None, scope) | (scope, Self::None) => scope,
            (Self::Document, _) | (_, Self::Document) => Self::Document,
            (Self::Viewport, Self::Viewport) => Self::Viewport,
            (Self::Viewport, _) | (_, Self::Viewport) => Self::Document,
            _ => Self::Document,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirtyScopeDebugLabel {
    Static(&'static str),
    RetainedId {
        prefix: &'static str,
        id: RetainedRenderId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DirtyEntry {
    pub phase: DirtyPhase,
    pub reason: DirtyReason,
    pub scope: DirtyScope,
}

impl DirtyEntry {
    pub const fn new(phase: DirtyPhase, reason: DirtyReason, scope: DirtyScope) -> Self {
        Self {
            phase,
            reason,
            scope,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderDirtyRequest {
    pub entry_point: RenderInvalidationEntryPoint,
    pub entries: Vec<DirtyEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderDirtyState {
    entries: Vec<DirtyEntry>,
}

impl RenderDirtyState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn document_initial() -> Self {
        let mut state = Self::new();
        state.push(DirtyEntry::new(
            DirtyPhase::Style,
            DirtyReason::ConservativeUnknownImpact,
            DirtyScope::Document,
        ));
        state.push(DirtyEntry::new(
            DirtyPhase::Layout,
            DirtyReason::CascadedFromStyle,
            DirtyScope::Document,
        ));
        state.push(DirtyEntry::new(
            DirtyPhase::Paint,
            DirtyReason::CascadedFromLayout,
            DirtyScope::Document,
        ));
        state
    }

    pub fn conservative_unknown() -> Self {
        let mut state = Self::new();
        state.push(DirtyEntry::new(
            DirtyPhase::Style,
            DirtyReason::ConservativeUnknownImpact,
            DirtyScope::Document,
        ));
        state.push(DirtyEntry::new(
            DirtyPhase::Layout,
            DirtyReason::ConservativeUnknownImpact,
            DirtyScope::Document,
        ));
        state.push(DirtyEntry::new(
            DirtyPhase::Paint,
            DirtyReason::ConservativeUnknownImpact,
            DirtyScope::Document,
        ));
        state
    }

    pub fn push(&mut self, entry: DirtyEntry) {
        if entry.scope == DirtyScope::None {
            return;
        }

        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|existing| existing.phase == entry.phase && existing.reason == entry.reason)
        {
            existing.scope = existing.scope.conservative_merge(entry.scope);
        } else {
            self.entries.push(entry);
        }
        self.entries.sort();
    }

    pub fn extend(&mut self, entries: impl IntoIterator<Item = DirtyEntry>) {
        for entry in entries {
            self.push(entry);
        }
    }

    pub fn merge(&mut self, next: &Self) {
        self.extend(next.entries.iter().copied());
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn clear_phase(&mut self, phase: DirtyPhase) {
        self.entries.retain(|entry| entry.phase != phase);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_phase_dirty(&self, phase: DirtyPhase) -> bool {
        self.entries.iter().any(|entry| entry.phase == phase)
    }

    pub fn entries(&self) -> &[DirtyEntry] {
        &self.entries
    }

    pub fn effective_scope(&self, phase: DirtyPhase) -> DirtyScope {
        self.entries
            .iter()
            .filter(|entry| entry.phase == phase)
            .map(|entry| entry.scope)
            .fold(DirtyScope::None, DirtyScope::conservative_merge)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyPropagationResult {
    pub direct: Vec<DirtyEntry>,
    pub propagated: Vec<DirtyEntry>,
    pub state: RenderDirtyState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepaintExecutionScope {
    Viewport,
    Document,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepaintExecutionPlan {
    pub scope: RepaintExecutionScope,
}
