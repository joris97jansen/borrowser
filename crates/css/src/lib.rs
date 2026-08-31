//! Public CSS crate surface.
//!
//! The crate root is model-first for whole-stylesheet parsing:
//! `parse_stylesheet(...)` and `parse_stylesheet_with_options(...)` produce the
//! engine-facing `css::model` parse result.
//!
//! Syntax-layer and compatibility-scoped APIs remain available explicitly for
//! parser work, migration support, and golden tests, but they are no longer the
//! preferred crate-root contract for new engine-facing CSS code.

#[cfg(feature = "css-fuzzing")]
pub mod fuzz_regressions;
#[cfg(any(test, feature = "css-fuzzing"))]
mod fuzz_support;

pub mod cascade;
pub mod computed;
mod document_selector_matching;
mod dom_attributes;
pub mod model;
pub mod properties;
pub mod selectors;
pub mod specified;
pub mod style_invalidation;
pub mod syntax;
pub mod values;

#[cfg(any(test, feature = "count-alloc", feature = "perf-tests"))]
pub mod perf_fixtures;

#[cfg(all(test, feature = "perf-tests"))]
mod perf_guards_heavy;
#[cfg(test)]
mod perf_guards_smoke;

// Model-first crate-root surface for engine-facing stylesheet work.
#[cfg(test)]
pub use cascade::resolve_cascade_style_from_rule_inputs;
#[cfg(test)]
pub(crate) use cascade::resolve_document_styles_debug_snapshot;
#[cfg(feature = "count-alloc")]
#[doc(hidden)]
pub use cascade::{
    Af5AllocationGuardError, Af6CascadeWorkspaceStats, af5_match_rule_inputs_for_allocation_guard,
    af6_resolve_winners_for_allocation_guard,
};
pub use cascade::{
    AtRuleSkipReason, BoundedDiagnosticText, CASCADE_EVALUATION_DIAGNOSTIC_VERSION,
    CandidateDataMismatch, CascadeDeclarationApplicability, CascadeDeclarationInput,
    CascadeDeclarationPrecedence, CascadeDeclarationProperty, CascadeDeclarationSource,
    CascadeDiagnosticCandidateId, CascadeDiagnosticText, CascadeEvaluationCandidateRecord,
    CascadeEvaluationDiagnostic, CascadeEvaluationDiagnosticFailure,
    CascadeEvaluationDiagnosticLimit, CascadeEvaluationDiagnosticLimits,
    CascadeEvaluationDiagnosticSnapshot, CascadeEvaluationWinnerRecord, CascadeImportance,
    CascadeInheritance, CascadeOrigin, CascadeOriginBand, CascadePriority, CascadePropertyId,
    CascadePropertyInvalidationImpact, CascadePropertyLengthSignPolicy, CascadePropertyMetadata,
    CascadePropertyRegistration, CascadePropertyRegistry, CascadeResolutionError,
    CascadeRuleContext, CascadeRuleInput, CascadeRuleInputBuildError, CascadeRuleMatch,
    CascadeRuleSource, CascadeShorthandId, CascadeShorthandRegistration, CascadeShorthandRegistry,
    CascadeSpecifiedValue, CascadeWinner, CascadeWinnerEntry, CascadeWinnerSet,
    CssWideResolvedSource, CurrentScopeCascadePriorityBand, DeclarationOrder,
    DeclarationSourceIndex, DiagnosticCondition, DiagnosticDeclarationProperty,
    DiagnosticRuleState, IncrementalResolvedDocumentStyle, IncrementalStyleResolutionStats,
    InitialStyleValue, InlineStyleDeclarationRef, InlineStyleRuleInput, InlineStyleRuleRef,
    LegacyStyleAttachmentError, MatchedStylesheetRuleInput, ProjectionComputedDocumentStyle,
    ProjectionResolvedDocumentStyle, RULE_COLLECTION_DIAGNOSTIC_VERSION, RawRuleIndex,
    ResolvedDocumentStyle, ResolvedElementStyle, ResolvedStyle, ResolvedStyleEntry,
    ResolvedValueSource, RuleCollection, RuleCollectionBuildError, RuleCollectionDiagnostic,
    RuleCollectionDiagnosticFailure, RuleCollectionDiagnosticLimit, RuleCollectionDiagnosticLimits,
    RuleCollectionDiagnosticRecord, RuleCollectionDiagnosticSnapshot,
    RuleCollectionDiagnosticStorage, RuleCollectionStorage, RuleInputSequenceViolation,
    SourceCoordinateError, StylePhaseExecutionError, StyleProjectionArtifactError,
    StyleResolutionError, StyleResolutionExecution, StyleResolutionLimit, StyleResolutionLimits,
    StyleRulePosition, StylesheetCollectionInput, StylesheetCollectionInputBuildError,
    StylesheetConditionInput, StylesheetDeclarationRef, StylesheetOrder, StylesheetRuleOrder,
    StylesheetRuleRef, StylesheetSourceId, StylesheetSourceIdError, attach_styles,
    cascade_evaluation_diagnostic, cascade_property_registry,
    cascade_property_registry_metadata_debug_snapshot, cascade_shorthand_registry,
    get_inline_style, is_css, resolve_cascade_style, resolve_document_styles,
    resolve_document_styles_from_cascade_inputs, resolve_initial_style, rule_collection_diagnostic,
    try_attach_styles, try_build_style_phase_output_from_cascade_inputs_with_limits,
    try_resolve_document_styles_from_cascade_inputs_with_limits,
    try_resolve_document_styles_from_rule_collection_with_limits,
    try_resolve_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
    try_resolve_document_styles_incremental_suffix_from_rule_collection_with_limits,
    try_resolve_document_styles_incremental_suffix_with_limits,
    try_resolve_document_styles_with_limits,
};
pub use computed::{
    BorderEdges, BorderSide, BoxMetrics, ComputedDocumentStyle,
    ComputedDocumentStyleInvalidationImpact, ComputedDocumentStyleWithStats, ComputedElementStyle,
    ComputedStyleBuildError, ComputedStyleBuilder, ComputedStyleEntry,
    ComputedStyleInvalidationImpact, ComputedStyleResolutionError, ComputedStyleReuseStats,
    ComputedValue, ComputedValueDiscriminant, ComputedValueNormalizationError,
    ComputedValueNormalizationErrorKind, IncrementalComputedDocumentStyle, StylePhaseOutput,
    StylePlanExecution, build_style_tree_from_computed_styles, build_style_tree_with_stylesheets,
    compute_document_styles, compute_document_styles_from_resolved_styles,
    compute_document_styles_from_resolved_styles_with_reuse_stats,
    compute_document_styles_incremental_suffix_from_cascade_inputs_with_limits,
    compute_document_styles_incremental_suffix_from_execution_with_limits,
    compute_document_styles_incremental_suffix_from_rule_collection_with_limits,
    compute_document_styles_incremental_suffix_with_limits, compute_document_styles_with_limits,
    compute_style_from_resolved_style, computed_value_debug_snapshot, normalize_specified_value,
    property_invalidation_classification_debug_snapshot,
    try_compute_document_styles_for_invalidation_plan_from_execution_with_limits,
    try_compute_document_styles_for_invalidation_plan_from_rule_collection_with_limits,
    try_compute_document_styles_for_invalidation_plan_with_limits,
};
pub use computed::{ComputedStyle, StyledNode, build_style_tree, compute_style};
pub use document_selector_matching::{
    DocumentSelectorMatchingDiagnostic, DocumentSelectorMatchingDiagnosticFailure,
    DocumentSelectorMatchingDiagnosticLimit, DocumentSelectorMatchingDiagnosticLimits,
    DocumentSelectorMatchingDiagnosticStorage, SelectorDiagnosticCondition,
    document_selector_matching_diagnostic,
};
pub use model::{
    AtRule, AtRuleBlock, Declaration, DeclarationBlock, DeclarationValue, ImportantAnnotation,
    PreservedBlock, PreservedComponentList, PropertyName, PropertyNameKind, Rule, StyleRule,
    Stylesheet, StylesheetParse, ValueBlock, ValueComponent, ValueFunction, ValueSymbol, ValueText,
    ValueToken, parse_stylesheet, parse_stylesheet_with_options,
    serialize_declaration_for_snapshot, serialize_rule_for_snapshot,
    serialize_stylesheet_for_snapshot, serialize_stylesheet_parse_for_snapshot,
    serialize_value_for_snapshot,
};
pub use properties::{
    PropertyComputedValueKind, PropertyId, PropertyInheritance, PropertyInvalidValuePolicy,
    PropertyInvalidationImpact, PropertyLengthSignPolicy, PropertyMetadata, PropertyRegistration,
    PropertyRegistry, PropertySpecifiedValueKind, PropertyValueBoundary, ShorthandId,
    ShorthandRegistration, ShorthandRegistry, SpecifiedToComputedConversionRule,
    property_coverage_debug_snapshot, property_registry, property_registry_metadata_debug_snapshot,
    property_value_boundaries, property_value_boundary, property_value_boundary_debug_snapshot,
    shorthand_registry, shorthand_registry_debug_snapshot,
};
pub use selectors::{
    AncestorElements, AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher,
    AttributeSelector, AttributeValue, ClassSelector, Combinator, CombinedSelector,
    ComplexSelector, CompoundSelector, ElementChildren, IdSelector, InvalidSelectorList,
    InvalidSelectorReason, MatchedSelector, NamedTypeSelector, NextSiblingElements,
    PreviousSiblingElements, SelectorDomAttribute, SelectorDomBuildError, SelectorDomBuildStorage,
    SelectorDomElementId, SelectorDomElementIter, SelectorDomIndex, SelectorDomNodeKind,
    SelectorIdent, SelectorList, SelectorListMatchBuilder, SelectorListMatchOutcome,
    SelectorListParseResult, SelectorMatchDom, SelectorMatchability, SelectorMatchingContext,
    SelectorMatchingEnvironment, SelectorMatchingLimitError, SelectorMatchingLimits,
    SelectorNamespaceConstraint, SelectorSnapshotSerializationError, SelectorString,
    SelectorStructureError, Specificity, StyleProjection, StyleProjectionBuildError,
    StyleProjectionElementKey, StyleProjectionKeyError, StyleProjectionMatchError,
    SubclassSelector, TypeSelector, UniversalSelector, UnsupportedSelectorFeature,
    UnsupportedSelectorHandling, UnsupportedSelectorList, parse_selector_list,
    parse_selector_list_with_limits, parse_selector_source, parse_selector_source_with_limits,
    serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot,
    serialize_selector_parse_result_for_snapshot_bounded,
};
pub use specified::{
    ExpandedLonghandDeclaration, ShorthandExpansion, ShorthandExpansionError,
    ShorthandExpansionErrorKind, SpecifiedBorderStyle, SpecifiedBorderStyleKeyword, SpecifiedColor,
    SpecifiedColorKeyword, SpecifiedColorSyntax, SpecifiedDeclarationValue, SpecifiedDisplay,
    SpecifiedDisplayKeyword, SpecifiedHexColor, SpecifiedLength, SpecifiedLengthPercentage,
    SpecifiedLengthPercentageOrAuto, SpecifiedLengthPercentageOrNone, SpecifiedLengthUnit,
    SpecifiedOutlineStyle, SpecifiedOutlineStyleKeyword, SpecifiedOverflow,
    SpecifiedOverflowKeyword, SpecifiedPercentage, SpecifiedPosition, SpecifiedPositionKeyword,
    SpecifiedPropertyValue, SpecifiedTextDecorationLine, SpecifiedTextDecorationLineKeyword,
    SpecifiedValue, SpecifiedValueLimits, SpecifiedValueParseError, SpecifiedValueParseErrorKind,
    SpecifiedZIndex, SpecifiedZIndexValue, expand_shorthand_declaration,
    parse_specified_declaration_value, parse_specified_declaration_value_with_limits,
    parse_specified_value, parse_specified_value_with_limits, shorthand_expansion_debug_snapshot,
};
pub use style_invalidation::{
    ChangedStyleNodeFacts, DomStyleAttributeMutation, DomStyleChangeFacts,
    DomStyleChangeFactsBuilder, DomStyleTextMutation, STYLE_DEPENDENCY_ARTIFACT_DEBUG_VERSION,
    StyleChangeFacts, StyleDependencyArtifact, StyleInvalidationDecision, StyleInvalidationInput,
    StyleInvalidationPlan, classify_style_invalidation,
    classify_style_invalidation_with_dependencies, merge_style_invalidation_plans,
};

