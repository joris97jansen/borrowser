use super::{
    ComputedDocumentStyle, ComputedStyle, ComputedStyleBuildError, ComputedStyleBuilder,
    ComputedStyleResolutionError, ComputedStyleReuseStats, ComputedValue,
    ComputedValueDiscriminant, ComputedValueNormalizationErrorKind, StylePlanExecution,
    build_style_tree, build_style_tree_from_computed_styles,
    build_style_tree_with_stylesheets as build_style_tree_with_stylesheets_with_environment,
    compute_document_styles as compute_document_styles_with_environment,
    compute_document_styles_from_resolved_styles,
    compute_document_styles_from_resolved_styles_with_reuse_stats,
    compute_document_styles_with_limits as compute_document_styles_with_limits_with_environment,
    compute_style, compute_style_from_resolved_style, normalize_specified_value,
};
use crate::{
    InitialStyleValue, ParseOptions, PropertyComputedValueKind, PropertyId, Rule,
    SpecifiedPropertyValue, SpecifiedToComputedConversionRule, StylesheetCascadeInput,
    parse_specified_value, parse_stylesheet_with_options, property_registry,
    property_value_boundary, resolve_cascade_style_from_rule_inputs,
    resolve_document_styles as resolve_document_styles_with_environment, resolve_initial_style,
    try_compute_document_styles_for_invalidation_plan_with_limits as try_compute_document_styles_for_invalidation_plan_with_limits_with_environment,
    values::{
        BorderStyle, Display, Length, LengthPercentage, OutlineStyle, Overflow, Percentage,
        Position, TextDecorationLine, ZIndex,
    },
};
use crate::{StyleChangeFacts, classify_style_invalidation};
use html::{Node, internal::Id};

use super::value::computed_value_discriminant;

fn compute_document_styles(
    root: &Node,
    sheets: &[crate::model::StylesheetParse],
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    compute_document_styles_with_environment(root, support::matching_environment(), sheets)
}

fn compute_document_styles_with_limits(
    root: &Node,
    sheets: &[crate::model::StylesheetParse],
    limits: &crate::StyleResolutionLimits,
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    compute_document_styles_with_limits_with_environment(
        root,
        support::matching_environment(),
        sheets,
        limits,
    )
}

fn resolve_document_styles(
    root: &Node,
    sheets: &[crate::model::StylesheetParse],
) -> Result<crate::ResolvedDocumentStyle, crate::StyleResolutionError> {
    resolve_document_styles_with_environment(root, support::matching_environment(), sheets)
}

fn try_compute_document_styles_for_invalidation_plan_with_limits(
    plan: &crate::StyleInvalidationPlan,
    root: &Node,
    sheets: &[StylesheetCascadeInput<'_>],
    previous: Option<(&crate::ResolvedDocumentStyle, &ComputedDocumentStyle)>,
    limits: &crate::StyleResolutionLimits,
) -> Result<StylePlanExecution, ComputedStyleResolutionError> {
    try_compute_document_styles_for_invalidation_plan_with_limits_with_environment(
        plan,
        root,
        support::matching_environment(),
        sheets,
        previous,
        limits,
    )
}

fn build_style_tree_with_stylesheets<'a>(
    root: &'a Node,
    sheets: &[crate::model::StylesheetParse],
) -> Result<super::StyledNode<'a>, ComputedStyleResolutionError> {
    build_style_tree_with_stylesheets_with_environment(
        root,
        support::matching_environment(),
        sheets,
    )
}

// Shared helpers.
mod support;

// Foundational computed-value and style contracts.
mod builder;
mod normalization;
mod style;

// Structured document and tree projections.
mod document;
mod style_tree;

// Compatibility bridge coverage.
mod legacy;
