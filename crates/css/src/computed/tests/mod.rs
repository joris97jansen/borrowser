use super::{
    ComputedStyle, ComputedStyleBuildError, ComputedStyleBuilder, ComputedStyleResolutionError,
    ComputedStyleReuseStats, ComputedValue, ComputedValueDiscriminant,
    ComputedValueNormalizationErrorKind, build_style_tree, build_style_tree_from_computed_styles,
    build_style_tree_with_stylesheets, compute_document_styles,
    compute_document_styles_from_resolved_styles,
    compute_document_styles_from_resolved_styles_with_reuse_stats,
    compute_document_styles_with_limits, compute_style, compute_style_from_resolved_style,
    normalize_specified_value,
};
use crate::{
    InitialStyleValue, ParseOptions, PropertyComputedValueKind, PropertyId, Rule,
    SpecifiedPropertyValue, parse_specified_value, parse_stylesheet_with_options,
    property_registry, resolve_cascade_style_from_rule_inputs, resolve_document_styles,
    resolve_initial_style,
    values::{
        BorderStyle, Display, Length, LengthPercentage, OutlineStyle, Overflow, Percentage,
        Position,
    },
};
use html::{Node, internal::Id};
use std::sync::Arc;

use super::value::computed_value_discriminant;

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