// Explicit syntax-layer surface for parser/tokenizer work and syntax tests.
pub use syntax::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclaration, CssDeclarationBlock, CssDimension,
    CssFunction, CssHashKind, CssInput, CssInputId, CssNumber, CssNumericKind, CssParseOrigin,
    CssPosition, CssQualifiedRule, CssRule, CssSimpleBlock, CssSpan, CssStylesheet, CssToken,
    CssTokenKind, CssTokenText, CssTokenization, CssTokenizationStats, CssUnicodeRange,
    DeclarationListParse, DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats,
    RecoveryPolicy, StylesheetParse as SyntaxStylesheetParse, SyntaxDiagnostic, SyntaxLimits,
    parse_declarations, parse_declarations_with_options,
    parse_stylesheet as parse_syntax_stylesheet,
    parse_stylesheet_with_options as parse_syntax_stylesheet_with_options,
    serialize_declaration_list_parse_for_snapshot, serialize_declarations_for_snapshot,
    serialize_stylesheet_for_snapshot as serialize_syntax_stylesheet_for_snapshot,
    serialize_stylesheet_parse_for_snapshot as serialize_syntax_stylesheet_parse_for_snapshot,
    serialize_tokenization_for_snapshot, serialize_tokens_for_snapshot, tokenize_str,
    tokenize_str_with_options,
};

