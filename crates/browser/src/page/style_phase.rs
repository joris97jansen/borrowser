use css::{
    ComputedStyleResolutionError, ComputedStyleReuseStats, StylePhaseOutput,
    build_style_tree_from_computed_styles,
};

use crate::rendering::RetainedStyleArtifactAction;

use super::PageState;
use super::restyle::StyleInvalidationScope;
use super::style_cache::{StyleRecalcKind, StyleRecomputeState, recompute_styles};

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
        let Some(dom) = self.dom.as_deref() else {
            return Ok(None);
        };

        let retained = &mut self.rendering;
        let needs_recompute = retained.style_dirty() || !retained.style_cache_matches_current_key();

        if needs_recompute {
            let had_cache_before = retained.style_cache.is_some();
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

        let cache = retained
            .style_cache
            .as_ref()
            .expect("style cache must exist after successful style computation");
        build_style_tree_from_computed_styles(dom, &cache.computed)
            .map(StylePhaseOutput::new)
            .map(Some)
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
