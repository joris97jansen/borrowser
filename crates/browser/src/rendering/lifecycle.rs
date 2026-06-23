//! Runtime-visible retained artifact lifecycle state.

use std::fmt::Write;

use super::types::{DirtyEntry, DirtyPhase, DirtyScopeDebugLabel};
use super::{
    RetainedRenderIdentity, RetainedRenderIdentityDomain, retained_render_anchor_debug_label,
    retained_render_artifact_kind_debug_label,
};
use gfx::paint::PaintArtifact;
use layout::{RetainedLayoutKey, RetainedLayoutKeySeed};

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
pub struct RetainedStyleArtifactKey {
    pub identity_domain: RetainedRenderIdentityDomain,
    pub style_input_generation: u64,
    pub stylesheet_generation: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetainedStyleArtifactStats {
    pub reuse_count: u64,
    pub recompute_count: u64,
    pub discard_count: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RetainedStyleArtifactAction {
    #[default]
    None,
    InitialCompute,
    Reused,
    FullRecompute,
    IncrementalSuffixRecompute,
    DiscardedForFullInvalidation,
    FallbackFullRecompute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedStyleArtifactDebugSnapshot {
    pub key: Option<RetainedStyleArtifactKey>,
    pub state: RenderArtifactState,
    pub last_action: RetainedStyleArtifactAction,
    pub stats: RetainedStyleArtifactStats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetainedLayoutArtifactStats {
    pub reuse_count: u64,
    pub recompute_count: u64,
    pub discard_count: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RetainedLayoutArtifactAction {
    #[default]
    None,
    InitialCompute,
    Reused,
    FullDocumentRelayout,
    ConservativeDocumentFallback,
    DiscardedForInvalidation,
    MaterializationFailedFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedLayoutArtifactDebugSnapshot {
    pub key_seed: RetainedLayoutKeySeed,
    pub key: Option<RetainedLayoutKey>,
    pub state: RenderArtifactState,
    pub last_action: RetainedLayoutArtifactAction,
    pub stats: RetainedLayoutArtifactStats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedPaintArtifactKey {
    pub identity_domain: RetainedRenderIdentityDomain,
    pub layout_key: RetainedLayoutKey,
    pub paint_style_generation: u64,
    pub paint_input_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedPaintArtifactKeySeed {
    pub identity_domain: RetainedRenderIdentityDomain,
    pub paint_style_generation: u64,
    pub paint_input_generation: u64,
}

impl RetainedPaintArtifactKeySeed {
    pub fn for_layout_key(self, layout_key: RetainedLayoutKey) -> RetainedPaintArtifactKey {
        RetainedPaintArtifactKey {
            identity_domain: self.identity_domain,
            layout_key,
            paint_style_generation: self.paint_style_generation,
            paint_input_generation: self.paint_input_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetainedPaintArtifactStats {
    pub reuse_count: u64,
    pub recompute_count: u64,
    pub discard_count: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RetainedPaintArtifactAction {
    #[default]
    None,
    InitialCompute,
    Reused,
    Recomputed,
    DiscardedForInvalidation,
    ConservativeDocumentFallback,
    ConservativeViewportFallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedPaintArtifactDebugSnapshot {
    pub key: Option<RetainedPaintArtifactKey>,
    pub state: RenderArtifactState,
    pub last_action: RetainedPaintArtifactAction,
    pub stats: RetainedPaintArtifactStats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetainedPaintFrameAction {
    Reused,
    Recomputed,
    ConservativeDocumentFallback,
    ConservativeViewportFallback,
}

#[derive(Clone, Debug)]
pub struct RetainedPaintFrameResult {
    pub key: RetainedPaintArtifactKey,
    pub action: RetainedPaintFrameAction,
    pub artifact: PaintArtifact,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetainedRenderGenerationDebugSnapshot {
    pub dom_generation: u64,
    pub style_input_generation: u64,
    pub stylesheet_generation: u64,
    pub layout_input_generation: u64,
    pub layout_style_generation: u64,
    pub paint_style_generation: u64,
    pub paint_input_generation: u64,
    pub text_measurement_generation: u64,
    pub replaced_metadata_generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderPipelineDebugSnapshot {
    pub has_dom: bool,
    pub resolved_styles: RenderArtifactState,
    pub computed_styles: RenderArtifactState,
    pub styled_tree: RenderArtifactState,
    pub layout_tree: RenderArtifactState,
    pub paint_output: RenderArtifactState,
    pub dirty_state: DirtyStateDebugSnapshot,
    pub style_dirty: bool,
    pub layout_dirty: bool,
    pub paint_dirty: bool,
    pub style_invalidation: StyleInvalidationState,
    pub generations: RetainedRenderGenerationDebugSnapshot,
    pub style_artifacts: RetainedStyleArtifactDebugSnapshot,
    pub layout_artifacts: RetainedLayoutArtifactDebugSnapshot,
    pub paint_artifacts: RetainedPaintArtifactDebugSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirtyStateDebugSnapshot {
    pub entries: Vec<DirtyEntry>,
}

impl DirtyStateDebugSnapshot {
    pub fn is_phase_dirty(&self, phase: DirtyPhase) -> bool {
        self.entries.iter().any(|entry| entry.phase == phase)
    }
}

/// Runtime-visible policy for identity domains that remain frame-local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameLocalIdentityState {
    NotRetained,
}

/// Deterministic browser/runtime debug summary of retained render state.
///
/// This snapshot reports retained runtime metadata and artifact lifetime
/// policy. It deliberately does not expose frame-local layout, paint,
/// traversal, or stacking IDs as retained identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedRenderStateDebugSnapshot {
    pub render_epoch: RenderEpoch,
    pub has_dom: bool,
    pub resolved_styles: RenderArtifactState,
    pub computed_styles: RenderArtifactState,
    pub styled_tree: RenderArtifactState,
    pub layout_tree: RenderArtifactState,
    pub paint_output: RenderArtifactState,
    pub dirty_state: DirtyStateDebugSnapshot,
    pub style_dirty: bool,
    pub layout_dirty: bool,
    pub paint_dirty: bool,
    pub style_invalidation: StyleInvalidationState,
    pub generations: RetainedRenderGenerationDebugSnapshot,
    pub style_artifacts: RetainedStyleArtifactDebugSnapshot,
    pub layout_artifacts: RetainedLayoutArtifactDebugSnapshot,
    pub paint_artifacts: RetainedPaintArtifactDebugSnapshot,
    pub retained_identity_domain: RetainedRenderIdentityDomain,
    pub retained_identities: Vec<RetainedRenderIdentity>,
    pub layout_identity: FrameLocalIdentityState,
    pub paint_identity: FrameLocalIdentityState,
    pub stacking_identity: FrameLocalIdentityState,
    pub traversal_source_order_identity: FrameLocalIdentityState,
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
        append_dirty_state_debug_snapshot(&mut out, &self.dirty_state);
        writeln!(&mut out, "  style-dirty: {}", self.style_dirty)
            .expect("write retained render state snapshot");
        writeln!(&mut out, "  layout-dirty: {}", self.layout_dirty)
            .expect("write retained render state snapshot");
        writeln!(&mut out, "  paint-dirty: {}", self.paint_dirty)
            .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  style-invalidation: {}",
            style_invalidation_state_debug_label(self.style_invalidation)
        )
        .expect("write retained render state snapshot");
        append_generation_debug_snapshot(&mut out, self.generations);
        append_retained_style_artifact_debug_snapshot(&mut out, self.style_artifacts);
        append_retained_layout_artifact_debug_snapshot(&mut out, self.layout_artifacts);
        append_retained_paint_artifact_debug_snapshot(&mut out, self.paint_artifacts);
        writeln!(&mut out, "retained-identities:").expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  identity-domain: {}",
            self.retained_identity_domain.value()
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  render-artifacts: {}",
            self.retained_identities.len()
        )
        .expect("write retained render state snapshot");
        for identity in &self.retained_identities {
            writeln!(
                &mut out,
                "    - retained-render-id={} kind={} anchor={}",
                identity.id.value(),
                retained_render_artifact_kind_debug_label(identity.kind),
                retained_render_anchor_debug_label(identity.anchor)
            )
            .expect("write retained render state snapshot");
        }
        writeln!(
            &mut out,
            "  frame-local-layout-ids: {}",
            frame_local_identity_state_debug_label(self.layout_identity)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  frame-local-paint-ids: {}",
            frame_local_identity_state_debug_label(self.paint_identity)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  frame-local-stacking-ids: {}",
            frame_local_identity_state_debug_label(self.stacking_identity)
        )
        .expect("write retained render state snapshot");
        writeln!(
            &mut out,
            "  frame-local-traversal-source-order-ids: {}",
            frame_local_identity_state_debug_label(self.traversal_source_order_identity)
        )
        .expect("write retained render state snapshot");
        out
    }
}

fn append_generation_debug_snapshot(
    out: &mut String,
    snapshot: RetainedRenderGenerationDebugSnapshot,
) {
    writeln!(out, "generations:").expect("write retained render state snapshot");
    writeln!(out, "  dom-generation: {}", snapshot.dom_generation)
        .expect("write retained render state snapshot");
    writeln!(
        out,
        "  style-input-generation: {}",
        snapshot.style_input_generation
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  stylesheet-generation: {}",
        snapshot.stylesheet_generation
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  layout-input-generation: {}",
        snapshot.layout_input_generation
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  layout-style-generation: {}",
        snapshot.layout_style_generation
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  paint-style-generation: {}",
        snapshot.paint_style_generation
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  paint-input-generation: {}",
        snapshot.paint_input_generation
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  text-measurement-generation: {}",
        snapshot.text_measurement_generation
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  replaced-metadata-generation: {}",
        snapshot.replaced_metadata_generation
    )
    .expect("write retained render state snapshot");
}

fn append_retained_paint_artifact_debug_snapshot(
    out: &mut String,
    snapshot: RetainedPaintArtifactDebugSnapshot,
) {
    writeln!(out, "paint-artifacts:").expect("write retained render state snapshot");
    match snapshot.key {
        Some(key) => writeln!(
            out,
            "  key: identity-domain={} layout-key=(identity-domain={} layout-input-generation={} layout-style-generation={} viewport-width-key={} text-measurement-generation={} replaced-metadata-generation={}) paint-style-generation={} paint-input-generation={}",
            key.identity_domain.value(),
            key.layout_key.identity_domain,
            key.layout_key.layout_input_generation,
            key.layout_key.layout_style_generation,
            key.layout_key.viewport_width.value(),
            key.layout_key.text_measurement_generation,
            key.layout_key.replaced_metadata_generation,
            key.paint_style_generation,
            key.paint_input_generation
        ),
        None => writeln!(out, "  key: none"),
    }
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  state: {}",
        render_artifact_state_debug_label(snapshot.state)
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  last-action: {}",
        retained_paint_artifact_action_debug_label(snapshot.last_action)
    )
    .expect("write retained render state snapshot");
    writeln!(out, "  reuse-count: {}", snapshot.stats.reuse_count)
        .expect("write retained render state snapshot");
    writeln!(out, "  recompute-count: {}", snapshot.stats.recompute_count)
        .expect("write retained render state snapshot");
    writeln!(out, "  discard-count: {}", snapshot.stats.discard_count)
        .expect("write retained render state snapshot");
}

fn append_retained_layout_artifact_debug_snapshot(
    out: &mut String,
    snapshot: RetainedLayoutArtifactDebugSnapshot,
) {
    writeln!(out, "layout-artifacts:").expect("write retained render state snapshot");
    writeln!(
        out,
        "  key-seed: identity-domain={} layout-input-generation={} layout-style-generation={} text-measurement-generation={} replaced-metadata-generation={}",
        snapshot.key_seed.identity_domain,
        snapshot.key_seed.layout_input_generation,
        snapshot.key_seed.layout_style_generation,
        snapshot.key_seed.text_measurement_generation,
        snapshot.key_seed.replaced_metadata_generation,
    )
    .expect("write retained render state snapshot");
    match snapshot.key {
        Some(key) => writeln!(
            out,
            "  key: identity-domain={} layout-input-generation={} layout-style-generation={} viewport-width-key={} text-measurement-generation={} replaced-metadata-generation={}",
            key.identity_domain,
            key.layout_input_generation,
            key.layout_style_generation,
            key.viewport_width.value(),
            key.text_measurement_generation,
            key.replaced_metadata_generation
        ),
        None => writeln!(out, "  key: none"),
    }
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  state: {}",
        render_artifact_state_debug_label(snapshot.state)
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  last-action: {}",
        retained_layout_artifact_action_debug_label(snapshot.last_action)
    )
    .expect("write retained render state snapshot");
    writeln!(out, "  reuse-count: {}", snapshot.stats.reuse_count)
        .expect("write retained render state snapshot");
    writeln!(out, "  recompute-count: {}", snapshot.stats.recompute_count)
        .expect("write retained render state snapshot");
    writeln!(out, "  discard-count: {}", snapshot.stats.discard_count)
        .expect("write retained render state snapshot");
}

fn append_retained_style_artifact_debug_snapshot(
    out: &mut String,
    snapshot: RetainedStyleArtifactDebugSnapshot,
) {
    writeln!(out, "style-artifacts:").expect("write retained render state snapshot");
    match snapshot.key {
        Some(key) => writeln!(
            out,
            "  key: identity-domain={} style-input-generation={} stylesheet-generation={}",
            key.identity_domain.value(),
            key.style_input_generation,
            key.stylesheet_generation
        ),
        None => writeln!(out, "  key: none"),
    }
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  state: {}",
        render_artifact_state_debug_label(snapshot.state)
    )
    .expect("write retained render state snapshot");
    writeln!(
        out,
        "  last-action: {}",
        retained_style_artifact_action_debug_label(snapshot.last_action)
    )
    .expect("write retained render state snapshot");
    writeln!(out, "  reuse-count: {}", snapshot.stats.reuse_count)
        .expect("write retained render state snapshot");
    writeln!(out, "  recompute-count: {}", snapshot.stats.recompute_count)
        .expect("write retained render state snapshot");
    writeln!(out, "  discard-count: {}", snapshot.stats.discard_count)
        .expect("write retained render state snapshot");
}

fn append_dirty_state_debug_snapshot(out: &mut String, snapshot: &DirtyStateDebugSnapshot) {
    writeln!(out, "dirty-state:").expect("write retained render state snapshot");
    writeln!(out, "  entries: {}", snapshot.entries.len())
        .expect("write retained render state snapshot");
    for (index, entry) in snapshot.entries.iter().enumerate() {
        writeln!(
            out,
            "    entry[{index}]: phase={} reason={} scope={}",
            entry.phase.debug_label(),
            entry.reason.debug_label(),
            dirty_scope_debug_label(entry.scope.debug_label())
        )
        .expect("write retained render state snapshot");
    }
}

fn dirty_scope_debug_label(label: DirtyScopeDebugLabel) -> String {
    match label {
        DirtyScopeDebugLabel::Static(label) => label.to_string(),
        DirtyScopeDebugLabel::RetainedId { prefix, id } => {
            format!("{prefix}(retained-render-id={})", id.value())
        }
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

fn retained_style_artifact_action_debug_label(action: RetainedStyleArtifactAction) -> &'static str {
    match action {
        RetainedStyleArtifactAction::None => "none",
        RetainedStyleArtifactAction::InitialCompute => "initial-compute",
        RetainedStyleArtifactAction::Reused => "reused",
        RetainedStyleArtifactAction::FullRecompute => "full-recompute",
        RetainedStyleArtifactAction::IncrementalSuffixRecompute => "incremental-suffix-recompute",
        RetainedStyleArtifactAction::DiscardedForFullInvalidation => {
            "discarded-for-full-invalidation"
        }
        RetainedStyleArtifactAction::FallbackFullRecompute => "fallback-full-recompute",
    }
}

fn retained_layout_artifact_action_debug_label(
    action: RetainedLayoutArtifactAction,
) -> &'static str {
    match action {
        RetainedLayoutArtifactAction::None => "none",
        RetainedLayoutArtifactAction::InitialCompute => "initial-compute",
        RetainedLayoutArtifactAction::Reused => "reused",
        RetainedLayoutArtifactAction::FullDocumentRelayout => "full-document-relayout",
        RetainedLayoutArtifactAction::ConservativeDocumentFallback => {
            "conservative-document-fallback"
        }
        RetainedLayoutArtifactAction::DiscardedForInvalidation => "discarded-for-invalidation",
        RetainedLayoutArtifactAction::MaterializationFailedFallback => {
            "materialization-failed-fallback"
        }
    }
}

fn retained_paint_artifact_action_debug_label(action: RetainedPaintArtifactAction) -> &'static str {
    match action {
        RetainedPaintArtifactAction::None => "none",
        RetainedPaintArtifactAction::InitialCompute => "initial-compute",
        RetainedPaintArtifactAction::Reused => "reused",
        RetainedPaintArtifactAction::Recomputed => "recomputed",
        RetainedPaintArtifactAction::DiscardedForInvalidation => "discarded-for-invalidation",
        RetainedPaintArtifactAction::ConservativeDocumentFallback => {
            "conservative-document-fallback"
        }
        RetainedPaintArtifactAction::ConservativeViewportFallback => {
            "conservative-viewport-fallback"
        }
    }
}

fn frame_local_identity_state_debug_label(state: FrameLocalIdentityState) -> &'static str {
    match state {
        FrameLocalIdentityState::NotRetained => "not-retained",
    }
}
