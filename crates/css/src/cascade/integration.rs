use super::contract::{
    CascadeDeclarationInput, CascadeDeclarationSource, CascadeImportance, CascadeOrigin,
    CascadeRuleInput, CascadeRuleInputBuildError, CascadeRuleMatch, CascadeSpecifiedValue,
    InlineStyleDeclarationRef, InlineStyleRuleRef, StylesheetDeclarationRef,
    append_cascade_evaluation_debug_snapshot, resolve_cascade_style,
    resolve_cascade_style_from_rule_inputs,
};
use super::document::{ResolvedDocumentStyle, ResolvedElementStyle};
use crate::model::{self, PropertyNameKind};
use crate::selectors::{
    SelectorDomElementId, SelectorDomIndex, SelectorMatchingContext, SelectorMatchingLimitError,
    SelectorMatchingLimits,
};
use crate::syntax::ParseOptions;
use crate::{PropertyInvalidValuePolicy, property_registry};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleResolutionLimits {
    pub max_stylesheets_per_style_pass: usize,
    pub max_style_rules_per_document: usize,
    pub max_matched_rules_per_element: usize,
    pub max_declaration_inputs_per_element: usize,
    pub max_inline_style_bytes: usize,
    pub max_inline_declarations_per_element: usize,
    pub max_styled_elements_per_document: usize,
    pub selector_matching: SelectorMatchingLimits,
}