// Migration-only compatibility surfaces retained for transitional code.
#[deprecated(
    note = "CompatRule is migration-only. New engine-facing CSS work should build on css::model::Rule or use css::syntax explicitly when syntax output is required."
)]
pub use syntax::CompatRule;
#[deprecated(
    note = "CompatSelector is migration-only. New engine-facing CSS work should build on css::model or use css::syntax explicitly when syntax output is required."
)]
pub use syntax::CompatSelector;
#[deprecated(
    note = "CompatStylesheet is migration-only. Store css::StylesheetParse or css::Stylesheet instead, and keep compatibility projection isolated at the consumer boundary that still needs it."
)]
pub use syntax::CompatStylesheet;
#[deprecated(
    note = "CompatDeclaration is migration-only. New declaration/value work should build on css::Declaration or use css::syntax explicitly when declaration-list compatibility output is required."
)]
pub use syntax::Declaration as CompatDeclaration;
#[deprecated(
    note = "Compatibility stylesheet snapshots are migration-only. Prefer the model snapshot serializers for the engine-facing contract."
)]
pub use syntax::serialize_compat_stylesheet_for_snapshot;

pub use values::{
    BorderStyle, CssColorKeyword, CssColorSyntax, CssColorValue, CssFunctionValue, CssHexColor,
    CssIntegerValue, CssKeywordValue, CssLengthPercentageValue, CssLengthUnit, CssLengthValue,
    CssNumberScalar, CssNumberValue, CssPercentageValue, CssStringValue, CssUrlValue,
    CssWideKeyword, CssWideKeywordValue, Display, Length, LengthPercentage, OutlineStyle, Overflow,
    Percentage, Position, TextDecorationLine, ZIndex, parse_color, parse_length,
};
