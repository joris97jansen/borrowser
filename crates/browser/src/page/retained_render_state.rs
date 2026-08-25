use crate::document_style::DocumentStyleSet;
use crate::rendering::{
    DirtyEntry, DirtyPhase, DirtyReason, DirtyScope, DirtyStateDebugSnapshot,
    FrameLocalIdentityState, RenderArtifactState, RenderDirtyState, RenderEpoch,
    RenderInvalidationEntryPoint, RenderPipelineDebugSnapshot, RetainedLayoutArtifactAction,
    RetainedLayoutArtifactDebugSnapshot, RetainedLayoutArtifactState, RetainedLayoutArtifactStats,
    RetainedPaintArtifactAction, RetainedPaintArtifactDebugSnapshot, RetainedPaintArtifactKey,
    RetainedPaintArtifactKeySeed, RetainedPaintArtifactState, RetainedPaintArtifactStats,
    RetainedPaintFrameAction, RetainedPaintFrameResult, RetainedRenderGenerationDebugSnapshot,
    RetainedRenderIdentityMap, RetainedRenderStateDebugSnapshot, RetainedStyleArtifactAction,
    RetainedStyleArtifactDebugSnapshot, RetainedStyleArtifactKey, RetainedStyleArtifactState,
    RetainedStyleArtifactStats,
};
use css::{
    ComputedDocumentStyleInvalidationImpact, ComputedStyleReuseStats, StyleChangeFacts,
    StyleInvalidationDecision, StyleInvalidationPlan, classify_style_invalidation,
    merge_style_invalidation_plans,
};
use gfx::paint::PaintArtifact;
use html::Node;
use layout::{
    RetainedLayoutArtifact, RetainedLayoutFallbackReason, RetainedLayoutFrameAction,
    RetainedLayoutFrameResult, RetainedLayoutKeySeed,
};

use super::style_cache::{
    PageStyleCache, PageStyleDependencyCache, PageStyleDependencyKey, PageStyleGenerations,
    StyleRecalcKind,
};

/// One-shot proof that CSS classified and the retained-style owner applied a
/// non-empty invalidation plan. Only this module can construct it.
pub(crate) struct AppliedCssStyleInvalidation {
    _private: (),
}

fn initial_document_style_change() -> StyleChangeFacts {
    StyleChangeFacts::dom_publication(
        css::DomStyleChangeFacts::builder()
            .document_replaced()
            .build(),
    )
}

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
    pub(super) style_dependency_cache: Option<PageStyleDependencyCache>,
    pub(super) last_style_invalidation_decision: Option<String>,
    pub(super) dirty_state: RenderDirtyState,
    pub(super) last_dom_mutation_facts: Option<super::DomMutationFacts>,
    pub(super) pending_style_invalidation: Option<StyleInvalidationPlan>,
    pub(super) last_style_recalc: Option<StyleRecalcKind>,
    pub(super) last_style_reuse: Option<ComputedStyleReuseStats>,
    pub(super) style_artifact_stats: RetainedStyleArtifactStats,
    pub(super) last_style_artifact_action: RetainedStyleArtifactAction,
    pub(super) layout_cache: Option<RetainedLayoutArtifact>,
    pub(super) layout_artifact_stats: RetainedLayoutArtifactStats,
    pub(super) last_layout_artifact_action: RetainedLayoutArtifactAction,
    pub(super) paint_cache: Option<RetainedPaintArtifactEntry>,
    pub(super) paint_artifact_stats: RetainedPaintArtifactStats,
    pub(super) last_paint_artifact_action: RetainedPaintArtifactAction,
    pub(super) identities: RetainedRenderIdentityMap,
}

#[derive(Clone, Debug)]
pub(super) struct RetainedPaintArtifactEntry {
    pub(super) key: RetainedPaintArtifactKey,
    pub(super) artifact: PaintArtifact,
}

