//! Incremental suffix recomputation for document-level computed styles.

use crate::{
    StyleInvalidationPlan,
    cascade::{
        ResolvedDocumentStyle, StyleResolutionLimits, StylesheetCascadeInput,
        try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
        try_resolve_document_styles_incremental_suffix_with_limits,
    },
    model,
    selectors::SelectorMatchingEnvironment,
};
use html::{Node, internal::Id};

use super::{
    compute::compute_document_styles_from_resolved_styles_pass,
    error::ComputedStyleResolutionError,
    model::{ComputedDocumentStyle, ComputedDocumentStyleWithStats, ComputedStyleReuseStats},
};

#[derive(Clone, Debug, PartialEq)]
pub struct IncrementalComputedDocumentStyle {
    pub resolved: ResolvedDocumentStyle,
    pub computed: ComputedDocumentStyle,
    pub reused_prefix_len: usize,
    pub recomputed_len: usize,
    pub reuse_stats: ComputedStyleReuseStats,
}

/// Result of asking CSS to execute an opaque style invalidation plan against
/// the retained artifacts that the caller made available.
///
/// This is an execution result, not a copy of the semantic invalidation plan.
/// A suffix plan can authorize incremental reuse while still producing
/// [`Self::IncrementalUnavailable`] when retained artifacts are missing or
/// incremental validation cannot materialize a safe result.
#[derive(Clone, Debug, PartialEq)]
pub enum StylePlanExecution {
    /// The semantic plan requires a full document computation.
    FullRequired,
    /// CSS permitted the incremental path, but no incremental result could be
    /// produced from the retained artifacts supplied by the caller.
    IncrementalUnavailable,
    /// The permitted incremental path produced a computed document style.
    IncrementalComputed(IncrementalComputedDocumentStyle),
}

impl StylePlanExecution {
    /// Returns whether the semantic plan authorized an incremental fallback.
    /// This does not claim that the incremental algorithm was invoked.
    pub fn is_incremental_eligible(&self) -> bool {
        matches!(
            self,
            Self::IncrementalUnavailable | Self::IncrementalComputed(_)
        )
    }
}

pub fn try_compute_document_styles_for_invalidation_plan_with_limits(
    plan: &StyleInvalidationPlan,
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCascadeInput<'_>],
    previous: Option<(&ResolvedDocumentStyle, &ComputedDocumentStyle)>,
    limits: &StyleResolutionLimits,
) -> Result<StylePlanExecution, ComputedStyleResolutionError> {
    let Some(dirty_node_ids) = plan.incremental_node_ids() else {
        return Ok(StylePlanExecution::FullRequired);
    };

    let Some((previous_resolved, previous_computed)) = previous else {
        return Ok(StylePlanExecution::IncrementalUnavailable);
    };

    let Some(incremental) =
        compute_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
            root,
            matching_environment,
            sheets,
            previous_resolved,
            previous_computed,
            dirty_node_ids,
            limits,
        )?
    else {
        return Ok(StylePlanExecution::IncrementalUnavailable);
    };

    Ok(StylePlanExecution::IncrementalComputed(incremental))
}

pub fn compute_document_styles_incremental_suffix_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    previous_resolved: &ResolvedDocumentStyle,
    previous_computed: &ComputedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalComputedDocumentStyle>, ComputedStyleResolutionError> {
    let Some(resolved) = try_resolve_document_styles_incremental_suffix_with_limits(
        root,
        matching_environment,
        sheets,
        previous_resolved,
        dirty_node_ids,
        limits,
    )
    .map_err(ComputedStyleResolutionError::StyleResolution)?
    else {
        return Ok(None);
    };

    let Some(computed) = compute_document_styles_from_resolved_styles_incremental_suffix(
        root,
        &resolved.resolved,
        previous_computed,
        resolved.stats.reused_prefix_len,
    )?
    else {
        return Ok(None);
    };

    let reuse_stats = computed.reuse_stats;
    Ok(Some(IncrementalComputedDocumentStyle {
        resolved: resolved.resolved,
        computed: computed.computed,
        reused_prefix_len: resolved.stats.reused_prefix_len,
        recomputed_len: resolved.stats.recomputed_len,
        reuse_stats,
    }))
}

pub fn compute_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCascadeInput<'_>],
    previous_resolved: &ResolvedDocumentStyle,
    previous_computed: &ComputedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalComputedDocumentStyle>, ComputedStyleResolutionError> {
    let Some(resolved) =
        try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
            root,
            matching_environment,
            sheets,
            previous_resolved,
            dirty_node_ids,
            limits,
        )
        .map_err(ComputedStyleResolutionError::StyleResolution)?
    else {
        return Ok(None);
    };

    let Some(computed) = compute_document_styles_from_resolved_styles_incremental_suffix(
        root,
        &resolved.resolved,
        previous_computed,
        resolved.stats.reused_prefix_len,
    )?
    else {
        return Ok(None);
    };

    let reuse_stats = computed.reuse_stats;
    Ok(Some(IncrementalComputedDocumentStyle {
        resolved: resolved.resolved,
        computed: computed.computed,
        reused_prefix_len: resolved.stats.reused_prefix_len,
        recomputed_len: resolved.stats.recomputed_len,
        reuse_stats,
    }))
}

fn compute_document_styles_from_resolved_styles_incremental_suffix(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
    previous_computed: &ComputedDocumentStyle,
    reused_prefix_len: usize,
) -> Result<Option<ComputedDocumentStyleWithStats>, ComputedStyleResolutionError> {
    compute_document_styles_from_resolved_styles_pass(
        root,
        resolved_styles,
        Some(previous_computed),
        reused_prefix_len,
    )
}
