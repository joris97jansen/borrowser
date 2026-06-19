use css::{
    ComputedDocumentStyle, ComputedStyleResolutionError, ComputedStyleReuseStats,
    ResolvedDocumentStyle, StyleResolutionLimits, StylesheetCascadeInput,
    compute_document_styles_from_resolved_styles_with_reuse_stats,
    compute_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
    resolve_document_styles_from_cascade_inputs,
};
use html::Node;

use super::restyle::StyleInvalidationScope;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PageStyleGenerations {
    pub(crate) dom: u64,
    pub(crate) style_inputs: u64,
    pub(crate) stylesheets: u64,
}

#[derive(Clone, Debug)]
pub(super) struct PageStyleCache {
    pub(super) style_input_generation: u64,
    pub(super) stylesheet_generation: u64,
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
}

pub(super) fn recompute_styles(
    dom: &Node,
    sheets: &[StylesheetCascadeInput<'_>],
    generations: PageStyleGenerations,
    pending: StyleInvalidationScope,
    state: StyleRecomputeState<'_>,
) -> Result<(), ComputedStyleResolutionError> {
    if let StyleInvalidationScope::AttributeSuffix { node_ids } = &pending
        && let Some(cache) = state.style_cache.as_ref()
        && cache.stylesheet_generation == generations.stylesheets
    {
        let limits = StyleResolutionLimits::default();
        if let Some(incremental) =
            compute_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
                dom,
                sheets,
                &cache.resolved,
                &cache.computed,
                node_ids,
                &limits,
            )?
        {
            *state.last_style_recalc = Some(StyleRecalcKind::IncrementalSuffix {
                reused_prefix_len: incremental.reused_prefix_len,
                recomputed_len: incremental.recomputed_len,
            });
            *state.last_style_reuse = Some(incremental.reuse_stats);
            *state.style_cache = Some(PageStyleCache {
                style_input_generation: generations.style_inputs,
                stylesheet_generation: generations.stylesheets,
                resolved: incremental.resolved,
                computed: incremental.computed,
            });
            *state.style_dirty = false;
            return Ok(());
        }
    }

    let resolved = resolve_document_styles_from_cascade_inputs(dom, sheets)
        .map_err(ComputedStyleResolutionError::StyleResolution)?;
    let computed = compute_document_styles_from_resolved_styles_with_reuse_stats(dom, &resolved)?;
    let elements = computed.computed.entries().len();
    *state.last_style_recalc = Some(StyleRecalcKind::Full { elements });
    *state.last_style_reuse = Some(computed.reuse_stats);
    *state.style_cache = Some(PageStyleCache {
        style_input_generation: generations.style_inputs,
        stylesheet_generation: generations.stylesheets,
        resolved,
        computed: computed.computed,
    });
    *state.style_dirty = false;
    Ok(())
}
