use super::super::contract::{
    CascadeResolutionBudget, CascadeRuleInput, InlineStyleRuleRef,
    ValidatedCascadeRuleInputBuilder, ValidatedCascadeRuleInputs,
};
use super::collection::{ActiveCollectedStyleRule, CollectedRule, RuleCollection};
use super::declarations::inline_style_declaration_inputs_from_model;
use super::limits::{StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits};
use super::source::get_inline_style;
use crate::model;
use crate::selectors::{
    SelectorDomElementId, SelectorDomIndex, SelectorMatchDom, SelectorMatchingContext,
};
use crate::syntax::ParseOptions;

pub(super) fn rule_inputs_for_element_with_limits<'collection, 'source>(
    dom: &SelectorDomIndex<'_>,
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    element: SelectorDomElementId,
    collection: &'collection RuleCollection<'source>,
    limits: &StyleResolutionLimits,
    budget: CascadeResolutionBudget,
) -> Result<ValidatedCascadeRuleInputs<'collection>, StyleResolutionError> {
    rule_inputs_for_element_with_observer(
        dom,
        context,
        element,
        collection,
        limits,
        budget,
        |_, _| {},
    )
}

pub(super) fn rule_inputs_for_element_with_observer<'collection, 'source>(
    dom: &SelectorDomIndex<'_>,
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    element: SelectorDomElementId,
    collection: &'collection RuleCollection<'source>,
    limits: &StyleResolutionLimits,
    budget: CascadeResolutionBudget,
    mut observer: impl FnMut(
        &ActiveCollectedStyleRule<'source>,
        &crate::selectors::SelectorListMatchOutcome,
    ),
) -> Result<ValidatedCascadeRuleInputs<'collection>, StyleResolutionError> {
    let mut rule_inputs = ValidatedCascadeRuleInputBuilder::new(budget);
    let mut matched_rules = 0usize;
    let mut declaration_inputs = 0usize;

    for collected in collection.rules() {
        let CollectedRule::ActiveStyle(rule) = collected else {
            continue;
        };
        let selector_context = context.with_namespace_constraint(rule.namespace_constraint());
        let outcome = selector_context
            .match_parsed_selector_list_checked(element, rule.selectors())
            .map_err(StyleResolutionError::SelectorMatching)?;
        observer(rule, &outcome);
        if !outcome.matched_any() {
            continue;
        }

        if matched_rules >= limits.max_matched_rules_per_element {
            return Err(StyleResolutionError::limit(
                StyleResolutionLimit::MatchedRulesPerElement,
                limits.max_matched_rules_per_element,
            ));
        }
        matched_rules += 1;

        let declarations = collection.declarations_for_rule(rule);
        let Some(remaining_declaration_inputs) = limits
            .max_declaration_inputs_per_element
            .checked_sub(declaration_inputs)
        else {
            return Err(StyleResolutionError::limit(
                StyleResolutionLimit::DeclarationInputsPerElement,
                limits.max_declaration_inputs_per_element,
            ));
        };
        if declarations.len() > remaining_declaration_inputs {
            return Err(StyleResolutionError::limit(
                StyleResolutionLimit::DeclarationInputsPerElement,
                limits.max_declaration_inputs_per_element,
            ));
        }
        declaration_inputs += declarations.len();

        if let Some(input) = CascadeRuleInput::from_validated_stylesheet_match(
            rule.rule_ref(),
            rule.origin(),
            rule.source_order(),
            outcome,
            declarations,
        ) {
            rule_inputs
                .try_reserve_rule_inputs(1)
                .and_then(|()| rule_inputs.push_stylesheet(input))
                .map_err(StyleResolutionError::CascadeResolution)?;
        }
    }

    if let Some(inline_style) =
        get_inline_style(dom.element_namespace(element), dom.attributes(element))
        && let Some(rule_input) = inline_style_rule_input(element, inline_style, limits)?
    {
        rule_inputs
            .try_reserve_rule_inputs(1)
            .and_then(|()| rule_inputs.push_inline(rule_input))
            .map_err(StyleResolutionError::CascadeResolution)?;
    }

    Ok(rule_inputs.finish())
}

fn inline_style_rule_input<'collection>(
    element: SelectorDomElementId,
    inline_style_text: &str,
    limits: &StyleResolutionLimits,
) -> Result<Option<CascadeRuleInput<'collection>>, StyleResolutionError> {
    if inline_style_text.trim().is_empty() {
        return Ok(None);
    }

    if inline_style_text.len() > limits.max_inline_style_bytes {
        return Err(StyleResolutionError::limit(
            StyleResolutionLimit::InlineStyleBytes,
            limits.max_inline_style_bytes,
        ));
    }

    let inline_style = InlineStyleRuleRef::from_selector_element(element);
    let parse = model::parse_declaration_list_with_options(
        inline_style_text,
        &ParseOptions::style_attribute(),
    );
    let declarations =
        inline_style_declaration_inputs_from_model(inline_style, &parse.declarations)
            .map_err(StyleResolutionError::SourceCoordinate)?;

    if declarations.len() > limits.max_inline_declarations_per_element {
        return Err(StyleResolutionError::limit(
            StyleResolutionLimit::InlineDeclarationsPerElement,
            limits.max_inline_declarations_per_element,
        ));
    }

    if declarations.is_empty() {
        return Ok(None);
    }

    Ok(Some(CascadeRuleInput::from_validated_inline_style(
        inline_style,
        declarations,
    )))
}
