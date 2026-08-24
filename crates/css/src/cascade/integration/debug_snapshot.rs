use super::super::contract::{
    CascadeResolutionBudget, CascadeResolutionWorkspace, InheritanceParentPresence,
    InlineStyleRuleRef, ValidatedCascadeRuleInputBuilder, append_cascade_evaluation_debug_snapshot,
    resolve_cascade_style_with_parent_presence,
};
use super::collection::RuleCollection;
use super::declarations::inline_style_declaration_inputs_from_model;
use super::limits::{StyleResolutionError, StyleResolutionLimits, validate_representation_limits};
use super::rule_inputs::rule_inputs_for_element_with_limits;
use super::selector_dom::build_document_selector_dom_with_element_limit;
use crate::selectors::{SelectorMatchingContext, SelectorMatchingEnvironment};
use crate::{model, syntax::ParseOptions};
use html::Node;
use std::fmt::Write;

/// Stable debug snapshot for declaration-list parsing and cascade eligibility.
///
/// This surface is intentionally CSS-owned and regression-test oriented. It
/// records parser diagnostics, model declarations, cascade applicability,
/// candidate materialization, and winners for one declaration-list input. It is
/// not a CSSOM serialization surface and does not affect rendering behavior.
pub(crate) fn declaration_list_pipeline_debug_snapshot(input: &str) -> String {
    match try_declaration_list_pipeline_debug_snapshot(input) {
        Ok(snapshot) => snapshot,
        Err(error) => format!(
            "version: 3\ndeclaration-list-pipeline\nfailure: kind={} detail={}\n",
            error.stable_label(),
            error
        ),
    }
}

fn try_declaration_list_pipeline_debug_snapshot(
    input: &str,
) -> Result<String, StyleResolutionError> {
    let parse = model::parse_declaration_list_with_options(input, &ParseOptions::style_attribute());
    let inline_style = InlineStyleRuleRef::diagnostic();
    let declarations =
        inline_style_declaration_inputs_from_model(inline_style, &parse.declarations)
            .map_err(StyleResolutionError::SourceCoordinate)?;
    let budget = CascadeResolutionBudget::try_new(0, declarations.len(), 0)
        .map_err(StyleResolutionError::CascadeResolution)?;
    let mut builder = ValidatedCascadeRuleInputBuilder::new(budget);
    if !declarations.is_empty() {
        let rule_input = super::super::contract::CascadeRuleInput::from_inline_style_collected(
            inline_style,
            declarations,
        )
        .map_err(StyleResolutionError::RuleInputBuild)?;
        builder
            .try_reserve_rule_inputs(1)
            .and_then(|()| builder.push_inline(rule_input))
            .map_err(StyleResolutionError::CascadeResolution)?;
    }
    let rule_inputs = builder.finish();
    let mut workspace = CascadeResolutionWorkspace::try_new(budget)
        .map_err(StyleResolutionError::CascadeResolution)?;

    let mut out = String::new();
    writeln!(&mut out, "version: 3").expect("write snapshot");
    writeln!(&mut out, "declaration-list-pipeline").expect("write snapshot");

    writeln!(&mut out, "model-parse").expect("write snapshot");
    append_indented_snapshot(&mut out, &parse.to_debug_snapshot(), 2);

    let mut cascade = String::new();
    append_cascade_evaluation_debug_snapshot(
        &mut cascade,
        &rule_inputs,
        budget,
        &mut workspace,
        false,
    )
    .map_err(StyleResolutionError::CascadeResolution)?;
    writeln!(&mut out, "cascade").expect("write snapshot");
    append_indented_snapshot(&mut out, &cascade, 2);

    Ok(out)
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
pub(crate) fn resolve_document_styles_debug_snapshot(
    root: &Node,
    matching_environment: SelectorMatchingEnvironment,
    sheets: &[model::StylesheetParse],
) -> Result<String, StyleResolutionError> {
    let limits = StyleResolutionLimits::default();
    validate_representation_limits(&limits)?;
    let inputs = super::compatibility_author_inputs(sheets)
        .map_err(StyleResolutionError::StylesheetInputBuild)?;
    let collection = RuleCollection::try_new(&inputs, &limits)
        .map_err(StyleResolutionError::RuleCollectionBuild)?;
    let index = build_document_selector_dom_with_element_limit(
        root,
        limits.max_styled_elements_per_document,
    )?;
    let mut out = String::new();

    writeln!(&mut out, "version: 5").expect("write snapshot");
    writeln!(&mut out, "document-style-resolution").expect("write snapshot");
    writeln!(
        &mut out,
        "matching-environment: document-mode={}",
        matching_environment.document_mode()
    )
    .expect("write snapshot");

    let context = SelectorMatchingContext::with_limits(
        &index,
        matching_environment,
        limits.selector_matching,
    );
    let cascade_budget = CascadeResolutionBudget::try_new(
        limits.max_declaration_inputs_per_element,
        limits.max_inline_declarations_per_element,
        limits.max_matched_rules_per_element,
    )
    .map_err(StyleResolutionError::CascadeResolution)?;
    let mut cascade_workspace = CascadeResolutionWorkspace::try_new(cascade_budget)
        .map_err(StyleResolutionError::CascadeResolution)?;

    for (element_index, element) in index.elements().enumerate() {
        let parent_element = context.parent_element(element);
        let parent_presence = match parent_element {
            Some(_) => InheritanceParentPresence::Present,
            None => InheritanceParentPresence::Absent,
        };

        let rule_inputs = rule_inputs_for_element_with_limits(
            &index,
            &context,
            element,
            &collection,
            &limits,
            cascade_budget,
        )?;

        let mut cascade_debug = String::new();
        let winners = append_cascade_evaluation_debug_snapshot(
            &mut cascade_debug,
            &rule_inputs,
            cascade_budget,
            &mut cascade_workspace,
            false,
        )
        .map_err(StyleResolutionError::CascadeResolution)?;
        let style = resolve_cascade_style_with_parent_presence(&winners, parent_presence);

        writeln!(
            &mut out,
            "element[{element_index}]: selector-id={} namespace={} name=\"{}\"",
            element.get(),
            context.element_namespace(element).snapshot_name(),
            context.element_local_name(element)
        )
        .expect("write snapshot");
        match parent_element {
            Some(parent) => writeln!(
                &mut out,
                "  inheritance-parent: selector-id={}",
                parent.get()
            )
            .expect("write snapshot"),
            None => writeln!(&mut out, "  inheritance-parent: none").expect("write snapshot"),
        }

        for line in cascade_debug.lines() {
            writeln!(&mut out, "  {line}").expect("write snapshot");
        }

        for line in style.to_debug_snapshot().lines().skip(1) {
            writeln!(&mut out, "  {line}").expect("write snapshot");
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::declaration_list_pipeline_debug_snapshot;

    fn fixture_input(text: &str) -> &str {
        text.strip_suffix("\r\n")
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(text)
    }

    #[test]
    fn declaration_pipeline_snapshot_golden_ad8_declarations() {
        assert_eq!(
            declaration_list_pipeline_debug_snapshot(fixture_input(include_str!(
                "../../../tests/fixtures/declarations/ad8_declaration_pipeline.css"
            ))),
            include_str!("../../../tests/fixtures/declarations/ad8_declaration_pipeline.snap"),
        );
    }
}
