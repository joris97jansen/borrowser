use super::super::contract::CascadeRuleInputBuildError;
use crate::model;
use crate::selectors::{SelectorMatchingLimitError, SelectorMatchingLimits};
use html::Node;

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
    pub(super) fn limit(limit: StyleResolutionLimit, configured: usize) -> Self {
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

pub(super) fn enforce_stylesheet_limits(
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

pub(super) fn validate_representation_limits(
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

pub(super) fn count_styled_elements_bounded(
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
