use crate::document_style::DocumentStyleSet;
use crate::rendering::{
    DirtyEntry, DirtyPhase, DirtyReason, DirtyScope, DirtyStateDebugSnapshot,
    FrameLocalIdentityState, RenderArtifactState, RenderDirtyState, RenderEpoch,
    RenderInvalidationEntryPoint, RenderPipelineDebugSnapshot, RetainedLayoutArtifactAction,
    RetainedLayoutArtifactDebugSnapshot, RetainedLayoutArtifactState, RetainedLayoutArtifactStats,
    RetainedRenderIdentityMap, RetainedRenderStateDebugSnapshot, RetainedStyleArtifactAction,
    RetainedStyleArtifactDebugSnapshot, RetainedStyleArtifactKey, RetainedStyleArtifactState,
    RetainedStyleArtifactStats, StyleInvalidationState, dirty_request_for_entry_point,
};
use css::{ComputedDocumentStyleLayoutImpact, ComputedStyleReuseStats};
use html::Node;
use layout::{
    RetainedLayoutArtifact, RetainedLayoutFallbackReason, RetainedLayoutFrameAction,
    RetainedLayoutFrameResult, RetainedLayoutKeySeed,
};

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
    pub(super) layout_cache: Option<RetainedLayoutArtifact>,
    pub(super) layout_artifact_stats: RetainedLayoutArtifactStats,
    pub(super) last_layout_artifact_action: RetainedLayoutArtifactAction,
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
            layout_cache: None,
            layout_artifact_stats: RetainedLayoutArtifactStats::default(),
            last_layout_artifact_action: RetainedLayoutArtifactAction::None,
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
        self.layout_cache = None;
        self.layout_artifact_stats = RetainedLayoutArtifactStats::default();
        self.last_layout_artifact_action = RetainedLayoutArtifactAction::None;
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

    pub(super) fn retained_layout_artifact_state(&self) -> RetainedLayoutArtifactState {
        match (&self.layout_cache, self.layout_dirty()) {
            (None, _) => RetainedLayoutArtifactState::Absent,
            (Some(_), true) => RetainedLayoutArtifactState::Stale,
            (Some(_), false) => RetainedLayoutArtifactState::Fresh,
        }
    }

    pub(super) fn retained_layout_key_seed(&self) -> RetainedLayoutKeySeed {
        RetainedLayoutKeySeed {
            identity_domain: self.identities.domain().value(),
            layout_input_generation: self.generations.layout_inputs,
            layout_style_generation: self.generations.layout_style,
            text_measurement_generation: self.generations.text_measurement,
            replaced_metadata_generation: self.generations.replaced_metadata,
        }
    }

    pub(super) fn retained_layout_artifact(&self) -> Option<&RetainedLayoutArtifact> {
        self.layout_cache.as_ref()
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

    pub(super) fn record_computed_style_layout_impact(
        &mut self,
        impact: ComputedDocumentStyleLayoutImpact,
    ) {
        match impact {
            ComputedDocumentStyleLayoutImpact::PaintOnly => {
                self.dirty_state
                    .remove_phase_reason(DirtyPhase::Layout, DirtyReason::CascadedFromStyle);
                if !self.layout_dirty() {
                    self.dirty_state
                        .remove_phase_reason(DirtyPhase::Paint, DirtyReason::CascadedFromLayout);
                }
                self.dirty_state.push(DirtyEntry::new(
                    DirtyPhase::Paint,
                    DirtyReason::PaintOnlyStyleChanged,
                    DirtyScope::Document,
                ));
            }
            ComputedDocumentStyleLayoutImpact::LayoutAffecting
            | ComputedDocumentStyleLayoutImpact::Unknown => {
                self.generations.layout_style = self
                    .generations
                    .layout_style
                    .checked_add(1)
                    .expect("layout style generation exhausted");
                self.dirty_state.push(DirtyEntry::new(
                    DirtyPhase::Layout,
                    DirtyReason::LayoutAffectingStyleChanged,
                    DirtyScope::Document,
                ));
                self.dirty_state.push(DirtyEntry::new(
                    DirtyPhase::Paint,
                    DirtyReason::CascadedFromLayout,
                    DirtyScope::Document,
                ));
            }
        }
    }

    pub(super) fn record_layout_frame_result(&mut self, result: RetainedLayoutFrameResult) {
        match result.action {
            RetainedLayoutFrameAction::Reused => {
                self.layout_artifact_stats.reuse_count = self
                    .layout_artifact_stats
                    .reuse_count
                    .checked_add(1)
                    .expect("retained layout artifact reuse count exhausted");
                self.last_layout_artifact_action = RetainedLayoutArtifactAction::Reused;
            }
            RetainedLayoutFrameAction::Recomputed => {
                self.layout_artifact_stats.recompute_count = self
                    .layout_artifact_stats
                    .recompute_count
                    .checked_add(1)
                    .expect("retained layout artifact recompute count exhausted");
                self.last_layout_artifact_action = if self.layout_cache.is_none()
                    && self.layout_artifact_stats.recompute_count == 1
                {
                    RetainedLayoutArtifactAction::InitialCompute
                } else {
                    RetainedLayoutArtifactAction::FullDocumentRelayout
                };
                self.layout_cache = Some(result.artifact);
            }
            RetainedLayoutFrameAction::ConservativeFallback(reason) => {
                self.layout_artifact_stats.recompute_count = self
                    .layout_artifact_stats
                    .recompute_count
                    .checked_add(1)
                    .expect("retained layout artifact recompute count exhausted");
                self.last_layout_artifact_action = match reason {
                    RetainedLayoutFallbackReason::MaterializationFailed => {
                        RetainedLayoutArtifactAction::MaterializationFailedFallback
                    }
                    _ => RetainedLayoutArtifactAction::ConservativeDocumentFallback,
                };
                self.layout_cache = Some(result.artifact);
            }
        }
        self.dirty_state.clear_phase(DirtyPhase::Layout);
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
            self.discard_layout_for_full_invalidation();
        }
        match entry_point {
            RenderInvalidationEntryPoint::DocumentReplaced
            | RenderInvalidationEntryPoint::DomStructureChanged
            | RenderInvalidationEntryPoint::DomTextChanged => {
                self.generations.layout_inputs = self
                    .generations
                    .layout_inputs
                    .checked_add(1)
                    .expect("layout input generation exhausted");
            }
            RenderInvalidationEntryPoint::ResourceStateChanged => {
                self.generations.replaced_metadata = self
                    .generations
                    .replaced_metadata
                    .checked_add(1)
                    .expect("replaced metadata generation exhausted");
            }
            RenderInvalidationEntryPoint::ViewportChanged
            | RenderInvalidationEntryPoint::DomAttributesChanged
            | RenderInvalidationEntryPoint::StylesheetSetChanged
            | RenderInvalidationEntryPoint::InputStateChanged => {}
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
                match (&self.layout_cache, self.layout_dirty()) {
                    (None, _) => RenderArtifactState::Absent,
                    (Some(_), true) => RenderArtifactState::RetainedStale,
                    (Some(_), false) => RenderArtifactState::RetainedFresh,
                },
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
            layout_artifacts: self.layout_artifact_debug_snapshot(layout_tree),
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
            layout_artifacts: pipeline.layout_artifacts,
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

    pub(super) fn discard_layout_for_full_invalidation(&mut self) {
        if self.layout_cache.is_some() {
            self.layout_artifact_stats.discard_count = self
                .layout_artifact_stats
                .discard_count
                .checked_add(1)
                .expect("retained layout artifact discard count exhausted");
            self.last_layout_artifact_action =
                RetainedLayoutArtifactAction::DiscardedForInvalidation;
        }
        self.layout_cache = None;
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

    fn layout_artifact_debug_snapshot(
        &self,
        state: RenderArtifactState,
    ) -> RetainedLayoutArtifactDebugSnapshot {
        RetainedLayoutArtifactDebugSnapshot {
            key_seed: self.retained_layout_key_seed(),
            key: self.layout_cache.as_ref().map(|cache| cache.key()),
            state,
            last_action: self.last_layout_artifact_action,
            stats: self.layout_artifact_stats,
        }
    }
}

impl Default for RetainedRenderState {
    fn default() -> Self {
        Self::new()
    }
}
