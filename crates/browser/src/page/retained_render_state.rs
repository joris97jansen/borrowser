use crate::document_style::DocumentStyleSet;
use crate::rendering::{
    DirtyPhase, DirtyStateDebugSnapshot, FrameLocalIdentityState, RenderArtifactState,
    RenderDirtyState, RenderEpoch, RenderInvalidationEntryPoint, RenderPipelineDebugSnapshot,
    RetainedRenderIdentityMap, RetainedRenderStateDebugSnapshot, RetainedStyleArtifactAction,
    RetainedStyleArtifactDebugSnapshot, RetainedStyleArtifactKey, RetainedStyleArtifactState,
    RetainedStyleArtifactStats, StyleInvalidationState, dirty_request_for_entry_point,
};
use css::ComputedStyleReuseStats;
use html::Node;

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
    pub(super) dirty_state: RenderDirtyState,
    pub(super) last_restyle_trigger: Option<RestyleTrigger>,
    pub(super) pending_style_invalidation: Option<StyleInvalidationScope>,
    pub(super) last_style_recalc: Option<StyleRecalcKind>,
    pub(super) last_style_reuse: Option<ComputedStyleReuseStats>,
    pub(super) style_artifact_stats: RetainedStyleArtifactStats,
    pub(super) last_style_artifact_action: RetainedStyleArtifactAction,
    pub(super) identities: RetainedRenderIdentityMap,
}

impl RetainedRenderState {
    pub(super) fn new() -> Self {
        Self {
            render_epoch: RenderEpoch::initial(),
            document_styles: DocumentStyleSet::default(),
            generations: PageStyleGenerations::default(),
            style_cache: None,
            dirty_state: RenderDirtyState::document_initial(),
            last_restyle_trigger: None,
            pending_style_invalidation: Some(StyleInvalidationScope::Full),
            last_style_recalc: None,
            last_style_reuse: None,
            style_artifact_stats: RetainedStyleArtifactStats::default(),
            last_style_artifact_action: RetainedStyleArtifactAction::None,
            identities: RetainedRenderIdentityMap::new(),
        }
    }

