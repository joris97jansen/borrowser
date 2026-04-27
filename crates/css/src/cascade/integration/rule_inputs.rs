use super::super::contract::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeOrigin, CascadeRuleInput,
    CascadeRuleMatch, InlineStyleDeclarationRef, InlineStyleRuleRef,
};
use super::declarations::{declaration_input_from_model, stylesheet_declaration_inputs, u32_index};
use super::limits::{StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits};
use crate::model;
use crate::selectors::{SelectorDomElementId, SelectorDomIndex, SelectorMatchingContext};
use crate::syntax::ParseOptions;

pub(super) fn rule_inputs_for_element_with_limits(
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    element: SelectorDomElementId,
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<Vec<CascadeRuleInput>, StyleResolutionError> {
    let mut rule_inputs = Vec::new();
    let mut rule_order = 0u32;
    let mut matched_rules = 0usize;
    let mut declaration_inputs = 0usize;

    for (stylesheet_index, sheet) in sheets.iter().enumerate() {
        let stylesheet_index = u32_index(stylesheet_index);

        for (rule_index, rule) in sheet.stylesheet.rules.iter().enumerate() {
            let rule_index = u32_index(rule_index);
            let model::Rule::Style(rule) = rule else {
                continue;
            };

            let current_rule_order = rule_order;
            rule_order = rule_order.saturating_add(1);

            let rule_match = CascadeRuleMatch {
                stylesheet_index,
                rule_index,
                outcome: context
                    .match_selector_list_checked(element, &rule.selectors)
                    .map_err(StyleResolutionError::SelectorMatching)?,
            };

            if !rule_match.contributes_candidates() {
                continue;
            }

            if matched_rules >= limits.max_matched_rules_per_element {
                return Err(StyleResolutionError::limit(
                    StyleResolutionLimit::MatchedRulesPerElement,
                    limits.max_matched_rules_per_element,
                ));
            }

            matched_rules += 1;

            let declarations = stylesheet_declaration_inputs(
                stylesheet_index,
                rule_index,
                &rule.declarations.declarations,
            );

            if declarations.is_empty() {
                continue;
            }

            declaration_inputs = declaration_inputs.saturating_add(declarations.len());

            if declaration_inputs > limits.max_declaration_inputs_per_element {
                return Err(StyleResolutionError::limit(
                    StyleResolutionLimit::DeclarationInputsPerElement,
                    limits.max_declaration_inputs_per_element,
                ));
            }

            if let Some(rule_input) = CascadeRuleInput::from_stylesheet_match(
                &rule_match,
                CascadeOrigin::Author,
                current_rule_order,
                declarations,
            )
            .map_err(StyleResolutionError::RuleInputBuild)?
            {
                rule_inputs.push(rule_input);
            }
        }
    }

    let inline_rule_order = rule_order;

    if let Some(inline_style) = context.attribute_value(element, "style")
        && let Some(rule_input) =
            inline_style_rule_input(element, inline_rule_order, inline_style, limits)?
    {
        rule_inputs.push(rule_input);
    }

    Ok(rule_inputs)
}

fn inline_style_rule_input(
    element: SelectorDomElementId,
    rule_order: u32,
    inline_style_text: &str,
    limits: &StyleResolutionLimits,
) -> Result<Option<CascadeRuleInput>, StyleResolutionError> {
    if inline_style_text.trim().is_empty() {
        return Ok(None);
    }

    if inline_style_text.len() > limits.max_inline_style_bytes {
        return Err(StyleResolutionError::limit(
            StyleResolutionLimit::InlineStyleBytes,
            limits.max_inline_style_bytes,
        ));
    }

    let inline_style = InlineStyleRuleRef::new(element.get());
    let declarations = inline_style_declaration_inputs(inline_style, inline_style_text);

    if declarations.len() > limits.max_inline_declarations_per_element {
        return Err(StyleResolutionError::limit(
            StyleResolutionLimit::InlineDeclarationsPerElement,
            limits.max_inline_declarations_per_element,
        ));
    }

    if declarations.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        CascadeRuleInput::from_inline_style(inline_style, rule_order, declarations)
            .map_err(StyleResolutionError::RuleInputBuild)?,
    ))
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
            let declaration_index = u32_index(declaration_index);
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
