//! Runtime-visible retained artifact lifecycle state.

use std::fmt::Write;

/// Browser/runtime generation for retained render state.
///
/// This is not a frame counter, phase execution counter, cache proof, artifact
/// reuse proof, or stable layout/paint identity. It advances only when the
/// page-owned retained render state changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RenderEpoch(u64);

impl RenderEpoch {
    pub const fn initial() -> Self {
        Self(0)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(
            self.0
                .checked_add(1)
                .expect("retained render epoch exhausted"),
        )
    }
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

/// Runtime-visible retained identity policy for artifacts whose concrete IDs
/// remain frame-local in AC1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedRenderIdentityState {
    NoneFrameLocal,
}

/// Deterministic browser/runtime debug summary of retained render state.
///
/// This snapshot reports retained runtime metadata and artifact lifetime
/// policy. It deliberately does not expose frame-local layout, paint,
/// traversal, or stacking IDs as retained identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedRenderStateDebugSnapshot {
    pub render_epoch: RenderEpoch,
    pub has_dom: bool,
    pub resolved_styles: RenderArtifactState,
    pub computed_styles: RenderArtifactState,
    pub styled_tree: RenderArtifactState,
    pub layout_tree: RenderArtifactState,
    pub paint_output: RenderArtifactState,
    pub style_dirty: bool,
    pub layout_dirty_placeholder: bool,
    pub style_invalidation: StyleInvalidationState,
    pub layout_identity: RetainedRenderIdentityState,
    pub paint_identity: RetainedRenderIdentityState,
    pub stacking_identity: RetainedRenderIdentityState,
    pub traversal_identity: RetainedRenderIdentityState,
}

impl RetainedRenderStateDebugSnapshot {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write retained render state snapshot");
        writeln!(&mut out, "retained-render-state").expect("write retained render state snapshot");
        writeln!(&mut out, "render-epoch: {}", self.render_epoch.value())
            .expect("write retained render state snapshot");
        writeln!(&mut out, "has-dom: {}", self.has_dom)
            .expect("write retained render state snapshot");
        writeln!(&mut out, "artifacts:").expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  resolved-styles: {}",
            render_artifact_state_debug_label(self.resolved_styles)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  computed-styles: {}",
            render_artifact_state_debug_label(self.computed_styles)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  styled-tree: {}",
            render_artifact_state_debug_label(self.styled_tree)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  layout-tree: {}",
            render_artifact_state_debug_label(self.layout_tree)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  paint-output: {}",
            render_artifact_state_debug_label(self.paint_output)
        )
        .expect("write retained render state snapshot");
        writeln!(&mut out, "dirty-state:").expect("write retained render state snapshot");
        writeln!(&mut out, "  style-dirty: {}", self.style_dirty)
            .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  layout-dirty-placeholder: {}",
            self.layout_dirty_placeholder
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  style-invalidation: {}",
            style_invalidation_state_debug_label(self.style_invalidation)
        )
        .expect("write retained render state snapshot");
        writeln!(&mut out, "retained-identities:").expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  layout: {}",
            retained_identity_state_debug_label(self.layout_identity)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  paint: {}",
            retained_identity_state_debug_label(self.paint_identity)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  stacking: {}",
            retained_identity_state_debug_label(self.stacking_identity)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  traversal: {}",
            retained_identity_state_debug_label(self.traversal_identity)
        )
        .expect("write retained render state snapshot");
        out
    }
}

fn render_artifact_state_debug_label(state: RenderArtifactState) -> &'static str {
    match state {
        RenderArtifactState::Absent => "absent",
        RenderArtifactState::RetainedFresh => "retained-fresh",
        RenderArtifactState::RetainedStale => "retained-stale",
        RenderArtifactState::BorrowBackedRebuiltOnDemand => "borrow-backed-rebuilt-on-demand",
        RenderArtifactState::FrameLocalRebuiltPerFrame => "frame-local-rebuilt-per-frame",
        RenderArtifactState::ImmediateFrameOutput => "immediate-frame-output",
    }
}

fn style_invalidation_state_debug_label(state: StyleInvalidationState) -> &'static str {
    match state {
        StyleInvalidationState::None => "none",
        StyleInvalidationState::Full => "full",
        StyleInvalidationState::AttributeSuffix => "attribute-suffix",
    }
}

fn retained_identity_state_debug_label(state: RetainedRenderIdentityState) -> &'static str {
    match state {
        RetainedRenderIdentityState::NoneFrameLocal => "none-frame-local",
    }
}
