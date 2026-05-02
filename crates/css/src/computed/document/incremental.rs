//! Incremental suffix recomputation for document-level computed styles.

use crate::{
    cascade::{
        ResolvedDocumentStyle, StyleResolutionLimits, StylesheetCascadeInput,
        try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
        try_resolve_document_styles_incremental_suffix_with_limits,
    },
    model,
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

pub fn compute_document_styles_incremental_suffix_with_limits(
    root: &Node,
    sheets: &[model::StylesheetParse],
    previous_resolved: &ResolvedDocumentStyle,
    previous_computed: &ComputedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalComputedDocumentStyle>, ComputedStyleResolutionError> {
    let Some(resolved) = try_resolve_document_styles_incremental_suffix_with_limits(
        root,
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
    sheets: &[StylesheetCascadeInput<'_>],
    previous_resolved: &ResolvedDocumentStyle,
    previous_computed: &ComputedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalComputedDocumentStyle>, ComputedStyleResolutionError> {
    let Some(resolved) =
        try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
            root,
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
