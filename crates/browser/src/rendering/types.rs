//! Core rendering pipeline vocabulary.

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
