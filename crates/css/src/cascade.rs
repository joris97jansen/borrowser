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
#[cfg(any(test, feature = "css-fuzzing"))]
pub mod fuzz;
mod integration;
mod legacy_bridge;

#[cfg(test)]
mod tests;

// Property metadata and defaults
pub use contract::{
    CascadeInheritance, CascadePropertyId, CascadePropertyInvalidationImpact,
    CascadePropertyLengthSignPolicy, CascadePropertyMetadata, CascadePropertyRegistration,
    CascadePropertyRegistry, CascadeShorthandId, CascadeShorthandRegistration,
    CascadeShorthandRegistry, InitialStyleValue, cascade_property_registry,
    cascade_property_registry_metadata_debug_snapshot, cascade_shorthand_registry,
};

// Origin and precedence
pub use contract::{
    CascadeDeclarationPrecedence, CascadeImportance, CascadeOrigin, CascadeOriginBand,
    CascadePriority, CurrentScopeCascadePriorityBand, DeclarationOrder, DeclarationSourceIndex,
    RawRuleIndex, SourceCoordinateError, StyleRulePosition, StylesheetOrder, StylesheetRuleOrder,
    StylesheetSourceId, StylesheetSourceIdError,
};

// Rule and declaration inputs
pub use contract::{
    CascadeDeclarationApplicability, CascadeDeclarationInput, CascadeDeclarationProperty,
    CascadeDeclarationSource, CascadeRuleContext, CascadeRuleInput, CascadeRuleInputBuildError,
    CascadeRuleMatch, CascadeRuleSource, CascadeSpecifiedValue, InlineStyleDeclarationRef,
    InlineStyleRuleInput, InlineStyleRuleRef, MatchedStylesheetRuleInput, StylesheetDeclarationRef,
    StylesheetRuleRef,
};

// Winner resolution and snapshots
pub use contract::{
    CandidateDataMismatch, CascadeResolutionError, CascadeWinner, CascadeWinnerEntry,
    CascadeWinnerSet, RuleInputSequenceViolation,
};

// Resolved-style contract
#[cfg(test)]
pub use contract::resolve_cascade_style_from_rule_inputs;
pub use contract::{
    CssWideResolvedSource, ResolvedStyle, ResolvedStyleEntry, ResolvedValueSource,
    resolve_cascade_style, resolve_initial_style,
};

// Document-level structured output
pub use document::{ResolvedDocumentStyle, ResolvedElementStyle};

// Document-resolution integration path
pub(crate) use integration::StylesheetConditionStatus;
#[cfg(test)]
pub(crate) use integration::resolve_document_styles_debug_snapshot;
#[cfg(feature = "count-alloc")]
#[doc(hidden)]
pub use integration::{
    Af5AllocationGuardError, Af6CascadeWorkspaceStats, af5_match_rule_inputs_for_allocation_guard,
    af6_resolve_winners_for_allocation_guard,
};
pub use integration::{
    AtRuleSkipReason, BoundedDiagnosticText, CASCADE_EVALUATION_DIAGNOSTIC_VERSION,
    CascadeDiagnosticCandidateId, CascadeDiagnosticText, CascadeEvaluationCandidateRecord,
    CascadeEvaluationDiagnostic, CascadeEvaluationDiagnosticFailure,
    CascadeEvaluationDiagnosticLimit, CascadeEvaluationDiagnosticLimits,
    CascadeEvaluationDiagnosticSnapshot, CascadeEvaluationWinnerRecord, DiagnosticCondition,
    DiagnosticDeclarationProperty, DiagnosticRuleState, IncrementalResolvedDocumentStyle,
    IncrementalStyleResolutionStats, ProjectionComputedDocumentStyle,
    ProjectionResolvedDocumentStyle, RULE_COLLECTION_DIAGNOSTIC_VERSION, RuleCollection,
    RuleCollectionBuildError, RuleCollectionDiagnostic, RuleCollectionDiagnosticFailure,
    RuleCollectionDiagnosticLimit, RuleCollectionDiagnosticLimits, RuleCollectionDiagnosticRecord,
    RuleCollectionDiagnosticSnapshot, RuleCollectionDiagnosticStorage, RuleCollectionStorage,
    StyleProjectionArtifactError, StyleResolutionError, StyleResolutionExecution,
    StyleResolutionLimit, StyleResolutionLimits, StylesheetCollectionInput,
    StylesheetCollectionInputBuildError, StylesheetConditionInput, cascade_evaluation_diagnostic,
    get_inline_style, is_css, resolve_document_styles, resolve_document_styles_from_cascade_inputs,
    rule_collection_diagnostic, try_resolve_document_styles_from_cascade_inputs_with_limits,
    try_resolve_document_styles_from_rule_collection_with_limits,
    try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
    try_resolve_document_styles_incremental_suffix_from_rule_collection_with_limits,
    try_resolve_document_styles_incremental_suffix_with_limits,
    try_resolve_document_styles_with_limits,
};
pub(crate) use integration::{CollectedRule, InactiveStyleRuleReason};

// Legacy compatibility bridge
pub use legacy_bridge::{LegacyStyleAttachmentError, attach_styles, try_attach_styles};