impl Default for StyleResolutionLimits {
    fn default() -> Self {
        Self {
            max_stylesheets_per_style_pass: 4_096,
            max_style_rules_per_document: 262_144,
            max_matched_rules_per_element: 4_096,
            max_declaration_inputs_per_element: 65_536,
            max_inline_style_bytes: 64 * 1024,
            max_inline_declarations_per_element: 1_024,
            max_styled_elements_per_document: 1_000_000,
            selector_matching: SelectorMatchingLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleResolutionLimit {
    StylesheetsPerStylePass,
    StyleRulesPerDocument,
    MatchedRulesPerElement,
    DeclarationInputsPerElement,
    InlineStyleBytes,
    InlineDeclarationsPerElement,
    StyledElementsPerDocument,
}

impl StyleResolutionLimit {
    pub fn stable_label(self) -> &'static str {
        match self {
            Self::StylesheetsPerStylePass => "stylesheets-per-style-pass",
            Self::StyleRulesPerDocument => "style-rules-per-document",
            Self::MatchedRulesPerElement => "matched-rules-per-element",
            Self::DeclarationInputsPerElement => "declaration-inputs-per-element",
            Self::InlineStyleBytes => "inline-style-bytes",
            Self::InlineDeclarationsPerElement => "inline-declarations-per-element",
            Self::StyledElementsPerDocument => "styled-elements-per-document",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleResolutionError {
    LimitExceeded {
        limit: StyleResolutionLimit,
        configured: usize,
    },
    UnsupportedConfiguration {
        limit: StyleResolutionLimit,
        configured: usize,
        max_supported: usize,
    },
    SelectorMatching(SelectorMatchingLimitError),
    RuleInputBuild(CascadeRuleInputBuildError),
}

impl StyleResolutionError {
    fn limit(limit: StyleResolutionLimit, configured: usize) -> Self {
        Self::LimitExceeded { limit, configured }
    }

    fn unsupported_configuration(
        limit: StyleResolutionLimit,
        configured: usize,
        max_supported: usize,
    ) -> Self {
        Self::UnsupportedConfiguration {
            limit,
            configured,
            max_supported,
        }
    }
}

impl std::fmt::Display for StyleResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LimitExceeded { limit, configured } => write!(
                f,
                "style resolution exceeded {} limit {}",
                limit.stable_label(),
                configured
            ),
            Self::UnsupportedConfiguration {
                limit,
                configured,
                max_supported,
            } => write!(
                f,
                "style resolution configured {} limit {} above representable maximum {}",
                limit.stable_label(),
                configured,
                max_supported
            ),
            Self::SelectorMatching(error) => write!(f, "{error}"),
            Self::RuleInputBuild(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for StyleResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SelectorMatching(error) => Some(error),
            Self::RuleInputBuild(error) => Some(error),
            Self::LimitExceeded { .. } | Self::UnsupportedConfiguration { .. } => None,
        }
    }
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

fn rule_inputs_for_element_with_limits(
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

fn stylesheet_declaration_inputs(
    stylesheet_index: u32,
    rule_index: u32,
    declarations: &[model::Declaration],
) -> Vec<CascadeDeclarationInput> {
    declarations
        .iter()
        .enumerate()
        .map(|(declaration_index, declaration)| {
            let declaration_index = u32_index(declaration_index);
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

fn enforce_stylesheet_limits(
    sheets: &[model::StylesheetParse],
    limits: &StyleResolutionLimits,
) -> Result<(), StyleResolutionError> {
    if sheets.len() > limits.max_stylesheets_per_style_pass {
        return Err(StyleResolutionError::limit(
            StyleResolutionLimit::StylesheetsPerStylePass,
            limits.max_stylesheets_per_style_pass,
        ));
    }
    enforce_style_rule_count(sheets, limits.max_style_rules_per_document)
}

fn enforce_style_rule_count(
    sheets: &[model::StylesheetParse],
    max_style_rules: usize,
) -> Result<(), StyleResolutionError> {
    let mut style_rules_seen = 0usize;
    for sheet in sheets {
        for rule in &sheet.stylesheet.rules {
            if !matches!(rule, model::Rule::Style(_)) {
                continue;
            }
            if style_rules_seen >= max_style_rules {
                return Err(StyleResolutionError::limit(
                    StyleResolutionLimit::StyleRulesPerDocument,
                    max_style_rules,
                ));
            }
            style_rules_seen += 1;
        }
    }
    Ok(())
}

fn validate_representation_limits(
    limits: &StyleResolutionLimits,
) -> Result<(), StyleResolutionError> {
    validate_u32_backed_limit(
        StyleResolutionLimit::StylesheetsPerStylePass,
        limits.max_stylesheets_per_style_pass,
    )?;
    validate_u32_backed_limit(
        StyleResolutionLimit::StyleRulesPerDocument,
        limits.max_style_rules_per_document,
    )?;
    validate_u32_backed_limit(
        StyleResolutionLimit::InlineDeclarationsPerElement,
        limits.max_inline_declarations_per_element,
    )?;
    validate_u32_backed_limit(
        StyleResolutionLimit::StyledElementsPerDocument,
        limits.max_styled_elements_per_document,
    )?;
    Ok(())
}

fn validate_u32_backed_limit(
    limit: StyleResolutionLimit,
    configured: usize,
) -> Result<(), StyleResolutionError> {
    let max_supported = u32::MAX as usize;
    if configured > max_supported {
        return Err(StyleResolutionError::unsupported_configuration(
            limit,
            configured,
            max_supported,
        ));
    }
    Ok(())
}

fn count_styled_elements_bounded(
    root: &Node,
    max_styled_elements: usize,
) -> Result<usize, StyleResolutionError> {
    let mut count = 0usize;
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        match node {
            Node::Document { children, .. } => {
                stack.extend(children.iter());
            }
            Node::Element { children, .. } => {
                if count >= max_styled_elements {
                    return Err(StyleResolutionError::limit(
                        StyleResolutionLimit::StyledElementsPerDocument,
                        max_styled_elements,
                    ));
                }
                count += 1;
                stack.extend(children.iter());
            }
            Node::Text { .. } | Node::Comment { .. } => {}
        }
    }

    Ok(count)
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

    match declaration.name.kind {
        PropertyNameKind::Standard => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                );
            };
            if let Some(property) = property_registry().lookup_id(property_name) {
                match CascadeSpecifiedValue::parse(property, &declaration.value) {
                    Ok(value) => CascadeDeclarationInput::supported(
                        source,
                        declaration_order,
                        importance,
                        property,
                        value,
                    ),
                    Err(error) => {
                        // Current supported properties only define strict
                        // declaration rejection. Keep policy dispatch here so
                        // any future invalid-value policy is added at the
                        // property/cascade boundary, not as computed-style or
                        // layout recovery.
                        match property.metadata().invalid_value_policy {
                            PropertyInvalidValuePolicy::RejectDeclaration => {
                                CascadeDeclarationInput::invalid_value(
                                    source,
                                    declaration_order,
                                    importance,
                                    property,
                                    error,
                                    CascadeSpecifiedValue::preserved(&declaration.value),
                                )
                            }
                        }
                    }
                }
            } else {
                CascadeDeclarationInput::unsupported_property(
                    source,
                    declaration_order,
                    importance,
                    property_name,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                )
            }
        }
        PropertyNameKind::Custom => {
            let Some(property_name) = declaration.name.text.as_deref() else {
                return CascadeDeclarationInput::invalid_property_name(
                    source,
                    declaration_order,
                    importance,
                    CascadeSpecifiedValue::preserved(&declaration.value),
                );
            };
            CascadeDeclarationInput::custom_property(
                source,
                declaration_order,
                importance,
                property_name,
                CascadeSpecifiedValue::preserved(&declaration.value),
            )
        }
        PropertyNameKind::Invalid => CascadeDeclarationInput::invalid_property_name(
            source,
            declaration_order,
            importance,
            CascadeSpecifiedValue::preserved(&declaration.value),
        ),
    }
}

fn u32_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}
