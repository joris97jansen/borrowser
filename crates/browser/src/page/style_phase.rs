use css::{
    ComputedDocumentStyleInvalidationImpact, ComputedStyleResolutionError, ComputedStyleReuseStats,
    StylePhaseOutput, build_style_tree_from_computed_styles,
};
use gfx::paint::PaintArtifact;
use layout::{RetainedLayoutArtifact, RetainedLayoutKeySeed};

use crate::rendering::RetainedPaintArtifactKeySeed;
use crate::rendering::{PendingRenderWork, RenderWorkPlan, RetainedStyleArtifactAction};

use super::PageState;
use super::style_cache::{StyleRecalcKind, StyleRecomputeState, recompute_styles};

pub(crate) struct PreparedStylePhaseForFrame<'a> {
    pub(crate) style_output: StylePhaseOutput<'a>,
    pub(crate) work_plan: RenderWorkPlan,
    pub(crate) retained_layout_key_seed: RetainedLayoutKeySeed,
    pub(crate) retained_layout_artifact: Option<RetainedLayoutArtifact>,
    pub(crate) retained_paint_key_seed: RetainedPaintArtifactKeySeed,
    pub(crate) retained_paint_artifact: Option<PaintArtifact>,
}

impl PageState {
    /// Runtime style-phase boundary for page rendering.
    ///
    /// `PageState` owns retained resolved/computed style artifacts and the
    /// invalidation logic that decides whether they can be reused. This method
    /// either reuses or recomputes those retained artifacts, then rebuilds the
    /// borrow-backed `StyledNode` view wrapped in an explicit style-phase
    /// output contract for downstream layout and paint.
    pub(crate) fn build_style_phase_output(
        &mut self,
    ) -> Result<Option<StylePhaseOutput<'_>>, ComputedStyleResolutionError> {
        if !self.ensure_retained_style_artifacts()? {
            return Ok(None);
        }

        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };
        let cache = self
            .rendering
            .style_cache
            .as_ref()
            .expect("style cache must exist after successful style computation");
        build_style_tree_from_computed_styles(dom, &cache.computed)
            .map(StylePhaseOutput::new)
            .map(Some)
    }

    pub(crate) fn prepare_style_phase_for_frame(
        &mut self,
        pending_work: &PendingRenderWork,
    ) -> Result<Option<PreparedStylePhaseForFrame<'_>>, ComputedStyleResolutionError> {
        if !self.ensure_retained_style_artifacts()? {
            return Ok(None);
        }

        let work_plan = self.derive_render_work_plan(pending_work);
        let retained_layout_key_seed = self.retained_layout_key_seed();
        let retained_layout_artifact = self.retained_layout_artifact().cloned();
        let retained_paint_key_seed = self.retained_paint_key_seed();
        let retained_paint_artifact = self.retained_paint_artifact().cloned();

        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };
        let cache = self
            .rendering
            .style_cache
            .as_ref()
            .expect("style cache must exist after successful style computation");
        build_style_tree_from_computed_styles(dom, &cache.computed)
            .map(StylePhaseOutput::new)
            .map(|style_output| {
                Some(PreparedStylePhaseForFrame {
                    style_output,
                    work_plan,
                    retained_layout_key_seed,
                    retained_layout_artifact,
                    retained_paint_key_seed,
                    retained_paint_artifact,
                })
            })
    }

    fn ensure_retained_style_artifacts(&mut self) -> Result<bool, ComputedStyleResolutionError> {
        let Some(dom) = self.dom.as_deref() else {
            return Ok(false);
        };

        let retained = &mut self.rendering;
        let Some(document_mode) = self.document_mode else {
            return Err(ComputedStyleResolutionError::MissingMatchingEnvironment);
        };
        let environment = css::SelectorMatchingEnvironment::new(document_mode);
        let cache_environment_matches = retained
            .style_cache
            .as_ref()
            .is_some_and(|cache| cache.computed.matching_environment() == environment);
        let needs_recompute = retained.style_dirty()
            || !retained.style_cache_matches_current_key()
            || !cache_environment_matches;

        if needs_recompute {
            let had_cache_before = retained.style_cache.is_some();
            let previous_computed = retained
                .style_cache
                .as_ref()
                .map(|cache| cache.computed.clone());
            let recompute_count_before = retained.style_artifact_stats.recompute_count;
            let style_key = retained.current_style_artifact_key();
            let dependency_key = retained.current_style_dependency_key();
            let pending_style_invalidation = retained.take_style_invalidation_for_recompute();
            let consumed_pending_invalidation = pending_style_invalidation.is_some();
            let mut style_dirty = true;
            let mut incremental_eligible = false;
            let stylesheet_inputs = retained
                .document_styles
                .stylesheet_collection_inputs()
                .map_err(|error| {
                    ComputedStyleResolutionError::StyleResolution(
                        css::StyleResolutionError::StylesheetInputBuild(error),
                    )
                })?;
            recompute_styles(
                super::style_cache::StyleRecomputeInput {
                    dom,
                    environment,
                    sheets: &stylesheet_inputs,
                    generations: retained.generations,
                    key: style_key,
                    pending: pending_style_invalidation.as_ref(),
                    dependency_key,
                },
                StyleRecomputeState {
                    style_cache: &mut retained.style_cache,
                    style_dirty: &mut style_dirty,
                    last_style_recalc: &mut retained.last_style_recalc,
                    last_style_reuse: &mut retained.last_style_reuse,
                    last_style_incremental_eligible: &mut incremental_eligible,
                    dependency_cache: &mut retained.style_dependency_cache,
                },
            )?;
            if !style_dirty {
                retained.clear_style_dirty_after_recompute();
            }
            if !consumed_pending_invalidation {
                retained.advance_render_epoch();
            }
            if let Some(previous) = previous_computed.as_ref()
                && let Some(current) = retained.style_cache.as_ref()
            {
                retained.record_computed_style_invalidation_impact(
                    current.computed.invalidation_impact_against(previous),
                );
            } else if had_cache_before {
                retained.record_computed_style_invalidation_impact(
                    ComputedDocumentStyleInvalidationImpact::Unknown,
                );
            }
            retained.record_style_artifact_recompute(style_artifact_action_for_recompute(
                retained.last_style_recalc,
                incremental_eligible,
                had_cache_before,
                recompute_count_before,
            ));
        } else {
            retained.last_style_recalc = Some(StyleRecalcKind::ReusedCache);
            retained.last_style_reuse = Some(ComputedStyleReuseStats::default());
            retained.record_style_artifact_reuse();
        }

        Ok(true)
    }
}

fn style_artifact_action_for_recompute(
    recalc: Option<StyleRecalcKind>,
    incremental_eligible: bool,
    had_cache_before: bool,
    recompute_count_before: u64,
) -> RetainedStyleArtifactAction {
    match recalc {
        Some(StyleRecalcKind::IncrementalSuffix { .. }) => {
            RetainedStyleArtifactAction::IncrementalSuffixRecompute
        }
        Some(StyleRecalcKind::Full { .. }) => {
            if incremental_eligible {
                RetainedStyleArtifactAction::FallbackFullRecompute
            } else if !had_cache_before && recompute_count_before == 0 {
                RetainedStyleArtifactAction::InitialCompute
            } else {
                RetainedStyleArtifactAction::FullRecompute
            }
        }
        Some(StyleRecalcKind::ReusedCache) | None => RetainedStyleArtifactAction::FullRecompute,
    }
}
