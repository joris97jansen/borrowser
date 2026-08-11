use css::{
    ComputedDocumentStyle, ComputedStyleResolutionError, ComputedStyleReuseStats,
    ResolvedDocumentStyle, StyleInvalidationPlan, StylePlanExecution, StyleResolutionLimits,
    StylesheetCascadeInput, compute_document_styles_from_resolved_styles_with_reuse_stats,
    resolve_document_styles_from_cascade_inputs,
    try_compute_document_styles_for_invalidation_plan_with_limits,
};
use html::Node;

use crate::rendering::RetainedStyleArtifactKey;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PageStyleGenerations {
    pub(crate) dom: u64,
    pub(crate) style_inputs: u64,
    pub(crate) stylesheets: u64,
    pub(crate) layout_inputs: u64,
    pub(crate) layout_style: u64,
    pub(crate) paint_style: u64,
    pub(crate) paint_inputs: u64,
    pub(crate) text_measurement: u64,
    pub(crate) replaced_metadata: u64,
}

#[derive(Clone, Debug)]
pub(super) struct PageStyleCache {
    pub(super) key: RetainedStyleArtifactKey,
    pub(super) resolved: ResolvedDocumentStyle,
    pub(super) computed: ComputedDocumentStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StyleRecalcKind {
    ReusedCache,
    Full {
        elements: usize,
    },
    IncrementalSuffix {
        reused_prefix_len: usize,
        recomputed_len: usize,
    },
}

pub(super) struct StyleRecomputeState<'a> {
    pub(super) style_cache: &'a mut Option<PageStyleCache>,
    pub(super) style_dirty: &'a mut bool,
    pub(super) last_style_recalc: &'a mut Option<StyleRecalcKind>,
    pub(super) last_style_reuse: &'a mut Option<ComputedStyleReuseStats>,
    /// CSS authorized an incremental path, but the execution result did not
    /// necessarily invoke it. This is used only to label a later full
    /// recomputation as fallback from incremental eligibility.
    pub(super) last_style_incremental_eligible: &'a mut bool,
}

pub(super) fn recompute_styles(
    dom: &Node,
    sheets: &[StylesheetCascadeInput<'_>],
    generations: PageStyleGenerations,
    key: RetainedStyleArtifactKey,
    pending: Option<&StyleInvalidationPlan>,
    state: StyleRecomputeState<'_>,
) -> Result<(), ComputedStyleResolutionError> {
    let limits = StyleResolutionLimits::default();
    let execution = pending
        .map(|plan| {
            let previous = state
                .style_cache
                .as_ref()
                .filter(|cache| cache.key.stylesheet_generation == generations.stylesheets)
                .map(|cache| (&cache.resolved, &cache.computed));
            try_compute_document_styles_for_invalidation_plan_with_limits(
                plan, dom, sheets, previous, &limits,
            )
        })
        .transpose()?;
    *state.last_style_incremental_eligible = execution
        .as_ref()
        .is_some_and(StylePlanExecution::is_incremental_eligible);

    if let Some(StylePlanExecution::IncrementalComputed(incremental)) = execution {
        *state.last_style_recalc = Some(StyleRecalcKind::IncrementalSuffix {
            reused_prefix_len: incremental.reused_prefix_len,
            recomputed_len: incremental.recomputed_len,
        });
        *state.last_style_reuse = Some(incremental.reuse_stats);
        *state.style_cache = Some(PageStyleCache {
            key,
            resolved: incremental.resolved,
            computed: incremental.computed,
        });
        *state.style_dirty = false;
        return Ok(());
    }

    let resolved = resolve_document_styles_from_cascade_inputs(dom, sheets)
        .map_err(ComputedStyleResolutionError::StyleResolution)?;
    let computed = compute_document_styles_from_resolved_styles_with_reuse_stats(dom, &resolved)?;
    let elements = computed.computed.entries().len();
    *state.last_style_recalc = Some(StyleRecalcKind::Full { elements });
    *state.last_style_reuse = Some(computed.reuse_stats);
    *state.style_cache = Some(PageStyleCache {
        key,
        resolved,
        computed: computed.computed,
    });
    *state.style_dirty = false;
    Ok(())
}