impl RetainedRenderState {
    pub(super) fn new() -> Self {
        Self {
            render_epoch: RenderEpoch::initial(),
            document_styles: DocumentStyleSet::default(),
            generations: PageStyleGenerations::default(),
            style_cache: None,
            style_dependency_cache: None,
            last_style_invalidation_decision: None,
            dirty_state: RenderDirtyState::document_initial(),
            last_dom_mutation_facts: None,
            pending_style_invalidation: Some(
                classify_style_invalidation(&initial_document_style_change())
                    .expect("document replacement must invalidate style"),
            ),
            last_style_recalc: None,
            last_style_reuse: None,
            style_artifact_stats: RetainedStyleArtifactStats::default(),
            last_style_artifact_action: RetainedStyleArtifactAction::None,
            layout_cache: None,
            layout_artifact_stats: RetainedLayoutArtifactStats::default(),
            last_layout_artifact_action: RetainedLayoutArtifactAction::None,
            paint_cache: None,
            paint_artifact_stats: RetainedPaintArtifactStats::default(),
            last_paint_artifact_action: RetainedPaintArtifactAction::None,
            identities: RetainedRenderIdentityMap::new(),
        }
    }

    pub(super) fn reset_for_navigation(&mut self) {
        self.render_epoch = RenderEpoch::initial();
        self.document_styles.clear();
        self.generations = PageStyleGenerations::default();
        self.style_cache = None;
        self.style_dependency_cache = None;
        self.last_style_invalidation_decision = None;
        self.dirty_state = RenderDirtyState::document_initial();
        self.last_dom_mutation_facts = None;
        self.pending_style_invalidation = Some(
            classify_style_invalidation(&initial_document_style_change())
                .expect("document replacement must invalidate style"),
        );
        self.last_style_recalc = None;
        self.last_style_reuse = None;
        self.style_artifact_stats = RetainedStyleArtifactStats::default();
        self.last_style_artifact_action = RetainedStyleArtifactAction::None;
        self.layout_cache = None;
        self.layout_artifact_stats = RetainedLayoutArtifactStats::default();
        self.last_layout_artifact_action = RetainedLayoutArtifactAction::None;
        self.paint_cache = None;
        self.paint_artifact_stats = RetainedPaintArtifactStats::default();
        self.last_paint_artifact_action = RetainedPaintArtifactAction::None;
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
    ) -> Option<StyleInvalidationPlan> {
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

    pub(super) fn retained_paint_artifact_state(&self) -> RetainedPaintArtifactState {
        match (&self.paint_cache, self.paint_dirty()) {
            (None, _) => RetainedPaintArtifactState::Absent,
            (Some(_), true) => RetainedPaintArtifactState::Stale,
            (Some(_), false) => RetainedPaintArtifactState::Fresh,
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

    pub(super) fn retained_paint_artifact(&self) -> Option<&PaintArtifact> {
        self.paint_cache.as_ref().map(|entry| &entry.artifact)
    }

    pub(super) fn retained_paint_key_seed(&self) -> RetainedPaintArtifactKeySeed {
        RetainedPaintArtifactKeySeed {
            identity_domain: self.identities.domain(),
            paint_style_generation: self.generations.paint_style,
            paint_input_generation: self.generations.paint_inputs,
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

    pub(super) fn record_computed_style_invalidation_impact(
        &mut self,
        impact: ComputedDocumentStyleInvalidationImpact,
    ) {
        match impact {
            ComputedDocumentStyleInvalidationImpact::NoVisualImpact
            | ComputedDocumentStyleInvalidationImpact::StyleOnly => {
                self.dirty_state
                    .remove_phase_reason(DirtyPhase::Layout, DirtyReason::CascadedFromStyle);
                if !self.layout_dirty() {
                    self.dirty_state
                        .remove_phase_reason(DirtyPhase::Paint, DirtyReason::CascadedFromLayout);
                }
            }
            ComputedDocumentStyleInvalidationImpact::PaintOnly => {
                self.generations.paint_style = self
                    .generations
                    .paint_style
                    .checked_add(1)
                    .expect("paint style generation exhausted");
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
            ComputedDocumentStyleInvalidationImpact::LayoutAffecting
            | ComputedDocumentStyleInvalidationImpact::Unknown => {
                self.generations.layout_style = self
                    .generations
                    .layout_style
                    .checked_add(1)
                    .expect("layout style generation exhausted");
                self.generations.paint_style = self
                    .generations
                    .paint_style
                    .checked_add(1)
                    .expect("paint style generation exhausted");
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

    pub(super) fn record_paint_frame_result(&mut self, result: RetainedPaintFrameResult) {
        match result.action {
            RetainedPaintFrameAction::Reused => {
                self.paint_artifact_stats.reuse_count = self
                    .paint_artifact_stats
                    .reuse_count
                    .checked_add(1)
                    .expect("retained paint artifact reuse count exhausted");
                self.last_paint_artifact_action = RetainedPaintArtifactAction::Reused;
            }
            RetainedPaintFrameAction::Recomputed => {
                self.paint_artifact_stats.recompute_count = self
                    .paint_artifact_stats
                    .recompute_count
                    .checked_add(1)
                    .expect("retained paint artifact recompute count exhausted");
                self.last_paint_artifact_action = if self.paint_cache.is_none()
                    && self.paint_artifact_stats.recompute_count == 1
                {
                    RetainedPaintArtifactAction::InitialCompute
                } else {
                    RetainedPaintArtifactAction::Recomputed
                };
            }
            RetainedPaintFrameAction::ConservativeDocumentFallback => {
                self.paint_artifact_stats.recompute_count = self
                    .paint_artifact_stats
                    .recompute_count
                    .checked_add(1)
                    .expect("retained paint artifact recompute count exhausted");
                self.last_paint_artifact_action =
                    RetainedPaintArtifactAction::ConservativeDocumentFallback;
            }
            RetainedPaintFrameAction::ConservativeViewportFallback => {
                self.paint_artifact_stats.recompute_count = self
                    .paint_artifact_stats
                    .recompute_count
                    .checked_add(1)
                    .expect("retained paint artifact recompute count exhausted");
                self.last_paint_artifact_action =
                    RetainedPaintArtifactAction::ConservativeViewportFallback;
            }
        }
        self.paint_cache = Some(RetainedPaintArtifactEntry {
            key: result.key,
            artifact: result.artifact,
        });
        self.dirty_state.clear_phase(DirtyPhase::Paint);
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

    #[cfg(test)]
    pub(super) fn clear_style_cache_for_tests(&mut self) {
        self.style_cache = None;
    }

    #[cfg(test)]
    pub(super) fn mark_dirty_for_entry_point(&mut self, entry_point: RenderInvalidationEntryPoint) {
        self.mark_dirty_for_request(crate::rendering::render_invalidation_request(entry_point));
    }

    pub(super) fn mark_dirty_for_request(
        &mut self,
        request: crate::rendering::RenderInvalidationRequest,
    ) {
        let entry_point = request.entry_point();
        if matches!(entry_point, RenderInvalidationEntryPoint::DocumentReplaced) {
            self.dirty_state.clear();
            self.discard_layout_for_full_invalidation();
            self.discard_paint_for_full_invalidation();
        }
        match entry_point {
            RenderInvalidationEntryPoint::DocumentReplaced
            | RenderInvalidationEntryPoint::DomStructureChanged
            | RenderInvalidationEntryPoint::DomTextChanged
            | RenderInvalidationEntryPoint::DomMutationUnclassified => {
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
                self.generations.paint_inputs = self
                    .generations
                    .paint_inputs
                    .checked_add(1)
                    .expect("paint input generation exhausted");
            }
            RenderInvalidationEntryPoint::InputStateChanged => {
                self.generations.paint_inputs = self
                    .generations
                    .paint_inputs
                    .checked_add(1)
                    .expect("paint input generation exhausted");
            }
            RenderInvalidationEntryPoint::ViewportChanged
            | RenderInvalidationEntryPoint::DomAttributesChanged
            | RenderInvalidationEntryPoint::DomPublicationStyleInvalidated
            | RenderInvalidationEntryPoint::StylesheetSetChanged => {}
        }
        self.dirty_state.extend(request.dirty_request().entries);
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
                match (&self.paint_cache, self.paint_dirty()) {
                    (None, _) => RenderArtifactState::Absent,
                    (Some(_), true) => RenderArtifactState::RetainedStale,
                    (Some(_), false) => RenderArtifactState::RetainedFresh,
                },
            )
        } else {
            (
                RenderArtifactState::Absent,
                RenderArtifactState::Absent,
                RenderArtifactState::Absent,
            )
        };

        let style_invalidation = self
            .pending_style_invalidation
            .as_ref()
            .map(StyleInvalidationPlan::to_debug_snapshot);

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
            generations: self.generation_debug_snapshot(),
            style_artifacts: self.style_artifact_debug_snapshot(style_cache_state),
            layout_artifacts: self.layout_artifact_debug_snapshot(layout_tree),
            paint_artifacts: self.paint_artifact_debug_snapshot(paint_output),
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
            generations: pipeline.generations,
            style_artifacts: pipeline.style_artifacts,
            layout_artifacts: pipeline.layout_artifacts,
            paint_artifacts: pipeline.paint_artifacts,
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
        self.style_dependency_cache = None;
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

    pub(super) fn discard_paint_for_full_invalidation(&mut self) {
        if self.paint_cache.is_some() {
            self.paint_artifact_stats.discard_count = self
                .paint_artifact_stats
                .discard_count
                .checked_add(1)
                .expect("retained paint artifact discard count exhausted");
            self.last_paint_artifact_action = RetainedPaintArtifactAction::DiscardedForInvalidation;
        }
        self.paint_cache = None;
    }

    pub(super) fn reconcile_retained_identities_from_dom(&mut self, dom: &Node) {
        self.identities.reconcile_live_dom(dom);
    }

    pub(super) fn apply_style_invalidation_decision(
        &mut self,
        decision: StyleInvalidationDecision,
    ) -> Option<AppliedCssStyleInvalidation> {
        self.last_style_invalidation_decision = Some(decision.to_debug_snapshot());
        self.apply_classified_style_input_change(decision.into_plan())
    }

    #[cfg(test)]
    pub(super) fn apply_style_input_change(
        &mut self,
        change: &StyleChangeFacts,
    ) -> Option<AppliedCssStyleInvalidation> {
        self.apply_classified_style_input_change(classify_style_invalidation(change))
    }

    pub(super) fn style_dependency_artifact_for_current_context(
        &self,
        environment: css::SelectorMatchingEnvironment,
    ) -> Option<&css::StyleDependencyArtifact> {
        let expected_key = self.current_style_dependency_key();
        self.style_dependency_cache
            .as_ref()
            .filter(|entry| {
                entry.key == expected_key && entry.artifact.matches_environment(environment)
            })
            .map(|entry| &entry.artifact)
    }

    pub(super) fn mark_stylesheets_changed(&mut self) -> AppliedCssStyleInvalidation {
        let plan = classify_style_invalidation(&StyleChangeFacts::stylesheet_set_changed())
            .expect("effective stylesheet-set changes must produce Style invalidation");
        self.generations.stylesheets = self
            .generations
            .stylesheets
            .checked_add(1)
            .expect("page stylesheet generation exhausted");
        self.style_dependency_cache = None;
        self.advance_render_epoch();
        self.apply_nonempty_style_invalidation(plan)
    }

    fn apply_classified_style_input_change(
        &mut self,
        incoming: Option<StyleInvalidationPlan>,
    ) -> Option<AppliedCssStyleInvalidation> {
        let plan = incoming?;
        self.generations.style_inputs = self
            .generations
            .style_inputs
            .checked_add(1)
            .expect("page style-input generation exhausted");
        Some(self.apply_nonempty_style_invalidation(plan))
    }

    fn apply_nonempty_style_invalidation(
        &mut self,
        plan: StyleInvalidationPlan,
    ) -> AppliedCssStyleInvalidation {
        self.merge_classified_style_invalidation(Some(plan));
        AppliedCssStyleInvalidation { _private: () }
    }

    fn merge_classified_style_invalidation(&mut self, incoming: Option<StyleInvalidationPlan>) {
        let merged =
            merge_style_invalidation_plans(self.pending_style_invalidation.take(), incoming);

        if merged
            .as_ref()
            .is_some_and(StyleInvalidationPlan::invalidates_all_cached_style_artifacts)
        {
            self.record_style_artifact_discard_for_full_invalidation();
            self.style_cache = None;
        }
        self.pending_style_invalidation = merged;
    }

    pub(super) fn current_style_dependency_key(&self) -> PageStyleDependencyKey {
        PageStyleDependencyKey {
            identity_domain: self.identities.domain(),
            stylesheet_generation: self.generations.stylesheets,
        }
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

    fn paint_artifact_debug_snapshot(
        &self,
        state: RenderArtifactState,
    ) -> RetainedPaintArtifactDebugSnapshot {
        RetainedPaintArtifactDebugSnapshot {
            key: self.paint_cache.as_ref().map(|cache| cache.key),
            state,
            last_action: self.last_paint_artifact_action,
            stats: self.paint_artifact_stats,
        }
    }

    fn generation_debug_snapshot(&self) -> RetainedRenderGenerationDebugSnapshot {
        RetainedRenderGenerationDebugSnapshot {
            dom_generation: self.generations.dom,
            style_input_generation: self.generations.style_inputs,
            stylesheet_generation: self.generations.stylesheets,
            layout_input_generation: self.generations.layout_inputs,
            layout_style_generation: self.generations.layout_style,
            paint_style_generation: self.generations.paint_style,
            paint_input_generation: self.generations.paint_inputs,
            text_measurement_generation: self.generations.text_measurement,
            replaced_metadata_generation: self.generations.replaced_metadata,
        }
    }
}

impl Default for RetainedRenderState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classified_none_preserves_style_generation_key_pending_plan_and_clean_style_state() {
        let mut retained = RetainedRenderState::new();
        retained.dirty_state.clear();
        let key_before = retained.current_style_artifact_key();
        let generation_before = retained.generations.style_inputs;
        let stats_before = retained.style_artifact_stats;
        let pending_before = retained
            .pending_style_invalidation
            .as_ref()
            .map(StyleInvalidationPlan::to_debug_snapshot);

        let requested = retained.apply_classified_style_input_change(None);

        assert!(requested.is_none());
        assert_eq!(retained.generations.style_inputs, generation_before);
        assert_eq!(retained.current_style_artifact_key(), key_before);
        assert_eq!(retained.style_artifact_stats, stats_before);
        assert!(!retained.style_dirty());
        assert_eq!(
            retained
                .pending_style_invalidation
                .as_ref()
                .map(StyleInvalidationPlan::to_debug_snapshot),
            pending_before
        );
    }

    #[test]
    fn css_some_authorizes_generation_and_plan_without_preapplying_dirty_state() {
        let mut retained = RetainedRenderState::new();
        retained.pending_style_invalidation = None;
        retained.dirty_state.clear();

        let facts = StyleChangeFacts::dom_publication(
            css::DomStyleChangeFacts::builder()
                .text(css::ChangedStyleNodeFacts::changed(Vec::new()))
                .build(),
        );
        let requested = retained.apply_style_input_change(&facts);

        assert!(requested.is_some());
        assert_eq!(retained.generations.style_inputs, 1);
        assert_eq!(
            retained
                .pending_style_invalidation
                .as_ref()
                .map(StyleInvalidationPlan::to_debug_snapshot)
                .as_deref(),
            Some("scope: full-document")
        );
        assert!(!retained.style_dirty());
        assert!(retained.dirty_state.entries().is_empty());
    }
}
