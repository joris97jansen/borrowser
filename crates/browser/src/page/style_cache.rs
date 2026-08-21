use css::{
    ComputedDocumentStyle, ComputedStyleResolutionError, ComputedStyleReuseStats,
    ResolvedDocumentStyle, RuleCollection, SelectorMatchingEnvironment, StyleInvalidationPlan,
    StylePlanExecution, StyleResolutionError, StyleResolutionExecution, StyleResolutionLimits,
    StylesheetCollectionInput, compute_document_styles_from_resolved_styles_with_reuse_stats,
    try_compute_document_styles_for_invalidation_plan_from_execution_with_limits,
};
use html::Node;

use crate::rendering::RetainedStyleArtifactKey;

#[cfg(test)]
thread_local! {
    static RULE_COLLECTION_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static STYLE_EXECUTION_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_rule_collection_build_count() {
    RULE_COLLECTION_BUILDS.set(0);
    STYLE_EXECUTION_BUILDS.set(0);
}

#[cfg(test)]
pub(crate) fn rule_collection_build_count() -> usize {
    RULE_COLLECTION_BUILDS.get()
}

#[cfg(test)]
pub(crate) fn style_execution_build_count() -> usize {
    STYLE_EXECUTION_BUILDS.get()
}

fn build_rule_collection<'source>(
    sheets: &[StylesheetCollectionInput<'source>],
    limits: &StyleResolutionLimits,
) -> Result<RuleCollection<'source>, ComputedStyleResolutionError> {
    #[cfg(test)]
    RULE_COLLECTION_BUILDS.with(|count| count.set(count.get() + 1));
    RuleCollection::try_new(sheets, limits).map_err(|error| {
        ComputedStyleResolutionError::StyleResolution(StyleResolutionError::RuleCollectionBuild(
            error,
        ))
    })
}

fn build_style_execution<'dom, 'collection, 'source>(
    dom: &'dom Node,
    environment: SelectorMatchingEnvironment,
    collection: &'collection RuleCollection<'source>,
    limits: &StyleResolutionLimits,
) -> Result<StyleResolutionExecution<'dom, 'collection, 'source>, ComputedStyleResolutionError> {
    #[cfg(test)]
    STYLE_EXECUTION_BUILDS.with(|count| count.set(count.get() + 1));
    StyleResolutionExecution::try_new(dom, environment, collection, limits)
        .map_err(ComputedStyleResolutionError::StyleResolution)
}

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
    environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCollectionInput<'_>],
    generations: PageStyleGenerations,
    key: RetainedStyleArtifactKey,
    pending: Option<&StyleInvalidationPlan>,
    state: StyleRecomputeState<'_>,
) -> Result<(), ComputedStyleResolutionError> {
    let limits = StyleResolutionLimits::default();
    let collection = build_rule_collection(sheets, &limits)?;
    let style_execution = build_style_execution(dom, environment, &collection, &limits)?;
    let execution = pending
        .map(|plan| {
            let previous = state
                .style_cache
                .as_ref()
                .filter(|cache| cache.key.stylesheet_generation == generations.stylesheets)
                .map(|cache| (&cache.resolved, &cache.computed));
            try_compute_document_styles_for_invalidation_plan_from_execution_with_limits(
                plan,
                &style_execution,
                previous,
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

    let resolved = style_execution
        .resolve_document_styles()
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
