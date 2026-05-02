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
