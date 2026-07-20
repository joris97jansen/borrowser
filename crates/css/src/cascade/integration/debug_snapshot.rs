use super::super::contract::{
    InlineStyleRuleRef, append_cascade_evaluation_debug_snapshot, resolve_cascade_style,
};
use super::declarations::inline_style_declaration_inputs_from_model;
use super::limits::{
    StyleResolutionLimits, count_styled_elements_bounded, enforce_stylesheet_limits,
    validate_representation_limits,
};
use super::rule_inputs::rule_inputs_for_element_with_limits;
use crate::selectors::{SelectorDomIndex, SelectorMatchingContext};
use crate::{model, syntax::ParseOptions};
use html::Node;
use std::collections::BTreeMap;
use std::fmt::Write;

/// Stable debug snapshot for declaration-list parsing and cascade eligibility.
///
/// This surface is intentionally CSS-owned and regression-test oriented. It
/// records parser diagnostics, model declarations, cascade applicability,
/// candidate materialization, and winners for one declaration-list input. It is
/// not a CSSOM serialization surface and does not affect rendering behavior.
pub fn declaration_list_pipeline_debug_snapshot(input: &str) -> String {
    let parse = model::parse_declaration_list_with_options(input, &ParseOptions::style_attribute());
    let inline_style = InlineStyleRuleRef::new(0);
    let declarations =
        inline_style_declaration_inputs_from_model(inline_style, &parse.declarations);
    let rule_inputs = if declarations.is_empty() {
        Vec::new()
    } else {
        vec![
            super::super::contract::CascadeRuleInput::from_inline_style(
                inline_style,
                0,
                declarations,
            )
            .expect("declaration-list debug snapshot uses one internally consistent inline source"),
        ]
    };

    let mut out = String::new();
    writeln!(&mut out, "version: 1").expect("write snapshot");
    writeln!(&mut out, "declaration-list-pipeline").expect("write snapshot");

    writeln!(&mut out, "model-parse").expect("write snapshot");
    append_indented_snapshot(&mut out, &parse.to_debug_snapshot(), 2);

    let mut cascade = String::new();
    append_cascade_evaluation_debug_snapshot(&mut cascade, &rule_inputs, false);
    writeln!(&mut out, "cascade").expect("write snapshot");
    append_indented_snapshot(&mut out, &cascade, 2);

    out
}

fn append_indented_snapshot(out: &mut String, snapshot: &str, indent: usize) {
    let indent = " ".repeat(indent);
    for line in snapshot.lines() {
        writeln!(out, "{indent}{line}").expect("write snapshot");
    }
}

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
            "element[{element_index}]: selector-id={} namespace={} name=\"{}\"",
            element.get(),
            context.element_namespace(element).snapshot_name(),
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
