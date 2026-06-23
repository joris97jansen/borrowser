use css::{
    ComputedDocumentStyleLayoutImpact, ComputedStyleResolutionError, ComputedStyleReuseStats,
    StylePhaseOutput, build_style_tree_from_computed_styles,
};
use gfx::paint::PaintArtifact;
use layout::{RetainedLayoutArtifact, RetainedLayoutKeySeed};

use crate::rendering::RetainedPaintArtifactKeySeed;
use crate::rendering::{PendingRenderWork, RenderWorkPlan, RetainedStyleArtifactAction};

use super::PageState;
use super::restyle::StyleInvalidationScope;
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
        let needs_recompute = retained.style_dirty() || !retained.style_cache_matches_current_key();

        if needs_recompute {
            let had_cache_before = retained.style_cache.is_some();
            let previous_computed = retained
                .style_cache
                .as_ref()
                .map(|cache| cache.computed.clone());
            let recompute_count_before = retained.style_artifact_stats.recompute_count;
            let style_key = retained.current_style_artifact_key();
            let pending_style_invalidation = retained.take_style_invalidation_for_recompute();
            let consumed_pending_invalidation = pending_style_invalidation.is_some();
            let pending_for_action = pending_style_invalidation.clone();
            let mut style_dirty = true;
            recompute_styles(
                dom,
                &retained.document_styles.cascade_stylesheet_inputs(),
                retained.generations,
                style_key,
                pending_style_invalidation.unwrap_or(StyleInvalidationScope::Full),
                StyleRecomputeState {
                    style_cache: &mut retained.style_cache,
                    style_dirty: &mut style_dirty,
                    last_style_recalc: &mut retained.last_style_recalc,
                    last_style_reuse: &mut retained.last_style_reuse,
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
                retained.record_computed_style_layout_impact(
                    current.computed.layout_impact_against(previous),
                );
            } else if had_cache_before {
                retained.record_computed_style_layout_impact(
                    ComputedDocumentStyleLayoutImpact::Unknown,
                );
            }
            retained.record_style_artifact_recompute(style_artifact_action_for_recompute(
                retained.last_style_recalc,
                pending_for_action,
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
    pending: Option<StyleInvalidationScope>,
    had_cache_before: bool,
    recompute_count_before: u64,
) -> RetainedStyleArtifactAction {
    match recalc {
        Some(StyleRecalcKind::IncrementalSuffix { .. }) => {
            RetainedStyleArtifactAction::IncrementalSuffixRecompute
        }
        Some(StyleRecalcKind::Full { .. }) => {
            if matches!(
                pending,
                Some(StyleInvalidationScope::AttributeSuffix { .. })
            ) {
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
