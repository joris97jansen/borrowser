use crate::document_style::DocumentStyleSet;
use crate::rendering::{
    RenderArtifactState, RenderEpoch, RenderPipelineDebugSnapshot, RetainedRenderIdentityState,
    RetainedRenderStateDebugSnapshot, StyleInvalidationState,
};
use css::ComputedStyleReuseStats;

use super::restyle::{RestyleTrigger, StyleInvalidationScope};
use super::style_cache::{PageStyleCache, PageStyleGenerations, StyleRecalcKind};

/// Retained rendering state owned by `PageState`.
///
/// This groups the page-local rendering artifacts and invalidation metadata
/// that survive across updates. Borrow-backed style trees, layout trees, and
/// paint output remain outside this struct by contract.
#[derive(Clone, Debug)]
pub(super) struct RetainedRenderState {
    pub(super) render_epoch: RenderEpoch,
    pub(super) document_styles: DocumentStyleSet,
    pub(super) generations: PageStyleGenerations,
    pub(super) style_cache: Option<PageStyleCache>,
    pub(super) style_dirty: bool,
    pub(super) layout_dirty: bool,
    pub(super) last_restyle_trigger: Option<RestyleTrigger>,
    pub(super) pending_style_invalidation: Option<StyleInvalidationScope>,
    pub(super) last_style_recalc: Option<StyleRecalcKind>,
    pub(super) last_style_reuse: Option<ComputedStyleReuseStats>,
}

impl RetainedRenderState {
    pub(super) fn new() -> Self {
        Self {
            render_epoch: RenderEpoch::initial(),
            document_styles: DocumentStyleSet::default(),
            generations: PageStyleGenerations::default(),
            style_cache: None,
            style_dirty: true,
            layout_dirty: true,
            last_restyle_trigger: None,
            pending_style_invalidation: Some(StyleInvalidationScope::Full),
            last_style_recalc: None,
            last_style_reuse: None,
        }
    }

    pub(super) fn reset_for_navigation(&mut self) {
        self.render_epoch = RenderEpoch::initial();
        self.document_styles.clear();
        self.generations = PageStyleGenerations::default();
        self.style_cache = None;
        self.style_dirty = true;
        self.layout_dirty = true;
        self.last_restyle_trigger = None;
        self.pending_style_invalidation = Some(StyleInvalidationScope::Full);
        self.last_style_recalc = None;
        self.last_style_reuse = None;
    }

    pub(super) fn advance_render_epoch(&mut self) {
        self.render_epoch = self.render_epoch.next();
    }

    pub(super) fn mark_dom_generation_changed(&mut self) {
        self.generations.dom = self
            .generations
            .dom
            .checked_add(1)
            .expect("page DOM generation exhausted");
        self.advance_render_epoch();
    }

    pub(super) fn take_style_invalidation_for_recompute(
        &mut self,
    ) -> Option<StyleInvalidationScope> {
        let pending = self.pending_style_invalidation.take();
        if pending.is_some() {
            self.advance_render_epoch();
        }
        pending
    }

    pub(super) fn debug_snapshot(&self, has_dom: bool) -> RenderPipelineDebugSnapshot {
        let style_cache_state = match (&self.style_cache, self.style_dirty) {
            (None, _) => RenderArtifactState::Absent,
            (Some(_), true) => RenderArtifactState::RetainedStale,
            (Some(_), false) => RenderArtifactState::RetainedFresh,
        };

        let (styled_tree, layout_tree, paint_output) = if has_dom {
            (
                RenderArtifactState::BorrowBackedRebuiltOnDemand,
                RenderArtifactState::FrameLocalRebuiltPerFrame,
                RenderArtifactState::ImmediateFrameOutput,
            )
        } else {
            (
                RenderArtifactState::Absent,
                RenderArtifactState::Absent,
                RenderArtifactState::Absent,
            )
        };

        let style_invalidation = match self.pending_style_invalidation {
            Some(StyleInvalidationScope::Full) => StyleInvalidationState::Full,
            Some(StyleInvalidationScope::AttributeSuffix { .. }) => {
                StyleInvalidationState::AttributeSuffix
            }
            None => StyleInvalidationState::None,
        };

        RenderPipelineDebugSnapshot {
            has_dom,
            resolved_styles: style_cache_state,
            computed_styles: style_cache_state,
            styled_tree,
            layout_tree,
            paint_output,
            style_dirty: self.style_dirty,
            layout_dirty: self.layout_dirty,
            style_invalidation,
        }
    }

    pub(super) fn retained_debug_snapshot(
        &self,
        has_dom: bool,
    ) -> RetainedRenderStateDebugSnapshot {
        let pipeline = self.debug_snapshot(has_dom);
        RetainedRenderStateDebugSnapshot {
            render_epoch: self.render_epoch,
            has_dom: pipeline.has_dom,
            resolved_styles: pipeline.resolved_styles,
            computed_styles: pipeline.computed_styles,
            styled_tree: pipeline.styled_tree,
            layout_tree: pipeline.layout_tree,
            paint_output: pipeline.paint_output,
            style_dirty: pipeline.style_dirty,
            layout_dirty_placeholder: pipeline.layout_dirty,
            style_invalidation: pipeline.style_invalidation,
            layout_identity: RetainedRenderIdentityState::NoneFrameLocal,
            paint_identity: RetainedRenderIdentityState::NoneFrameLocal,
            stacking_identity: RetainedRenderIdentityState::NoneFrameLocal,
            traversal_identity: RetainedRenderIdentityState::NoneFrameLocal,
        }
    }

    pub(super) fn mark_style_inputs_changed(&mut self, scope: StyleInvalidationScope) {
        self.generations.style_inputs = self
            .generations
            .style_inputs
            .checked_add(1)
            .expect("page style-input generation exhausted");
        self.invalidate_style(scope);
    }

    pub(super) fn mark_stylesheets_changed(&mut self) {
        self.generations.stylesheets = self
            .generations
            .stylesheets
            .checked_add(1)
            .expect("page stylesheet generation exhausted");
        self.advance_render_epoch();
        self.invalidate_style(StyleInvalidationScope::Full);
    }

    pub(super) fn invalidate_style(&mut self, scope: StyleInvalidationScope) {
        self.style_dirty = true;
        self.layout_dirty = true;

        let merged = match self.pending_style_invalidation.take() {
            Some(existing) => existing.merge(scope),
            None => scope,
        };

        if matches!(merged, StyleInvalidationScope::Full) {
            self.style_cache = None;
        }
        self.pending_style_invalidation = Some(merged);
    }
}

impl Default for RetainedRenderState {
    fn default() -> Self {
        Self::new()
    }
}
