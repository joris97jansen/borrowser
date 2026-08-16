mod debug_snapshot;
mod declarations;
mod limits;
mod rule_inputs;
mod selector_dom;
mod source;

pub use self::debug_snapshot::{
    declaration_list_pipeline_debug_snapshot, resolve_document_styles_debug_snapshot,
};
pub use self::limits::{StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits};
pub use self::source::{StylesheetCascadeInput, get_inline_style, is_css};

use self::limits::{
    enforce_stylesheet_input_limits, enforce_stylesheet_limits, validate_representation_limits,
};
use self::rule_inputs::{
    rule_inputs_for_element_from_cascade_inputs_with_limits, rule_inputs_for_element_with_limits,
};
use self::selector_dom::{
    build_document_selector_dom_with_element_limit,
    build_element_subtree_selector_dom_with_element_limit,
    preflight_document_selector_dom_with_element_limit,
};
use super::contract::resolve_cascade_style_from_rule_inputs;
use super::document::{ResolvedDocumentStyle, ResolvedElementStyle};
use crate::model;
use crate::selectors::{SelectorDomIndex, SelectorMatchingContext, SelectorMatchingEnvironment};
use html::{ElementNode, Node, internal::Id};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncrementalStyleResolutionStats {
    pub reused_prefix_len: usize,
    pub recomputed_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncrementalResolvedDocumentStyle {
    pub resolved: ResolvedDocumentStyle,
    pub stats: IncrementalStyleResolutionStats,
}

/// Applies the authoritative style-resolution representation limits and the
/// exact bounded selector-DOM construction preflight without materializing a
/// projection. This is used when an incremental-eligible plan has no retained
/// artifacts but invalid document structure must still remain a typed error.
pub(crate) fn preflight_document_selector_dom_with_limits(
    root: &Node,
    limits: &StyleResolutionLimits,
) -> Result<(), StyleResolutionError> {
    validate_representation_limits(limits)?;
    preflight_document_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )
}

/// Resolves structured cascade output for every element in `root`.
///
/// The output is ordered by selector-DOM document order and does not mutate the
/// DOM. Stylesheet declarations, inline style attributes, selector match
/// outcomes, winner resolution, inheritance, and initial/default fill all flow
/// through the Milestone R structured cascade pipeline. Limit failures remain
/// explicit on this authoritative path; compatibility fallbacks belong in
/// callers that deliberately opt into them.
pub fn resolve_document_styles(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    try_resolve_document_styles_with_limits(
        root,
        matching_environment,
        sheets,
        &StyleResolutionLimits::default(),
    )
}

pub fn resolve_document_styles_from_cascade_inputs(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCascadeInput<'_>],
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    try_resolve_document_styles_from_cascade_inputs_with_limits(
        root,
        matching_environment,
        sheets,
        &StyleResolutionLimits::default(),
    )
}

pub fn try_resolve_document_styles_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    validate_representation_limits(limits)?;
    enforce_stylesheet_limits(sheets, limits)?;
    let index = build_document_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )?;
    resolve_document_styles_with_index(&index, matching_environment, sheets, limits)
}

fn resolve_document_styles_with_index(
    index: &SelectorDomIndex<'_>,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    let context =
        SelectorMatchingContext::with_limits(index, matching_environment, limits.selector_matching);
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for element in index.elements() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));

        let rule_inputs =
            rule_inputs_for_element_with_limits(index, &context, element, sheets, limits)?;
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);

        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle::new(
            element,
            context.element_namespace(element),
            context.element_local_name(element).to_string(),
            style,
        ));
    }

    Ok(ResolvedDocumentStyle::new(matching_environment, entries))
}

pub fn try_resolve_document_styles_from_cascade_inputs_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCascadeInput<'_>],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    validate_representation_limits(limits)?;
    enforce_stylesheet_input_limits(sheets, limits)?;
    let index = build_document_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )?;
    resolve_document_styles_from_cascade_inputs_with_index(
        &index,
        matching_environment,
        sheets,
        limits,
    )
}

