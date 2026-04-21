use super::contract::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeImportance, CascadeOrigin,
    CascadeRuleInput, CascadeRuleMatch, CascadeSpecifiedValue, InlineStyleDeclarationRef,
    InlineStyleRuleRef, StylesheetDeclarationRef, append_cascade_evaluation_debug_snapshot,
    resolve_cascade_style, resolve_cascade_style_from_rule_inputs,
};
use super::document::{ResolvedDocumentStyle, ResolvedElementStyle};
use crate::model::{self, PropertyNameKind};
use crate::property_registry;
use crate::selectors::{SelectorDomElementId, SelectorDomIndex, SelectorMatchingContext};
use crate::syntax::ParseOptions;
use html::Node;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::Arc;

pub fn is_css(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase().starts_with("text/css"))
        .unwrap_or(false)
}

/// If the element has an inline style attribute, return its value.
pub fn get_inline_style(attributes: &[(Arc<str>, Option<String>)]) -> Option<&str> {
    attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("style"))
        .and_then(|(_, value)| value.as_deref())
}

/// Resolves structured cascade output for every element in `root`.
///
/// The output is ordered by selector-DOM document order and does not mutate the
/// DOM. Stylesheet declarations, inline style attributes, selector match
/// outcomes, winner resolution, inheritance, and initial/default fill all flow
/// through the Milestone R structured cascade pipeline.
pub fn resolve_document_styles(
    root: &Node,
    sheets: &[model::StylesheetParse],
) -> ResolvedDocumentStyle {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);
    let mut entries = Vec::with_capacity(index.len());
    let mut styles_by_element = BTreeMap::new();

    for element in index.elements() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));
        let rule_inputs = rule_inputs_for_element(&context, element, sheets);
        let style = resolve_cascade_style_from_rule_inputs(&rule_inputs, parent_style);

        styles_by_element.insert(element, style.clone());
        entries.push(ResolvedElementStyle::new(
            element,
            context.element_name(element).to_string(),
            style,
        ));
    }

    ResolvedDocumentStyle::new(entries)
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
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);
    let mut styles_by_element = BTreeMap::new();
    let mut out = String::new();

    writeln!(&mut out, "version: 1").expect("write snapshot");
    writeln!(&mut out, "document-style-resolution").expect("write snapshot");

    for (element_index, element) in index.elements().enumerate() {
        let parent_style = context
            .parent_element(element)
            .and_then(|parent| styles_by_element.get(&parent));
        let rule_inputs = rule_inputs_for_element(&context, element, sheets);
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

fn rule_inputs_for_element(
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    element: SelectorDomElementId,
    sheets: &[model::StylesheetParse],
) -> Vec<CascadeRuleInput> {
    let mut rule_inputs = Vec::new();
    let mut rule_order = 0u32;

    for (stylesheet_index, sheet) in sheets.iter().enumerate() {
        let stylesheet_index = u32_index(stylesheet_index, "stylesheet");
        for (rule_index, rule) in sheet.stylesheet.rules.iter().enumerate() {
            let rule_index = u32_index(rule_index, "rule");
            let model::Rule::Style(rule) = rule else {
                continue;
            };
            let current_rule_order = rule_order;
            rule_order = rule_order
                .checked_add(1)
                .expect("stylesheet rule order exceeds u32 range");

            let rule_match = CascadeRuleMatch {
                stylesheet_index,
                rule_index,
                outcome: context.match_selector_list(element, &rule.selectors),
            };
            if !rule_match.contributes_candidates() {
                continue;
            }

            let declarations = stylesheet_declaration_inputs(
                stylesheet_index,
                rule_index,
                &rule.declarations.declarations,
            );
            if declarations.is_empty() {
                continue;
            }

            if let Some(rule_input) = CascadeRuleInput::from_stylesheet_match(
                &rule_match,
                CascadeOrigin::Author,
                current_rule_order,
                declarations,
            )
            .expect("stylesheet declarations must belong to their stylesheet rule")
            {
                rule_inputs.push(rule_input);
            }
        }
    }

    let inline_rule_order = rule_order;
    if let Some(inline_style) = context.attribute_value(element, "style")
        && let Some(rule_input) = inline_style_rule_input(element, inline_rule_order, inline_style)
    {
        rule_inputs.push(rule_input);
    }

    rule_inputs
}

fn stylesheet_declaration_inputs(
    stylesheet_index: u32,
    rule_index: u32,
    declarations: &[model::Declaration],
) -> Vec<CascadeDeclarationInput> {
    declarations
        .iter()
        .enumerate()
        .map(|(declaration_index, declaration)| {
            let declaration_index = u32_index(declaration_index, "declaration");
            declaration_input_from_model(
                CascadeDeclarationSource::Stylesheet(StylesheetDeclarationRef {
                    stylesheet_index,
                    rule_index,
                    declaration_index,
                }),
                declaration_index,
                declaration,
            )
        })
        .collect()
}

fn inline_style_rule_input(
    element: SelectorDomElementId,
    rule_order: u32,
    inline_style_text: &str,
) -> Option<CascadeRuleInput> {
    if inline_style_text.trim().is_empty() {
        return None;
    }

    let inline_style = InlineStyleRuleRef::new(element.get());
    let declarations = inline_style_declaration_inputs(inline_style, inline_style_text);
    if declarations.is_empty() {
        return None;
    }

    Some(
        CascadeRuleInput::from_inline_style(inline_style, rule_order, declarations)
            .expect("inline declarations must belong to their inline style rule"),
    )
}

fn inline_style_declaration_inputs(
    inline_style: InlineStyleRuleRef,
    inline_style_text: &str,
) -> Vec<CascadeDeclarationInput> {
    // The model layer does not yet expose a first-class declaration-list parse
    // entrypoint. Keep the wrapper localized here so inline style attributes
    // still flow through structured model declarations rather than the legacy
    // string-vector projection.
    let wrapped_rule = format!("* {{ {inline_style_text} }}");
    let parse = model::parse_stylesheet_with_options(&wrapped_rule, &ParseOptions::stylesheet());
    let Some(model::Rule::Style(rule)) = parse.stylesheet.rules.first() else {
        return Vec::new();
    };

    rule.declarations
        .declarations
        .iter()
        .enumerate()
        .map(|(declaration_index, declaration)| {
            let declaration_index = u32_index(declaration_index, "inline declaration");
            declaration_input_from_model(
                CascadeDeclarationSource::InlineStyle(InlineStyleDeclarationRef {
                    inline_style,
                    declaration_index,
                }),
                declaration_index,
                declaration,
            )
        })
        .collect()
}

fn declaration_input_from_model(
    source: CascadeDeclarationSource,
    declaration_order: u32,
    declaration: &model::Declaration,
) -> CascadeDeclarationInput {
    let importance = if declaration.important.is_some() {
        CascadeImportance::Important
    } else {
        CascadeImportance::Normal
    };
    let value = CascadeSpecifiedValue::from_declaration_value(&declaration.value);

    match declaration.name.kind {
        PropertyNameKind::Standard => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    value,
                );
            };
            if let Some(property) = property_registry().lookup_id(property_name) {
                CascadeDeclarationInput::supported(
                    source,
                    declaration_order,
                    importance,
                    property,
                    value,
                )
            } else {
                CascadeDeclarationInput::unsupported_property(
                    source,
                    declaration_order,
                    importance,
                    property_name,
                    value,
                )
            }
        }
        PropertyNameKind::Custom => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    value,
                );
            };
            CascadeDeclarationInput::custom_property(
                source,
                declaration_order,
                importance,
                property_name,
                value,
            )
        }
        PropertyNameKind::Invalid => CascadeDeclarationInput::invalid_property_name(
            source,
            declaration_order,
            importance,
            value,
        ),
    }
}

fn u32_index(index: usize, label: &str) -> u32 {
    u32::try_from(index).unwrap_or_else(|_| panic!("{label} index exceeds u32 range"))
}
