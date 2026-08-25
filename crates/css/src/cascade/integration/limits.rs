use super::super::contract::CascadeResolutionError;
use super::super::contract::CascadeRuleInputBuildError;
use super::collection::RuleCollectionBuildError;
use super::source::StylesheetCollectionInputBuildError;
use crate::cascade::contract::SourceCoordinateError;
use crate::selectors::{
    SelectorDomBuildError, SelectorMatchingEnvironment, SelectorMatchingLimitError,
    SelectorMatchingLimits,
};

/// Public style-execution limits. The derived AF6 cascade budget is an
/// internal CSS execution detail rather than a caller-constructed API.
///
/// ```compile_fail
/// use css::CascadeResolutionBudget;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleResolutionLimits {
    pub max_stylesheets_per_style_pass: usize,
    pub max_top_level_rules_per_document: usize,
    pub max_collected_declaration_inputs_per_document: usize,
    pub max_matched_rules_per_element: usize,
    pub max_declaration_inputs_per_element: usize,
    pub max_inline_style_bytes: usize,
    pub max_inline_declarations_per_element: usize,
    pub max_styled_elements_per_document: usize,
    pub max_selector_dependency_records_per_document: usize,
    pub max_selector_dependency_bytes_per_document: usize,
    pub max_selector_dependency_path_steps_per_document: usize,
    pub max_selector_dependency_evaluations_per_publication: usize,
    pub selector_matching: SelectorMatchingLimits,
}

impl Default for StyleResolutionLimits {
    fn default() -> Self {
        Self {
            max_stylesheets_per_style_pass: 4_096,
            max_top_level_rules_per_document: 262_144,
            max_collected_declaration_inputs_per_document: 1_048_576,
            max_matched_rules_per_element: 4_096,
            max_declaration_inputs_per_element: 65_536,
            max_inline_style_bytes: 64 * 1024,
            max_inline_declarations_per_element: 1_024,
            max_styled_elements_per_document: 1_000_000,
            max_selector_dependency_records_per_document: 1_048_576,
            max_selector_dependency_bytes_per_document: 64 * 1024 * 1024,
            max_selector_dependency_path_steps_per_document: 4_194_304,
            max_selector_dependency_evaluations_per_publication: 4_194_304,
            selector_matching: SelectorMatchingLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleResolutionLimit {
    StylesheetsPerStylePass,
    TopLevelRulesPerDocument,
    CollectedDeclarationInputsPerDocument,
    MatchedRulesPerElement,
    DeclarationInputsPerElement,
    InlineStyleBytes,
    InlineDeclarationsPerElement,
    StyledElementsPerDocument,
    SelectorDependencyRecordsPerDocument,
    SelectorDependencyBytesPerDocument,
    SelectorDependencyPathStepsPerDocument,
}

impl StyleResolutionLimit {
    pub fn stable_label(self) -> &'static str {
        match self {
            Self::StylesheetsPerStylePass => "stylesheets-per-style-pass",
            Self::TopLevelRulesPerDocument => "top-level-rules-per-document",
            Self::CollectedDeclarationInputsPerDocument => {
                "collected-declaration-inputs-per-document"
            }
            Self::MatchedRulesPerElement => "matched-rules-per-element",
            Self::DeclarationInputsPerElement => "declaration-inputs-per-element",
            Self::InlineStyleBytes => "inline-style-bytes",
            Self::InlineDeclarationsPerElement => "inline-declarations-per-element",
            Self::StyledElementsPerDocument => "styled-elements-per-document",
            Self::SelectorDependencyRecordsPerDocument => {
                "selector-dependency-records-per-document"
            }
            Self::SelectorDependencyBytesPerDocument => "selector-dependency-bytes-per-document",
            Self::SelectorDependencyPathStepsPerDocument => {
                "selector-dependency-path-steps-per-document"
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleResolutionError {
    SelectorDomBuild(SelectorDomBuildError),
    MatchingEnvironmentMismatch {
        expected: SelectorMatchingEnvironment,
        actual: SelectorMatchingEnvironment,
    },
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
    StylesheetInputBuild(StylesheetCollectionInputBuildError),
    SourceCoordinate(SourceCoordinateError),
    RuleCollectionBuild(RuleCollectionBuildError),
    CascadeResolution(CascadeResolutionError),
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

    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::SelectorDomBuild(_) => "selector-dom-build",
            Self::MatchingEnvironmentMismatch { .. } => "matching-environment-mismatch",
            Self::LimitExceeded { .. } => "limit-exceeded",
            Self::UnsupportedConfiguration { .. } => "unsupported-configuration",
            Self::SelectorMatching(_) => "selector-matching",
            Self::RuleInputBuild(_) => "rule-input-build",
            Self::StylesheetInputBuild(error) => error.stable_label(),
            Self::SourceCoordinate(error) => error.stable_label(),
            Self::RuleCollectionBuild(error) => error.stable_label(),
            Self::CascadeResolution(error) => error.stable_label(),
        }
    }
}

impl std::fmt::Display for StyleResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelectorDomBuild(error) => write!(f, "{error}"),
            Self::MatchingEnvironmentMismatch { expected, actual } => write!(
                f,
                "resolved style matching environment mismatch: expected document mode {}, got {}",
                expected.document_mode(),
                actual.document_mode()
            ),
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
            Self::StylesheetInputBuild(error) => write!(f, "{error}"),
            Self::SourceCoordinate(error) => {
                write!(f, "style execution source coordinate: {error}")
            }
            Self::RuleCollectionBuild(error) => write!(f, "{error}"),
            Self::CascadeResolution(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for StyleResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SelectorDomBuild(error) => Some(error),
            Self::SelectorMatching(error) => Some(error),
            Self::RuleInputBuild(error) => Some(error),
            Self::StylesheetInputBuild(error) => Some(error),
            Self::SourceCoordinate(error) => Some(error),
            Self::RuleCollectionBuild(error) => Some(error),
            Self::CascadeResolution(error) => Some(error),
            Self::MatchingEnvironmentMismatch { .. }
            | Self::LimitExceeded { .. }
            | Self::UnsupportedConfiguration { .. } => None,
        }
    }
}

pub(super) fn validate_representation_limits(
    limits: &StyleResolutionLimits,
) -> Result<(), StyleResolutionError> {
    validate_u32_backed_limit(
        StyleResolutionLimit::StylesheetsPerStylePass,
        limits.max_stylesheets_per_style_pass,
    )?;
    validate_u32_backed_limit(
        StyleResolutionLimit::TopLevelRulesPerDocument,
        limits.max_top_level_rules_per_document,
    )?;
    validate_u32_backed_limit(
        StyleResolutionLimit::CollectedDeclarationInputsPerDocument,
        limits.max_collected_declaration_inputs_per_document,
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
