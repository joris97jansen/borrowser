//! Full document-level computed-style passes.

use std::collections::BTreeMap;

use crate::{
    cascade::{
        ResolvedDocumentStyle, StyleResolutionLimits, try_resolve_document_styles_with_limits,
    },
    model,
    selectors::{SelectorDomIndex, SelectorMatchingContext},
};
use html::Node;

use super::{
    error::ComputedStyleResolutionError,
    model::{ComputedDocumentStyle, ComputedDocumentStyleWithStats, ComputedElementStyle},
    reuse::ComputedStyleReuseCache,
};

/// Resolves and computes document-level styles without mutating the DOM.
pub fn compute_document_styles(
    root: &Node,
    sheets: &[model::StylesheetParse],
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    compute_document_styles_with_limits(root, sheets, &StyleResolutionLimits::default())
}

pub fn compute_document_styles_with_limits(
    root: &Node,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    let resolved = try_resolve_document_styles_with_limits(root, sheets, limits)
        .map_err(ComputedStyleResolutionError::StyleResolution)?;
    compute_document_styles_from_resolved_styles(root, &resolved)
}

pub fn compute_document_styles_from_resolved_styles_with_reuse_stats(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
) -> Result<ComputedDocumentStyleWithStats, ComputedStyleResolutionError> {
    compute_document_styles_from_resolved_styles_pass(root, resolved_styles, None, 0)
        .map(|computed| computed.expect("full computed style pass cannot miss prefix validation"))
}

/// Computes document-level styles from an already materialized structured
/// cascade result.
pub fn compute_document_styles_from_resolved_styles(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    compute_document_styles_from_resolved_styles_with_reuse_stats(root, resolved_styles)
        .map(|computed| computed.computed)
}

pub(super) fn compute_document_styles_from_resolved_styles_pass(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
    previous_computed: Option<&ComputedDocumentStyle>,
    reused_prefix_len: usize,
) -> Result<Option<ComputedDocumentStyleWithStats>, ComputedStyleResolutionError> {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);

    if let Some(previous_computed) = previous_computed
        && (resolved_styles.entries().len() != index.len()
            || previous_computed.entries().len() != index.len()
            || reused_prefix_len > index.len())
    {
        return Ok(None);
    }

    let mut computed_by_element = BTreeMap::new();
    let mut entries = Vec::with_capacity(index.len());
    let mut reuse_cache = ComputedStyleReuseCache::default();

    for (element_index, element) in index.elements().enumerate() {
        let resolved = resolved_styles
            .get(element)
            .ok_or(ComputedStyleResolutionError::MissingResolvedElement { element })?;
        let expected_name = context.element_name(element);
        if resolved.element_name() != expected_name {
            return Err(ComputedStyleResolutionError::ResolvedElementNameMismatch {
                element,
                expected: expected_name.to_string(),
                actual: resolved.element_name().to_string(),
            });
        }

        let parent_style =
            match context.parent_element(element) {
                Some(parent) => Some(computed_by_element.get(&parent).ok_or(
                    ComputedStyleResolutionError::MissingComputedParent { element, parent },
                )?),
                None => None,
            };

        if element_index < reused_prefix_len {
            let previous = previous_computed
                .and_then(|computed| computed.entries().get(element_index))
                .expect("validated previous computed prefix");
            if previous.selector_element_id() != element || previous.element_name() != expected_name
            {
                return Ok(None);
            }

            reuse_cache.seed(resolved.style(), parent_style, *previous.style());
            computed_by_element.insert(element, *previous.style());
            entries.push(previous.clone());
            continue;
        }

        let style = reuse_cache.lookup_or_compute(resolved.style(), parent_style)?;

        computed_by_element.insert(element, style);
        entries.push(ComputedElementStyle::new(
            element,
            expected_name.to_string(),
            style,
        ));
    }

    Ok(Some(ComputedDocumentStyleWithStats {
        computed: ComputedDocumentStyle::new(entries),
        reuse_stats: reuse_cache.stats(),
    }))
}