fn resolve_document_styles_from_cascade_inputs_with_index(
    index: &SelectorDomIndex<'_>,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCascadeInput<'_>],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    let context =
        SelectorMatchingContext::with_limits(index, matching_environment, limits.selector_matching);
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for element in index.elements() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));

        let rule_inputs = rule_inputs_for_element_from_cascade_inputs_with_limits(
            index, &context, element, sheets, limits,
        )?;
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);

        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle::new(
            element,
            context.element_namespace(element),
            context.element_local_name(element).to_string(),
            style,
        ));
    }

    Ok(ResolvedDocumentStyle::new(matching_environment, entries))
}

pub fn try_resolve_document_styles_incremental_suffix_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    previous: &ResolvedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalResolvedDocumentStyle>, StyleResolutionError> {
    let inputs = sheets
        .iter()
        .map(StylesheetCascadeInput::author)
        .collect::<Vec<_>>();
    try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
        root,
        matching_environment,
        &inputs,
        previous,
        dirty_node_ids,
        limits,
    )
}

pub fn try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[StylesheetCascadeInput<'_>],
    previous: &ResolvedDocumentStyle,
    dirty_node_ids: &[Id],
    limits: &StyleResolutionLimits,
) -> Result<Option<IncrementalResolvedDocumentStyle>, StyleResolutionError> {
    validate_representation_limits(limits)?;
    enforce_stylesheet_input_limits(sheets, limits)?;
    let index = build_document_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )?;

    if previous.matching_environment() != matching_environment {
        return Err(StyleResolutionError::MatchingEnvironmentMismatch {
            expected: matching_environment,
            actual: previous.matching_environment(),
        });
    }

    if dirty_node_ids.is_empty() {
        return Ok(None);
    }

    if previous.entries().len() != index.len() {
        return Ok(None);
    }

    let Some(reused_prefix_len) = earliest_dirty_element_index(&index, dirty_node_ids) else {
        return Ok(None);
    };

    let context = SelectorMatchingContext::with_limits(
        &index,
        matching_environment,
        limits.selector_matching,
    );
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for (element_index, element) in index.elements().enumerate() {
        if element_index < reused_prefix_len {
            let Some(previous_entry) = previous.entries().get(element_index) else {
                return Ok(None);
            };
            if previous_entry.selector_element_id() != element
                || previous_entry.element_namespace() != context.element_namespace(element)
                || previous_entry.element_name() != context.element_local_name(element)
            {
                return Ok(None);
            }

            styles_by_element.insert(element, previous_entry.style().clone());
            entries.push(previous_entry.clone());
            continue;
        }

        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));

        let rule_inputs = rule_inputs_for_element_from_cascade_inputs_with_limits(
            &index, &context, element, sheets, limits,
        )?;
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);

        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle::new(
            element,
            context.element_namespace(element),
            context.element_local_name(element).to_string(),
            style,
        ));
    }

    Ok(Some(IncrementalResolvedDocumentStyle {
        resolved: ResolvedDocumentStyle::new(matching_environment, entries),
        stats: IncrementalStyleResolutionStats {
            reused_prefix_len,
            recomputed_len: index.len() - reused_prefix_len,
        },
    }))
}

pub(crate) fn try_resolve_element_subtree_styles_with_limits(
    root: &ElementNode,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    validate_representation_limits(limits)?;
    enforce_stylesheet_limits(sheets, limits)?;
    let index = build_element_subtree_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )?;
    resolve_document_styles_with_index(&index, matching_environment, sheets, limits)
}

fn earliest_dirty_element_index(
    index: &SelectorDomIndex<'_>,
    dirty_node_ids: &[Id],
) -> Option<usize> {
    dirty_node_ids
        .iter()
        .filter_map(|node_id| index.element_for_node_id(*node_id))
        .map(|element| (element.get() - 1) as usize)
        .min()
}