    pub(super) fn reset_for_navigation(&mut self) {
        self.render_epoch = RenderEpoch::initial();
        self.document_styles.clear();
        self.generations = PageStyleGenerations::default();
        self.style_cache = None;
        self.dirty_state = RenderDirtyState::document_initial();
        self.last_restyle_trigger = None;
        self.pending_style_invalidation = Some(StyleInvalidationScope::Full);
        self.last_style_recalc = None;
        self.last_style_reuse = None;
        self.style_artifact_stats = RetainedStyleArtifactStats::default();
        self.last_style_artifact_action = RetainedStyleArtifactAction::None;
        self.identities.reset_for_navigation();
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

    pub(super) fn style_dirty(&self) -> bool {
        self.dirty_state.is_phase_dirty(DirtyPhase::Style)
    }

    pub(super) fn layout_dirty(&self) -> bool {
        self.dirty_state.is_phase_dirty(DirtyPhase::Layout)
    }

    pub(super) fn paint_dirty(&self) -> bool {
        self.dirty_state.is_phase_dirty(DirtyPhase::Paint)
    }

    pub(super) fn dirty_state(&self) -> &RenderDirtyState {
        &self.dirty_state
    }

    pub(super) fn retained_style_artifact_state(&self) -> RetainedStyleArtifactState {
        match (&self.style_cache, self.style_dirty()) {
            (None, _) => RetainedStyleArtifactState::Absent,
            (Some(_), true) => RetainedStyleArtifactState::Stale,
            (Some(_), false) => RetainedStyleArtifactState::Fresh,
        }
    }

    pub(super) fn current_style_artifact_key(&self) -> RetainedStyleArtifactKey {
        RetainedStyleArtifactKey {
            identity_domain: self.identities.domain(),
            style_input_generation: self.generations.style_inputs,
            stylesheet_generation: self.generations.stylesheets,
        }
    }

    pub(super) fn style_cache_matches_current_key(&self) -> bool {
        self.style_cache
            .as_ref()
            .is_some_and(|cache| cache.key == self.current_style_artifact_key())
    }

    pub(super) fn record_style_artifact_reuse(&mut self) {
        self.style_artifact_stats.reuse_count = self
            .style_artifact_stats
            .reuse_count
            .checked_add(1)
            .expect("retained style artifact reuse count exhausted");
        self.last_style_artifact_action = RetainedStyleArtifactAction::Reused;
    }

    pub(super) fn record_style_artifact_recompute(&mut self, action: RetainedStyleArtifactAction) {
        self.style_artifact_stats.recompute_count = self
            .style_artifact_stats
            .recompute_count
            .checked_add(1)
            .expect("retained style artifact recompute count exhausted");
        self.last_style_artifact_action = action;
    }

    fn record_style_artifact_discard_for_full_invalidation(&mut self) {
        if self.style_cache.is_none() {
            return;
        }

        self.style_artifact_stats.discard_count = self
            .style_artifact_stats
            .discard_count
            .checked_add(1)
            .expect("retained style artifact discard count exhausted");
        self.last_style_artifact_action = RetainedStyleArtifactAction::DiscardedForFullInvalidation;
    }

    pub(super) fn clear_style_dirty_after_recompute(&mut self) {
        self.dirty_state.clear_phase(DirtyPhase::Style);
    }

    #[cfg(test)]
    pub(super) fn clear_layout_dirty_for_tests(&mut self) {
        self.dirty_state.clear_phase(DirtyPhase::Layout);
    }

    #[cfg(test)]
    pub(super) fn clear_all_dirty_for_tests(&mut self) {
        self.dirty_state.clear();
    }

    pub(super) fn mark_dirty_for_entry_point(&mut self, entry_point: RenderInvalidationEntryPoint) {
        if matches!(entry_point, RenderInvalidationEntryPoint::DocumentReplaced) {
            self.dirty_state.clear();
        }
        let request = dirty_request_for_entry_point(entry_point);
        self.dirty_state.extend(request.entries);
    }

    pub(super) fn debug_snapshot(&self, has_dom: bool) -> RenderPipelineDebugSnapshot {
        let style_cache_state = match (&self.style_cache, self.style_dirty()) {
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
            dirty_state: DirtyStateDebugSnapshot {
                entries: self.dirty_state.entries().to_vec(),
            },
            style_dirty: self.style_dirty(),
            layout_dirty: self.layout_dirty(),
            paint_dirty: self.paint_dirty(),
            style_invalidation,
            style_artifacts: self.style_artifact_debug_snapshot(style_cache_state),
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
            dirty_state: pipeline.dirty_state,
            style_dirty: pipeline.style_dirty,
            layout_dirty: pipeline.layout_dirty,
            paint_dirty: pipeline.paint_dirty,
            style_invalidation: pipeline.style_invalidation,
            style_artifacts: pipeline.style_artifacts,
            retained_identity_domain: self.identities.domain(),
            retained_identities: self.identities.identities(),
            layout_identity: FrameLocalIdentityState::NotRetained,
            paint_identity: FrameLocalIdentityState::NotRetained,
            stacking_identity: FrameLocalIdentityState::NotRetained,
            traversal_source_order_identity: FrameLocalIdentityState::NotRetained,
        }
    }

    pub(super) fn reset_retained_identities_for_document_replacement(&mut self) {
        self.identities.reset_for_document_replacement();
    }

    pub(super) fn reconcile_retained_identities_from_dom(&mut self, dom: &Node) {
        self.identities.reconcile_live_dom(dom);
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
        self.mark_dirty_for_entry_point(RenderInvalidationEntryPoint::StylesheetSetChanged);
    }

    pub(super) fn invalidate_style(&mut self, scope: StyleInvalidationScope) {
        let merged = match self.pending_style_invalidation.take() {
            Some(existing) => existing.merge(scope),
            None => scope,
        };

        if matches!(merged, StyleInvalidationScope::Full) {
            self.record_style_artifact_discard_for_full_invalidation();
            self.style_cache = None;
        }
        self.pending_style_invalidation = Some(merged);
    }

    fn style_artifact_debug_snapshot(
        &self,
        state: RenderArtifactState,
    ) -> RetainedStyleArtifactDebugSnapshot {
        RetainedStyleArtifactDebugSnapshot {
            key: self.style_cache.as_ref().map(|cache| cache.key),
            state,
            last_action: self.last_style_artifact_action,
            stats: self.style_artifact_stats,
        }
    }
}

impl Default for RetainedRenderState {
    fn default() -> Self {
        Self::new()
    }
}
