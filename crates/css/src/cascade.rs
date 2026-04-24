//! CSS cascade resolved-style engine plus the legacy compatibility projection.
//!
//! The Milestone R cascade engine resolves structured declaration winners into
//! deterministic resolved-style objects. The core per-element contract is
//! defined by the `contract` submodule below; this module adds the current
//! document-level integration path that consumes DOM selector matches and
//! stylesheet model data.
//!
//! `attach_styles` remains only as a legacy projection from structured
//! resolved styles into `html::Node::style` so the pre-R computed-style and
//! layout path can continue to run while the computed-value cutover is still in
//! progress.

mod contract;
mod document;
mod integration;
mod legacy_bridge;

#[cfg(test)]
mod tests;

// Property metadata and defaults
pub use contract::{
    CascadeInheritance, CascadePropertyId, CascadePropertyLengthSignPolicy,
    CascadePropertyMetadata, CascadePropertyRegistration, CascadePropertyRegistry,
    InitialStyleValue, cascade_property_registry,
};

// Origin and precedence
pub use contract::{
    CascadeImportance, CascadeOrigin, CascadeOriginBand, CascadePriority, CascadeSpecificity,
    CurrentScopeCascadePriorityBand,
};

// Rule and declaration inputs
pub use contract::{
    CascadeDeclarationApplicability, CascadeDeclarationCandidate, CascadeDeclarationCandidateKey,
    CascadeDeclarationInput, CascadeDeclarationProperty, CascadeDeclarationSource,
    CascadeRuleContext, CascadeRuleInput, CascadeRuleInputBuildError, CascadeRuleMatch,
    CascadeRuleSource, CascadeSpecifiedValue, InlineStyleDeclarationRef, InlineStyleRuleRef,
    StylesheetDeclarationRef, StylesheetRuleRef,
};

// Winner resolution and snapshots
pub use contract::{
    CascadeWinner, CascadeWinnerEntry, CascadeWinnerSet, cascade_evaluation_debug_snapshot,
    resolve_cascade_winners, resolve_cascade_winners_from_rule_inputs,
    sort_candidates_by_cascade_order,
};

// Resolved-style contract
pub use contract::{
    ResolvedStyle, ResolvedStyleBuildError, ResolvedStyleBuilder, ResolvedStyleEntry,
    ResolvedValueSource, resolve_cascade_style, resolve_cascade_style_from_rule_inputs,
    resolve_initial_style,
};

// Document-level structured output
pub use document::{ResolvedDocumentStyle, ResolvedElementStyle};

// Document-resolution integration path
pub use integration::{
    StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits, get_inline_style, is_css,
    resolve_document_styles, resolve_document_styles_debug_snapshot,
    try_resolve_document_styles_with_limits,
};

// Legacy compatibility bridge
pub use legacy_bridge::attach_styles;
