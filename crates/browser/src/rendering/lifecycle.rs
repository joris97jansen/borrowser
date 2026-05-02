//! Runtime-visible retained artifact lifecycle state.

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
