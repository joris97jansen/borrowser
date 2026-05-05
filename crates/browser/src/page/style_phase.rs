use css::{
    ComputedStyleResolutionError, ComputedStyleReuseStats, StylePhaseOutput,
    build_style_tree_from_computed_styles,
};

use super::PageState;
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
        let needs_recompute = retained.style_dirty
            || retained.style_cache.as_ref().is_none_or(|cache| {
                cache.style_input_generation != retained.generations.style_inputs
                    || cache.stylesheet_generation != retained.generations.stylesheets
            });

        if needs_recompute {
            recompute_styles(
                dom,
                &retained.document_styles.cascade_stylesheet_inputs(),
                retained.generations,
                StyleRecomputeState {
                    style_cache: &mut retained.style_cache,
                    pending_style_invalidation: &mut retained.pending_style_invalidation,
                    style_dirty: &mut retained.style_dirty,
                    last_style_recalc: &mut retained.last_style_recalc,
                    last_style_reuse: &mut retained.last_style_reuse,
                },
            )?;
        } else {
            retained.last_style_recalc = Some(StyleRecalcKind::ReusedCache);
            retained.last_style_reuse = Some(ComputedStyleReuseStats::default());
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
