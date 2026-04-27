use super::super::contract::{append_cascade_evaluation_debug_snapshot, resolve_cascade_style};
use super::limits::{
    StyleResolutionLimits, count_styled_elements_bounded, enforce_stylesheet_limits,
    validate_representation_limits,
};
use super::rule_inputs::rule_inputs_for_element_with_limits;
use crate::model;
use crate::selectors::{SelectorDomIndex, SelectorMatchingContext};
use html::Node;
use std::collections::BTreeMap;
use std::fmt::Write;

/// Stable debug snapshot for document-level cascade style resolution.
///
/// This trace composes the per-element candidate evaluation snapshot with the
/// final resolved style for each element. It is intended for regression tests
/// and triage of cascade ordering, inheritance, and defaulting behavior.
pub fn resolve_document_styles_debug_snapshot(
    root: &Node,
    sheets: &[model::StylesheetParse],
) -> String {
    let limits = StyleResolutionLimits::default();
    let mut out = String::new();

    writeln!(&mut out, "version: 1").expect("write snapshot");
    writeln!(&mut out, "document-style-resolution").expect("write snapshot");

    if let Err(error) = validate_representation_limits(&limits) {
        writeln!(&mut out, "limit-error: {error}").expect("write snapshot");
        return out;
    }

    if let Err(error) = enforce_stylesheet_limits(sheets, &limits) {
        writeln!(&mut out, "limit-error: {error}").expect("write snapshot");
        return out;
    }

    if let Err(error) = count_styled_elements_bounded(root, limits.max_styled_elements_per_document)
    {
        writeln!(&mut out, "limit-error: {error}").expect("write snapshot");
        return out;
    }

    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::with_limits(&index, limits.selector_matching);
    let mut styles_by_element = BTreeMap::new();

    for (element_index, element) in index.elements().enumerate() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));

        let rule_inputs = rule_inputs_for_element_with_limits(&context, element, sheets, &limits);
        let rule_inputs = match rule_inputs {
            Ok(rule_inputs) => rule_inputs,
            Err(error) => {
                writeln!(&mut out, "limit-error: {error}").expect("write snapshot");
                return out;
            }
        };

        let mut cascade_debug = String::new();
        let winners =
            append_cascade_evaluation_debug_snapshot(&mut cascade_debug, &rule_inputs, false);
        let style = resolve_cascade_style(&winners, parent_style);

        writeln!(
            &mut out,
            "element[{element_index}]: selector-id={} name=\"{}\"",
            element.get(),
            context.element_name(element)
        )
        .expect("write snapshot");

        for line in cascade_debug.lines() {
            writeln!(&mut out, "  {line}").expect("write snapshot");
        }

        for line in style.to_debug_snapshot().lines().skip(1) {
            writeln!(&mut out, "  {line}").expect("write snapshot");
        }

        styles_by_element.insert(element, style);
    }

    out
}
