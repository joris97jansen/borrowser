mod debug_snapshot;
mod declarations;
mod limits;
mod rule_inputs;
mod source;

pub use self::debug_snapshot::resolve_document_styles_debug_snapshot;
pub use self::limits::{StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits};
pub use self::source::{get_inline_style, is_css};

use self::limits::{
    count_styled_elements_bounded, enforce_stylesheet_limits, validate_representation_limits,
};
use self::rule_inputs::rule_inputs_for_element_with_limits;
use super::contract::resolve_cascade_style_from_rule_inputs;
use super::document::{ResolvedDocumentStyle, ResolvedElementStyle};
use crate::model;
use crate::selectors::{SelectorDomIndex, SelectorMatchingContext};
use html::Node;
use std::collections::BTreeMap;

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
    sheets: &[model::StylesheetParse],
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    try_resolve_document_styles_with_limits(root, sheets, &StyleResolutionLimits::default())
}

pub fn try_resolve_document_styles_with_limits(
    root: &Node,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<ResolvedDocumentStyle, StyleResolutionError> {
    validate_representation_limits(limits)?;
    enforce_stylesheet_limits(sheets, limits)?;
    count_styled_elements_bounded(root, limits.max_styled_elements_per_document)?;

    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::with_limits(&index, limits.selector_matching);
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for element in index.elements() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));

        let rule_inputs = rule_inputs_for_element_with_limits(&context, element, sheets, limits)?;
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);

        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle::new(
            element,
            context.element_name(element).to_string(),
            style,
        ));
    }

    Ok(ResolvedDocumentStyle::new(entries))
}
