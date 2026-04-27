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
pub mod model;
pub mod properties;
pub mod selectors;
pub mod specified;
pub mod syntax;
pub mod values;

// Model-first crate-root surface for engine-facing stylesheet work.
pub use cascade::{
    CascadeDeclarationApplicability, CascadeDeclarationCandidate, CascadeDeclarationCandidateKey,
    CascadeDeclarationInput, CascadeDeclarationProperty, CascadeDeclarationSource,
    CascadeImportance, CascadeInheritance, CascadeOrigin, CascadeOriginBand, CascadePriority,
    CascadePropertyId, CascadePropertyLengthSignPolicy, CascadePropertyMetadata,
    CascadePropertyRegistration, CascadePropertyRegistry, CascadeRuleContext, CascadeRuleInput,
    CascadeRuleInputBuildError, CascadeRuleMatch, CascadeRuleSource, CascadeSpecificity,
    CascadeSpecifiedValue, CascadeWinner, CascadeWinnerEntry, CascadeWinnerSet,
    CurrentScopeCascadePriorityBand, InitialStyleValue, InlineStyleDeclarationRef,
    InlineStyleRuleRef, ResolvedDocumentStyle, ResolvedElementStyle, ResolvedStyle,
    ResolvedStyleBuildError, ResolvedStyleBuilder, ResolvedStyleEntry, ResolvedValueSource,
    StyleResolutionError, StyleResolutionLimit, StyleResolutionLimits, StylesheetDeclarationRef,
    StylesheetRuleRef, attach_styles, cascade_evaluation_debug_snapshot, cascade_property_registry,
    get_inline_style, is_css, resolve_cascade_style, resolve_cascade_style_from_rule_inputs,
    resolve_cascade_winners, resolve_cascade_winners_from_rule_inputs, resolve_document_styles,
    resolve_document_styles_debug_snapshot, resolve_initial_style,
    sort_candidates_by_cascade_order, try_resolve_document_styles_with_limits,
};
pub use computed::{
    BoxMetrics, ComputedDocumentStyle, ComputedElementStyle, ComputedStyleBuildError,
    ComputedStyleBuilder, ComputedStyleEntry, ComputedStyleResolutionError, ComputedValue,
    ComputedValueDiscriminant, ComputedValueNormalizationError,
    ComputedValueNormalizationErrorKind, build_style_tree_from_computed_styles,
    build_style_tree_with_stylesheets, compute_document_styles,
    compute_document_styles_from_resolved_styles, compute_document_styles_with_limits,
    compute_style_from_resolved_style, computed_value_debug_snapshot, normalize_specified_value,
};
pub use computed::{ComputedStyle, StyledNode, build_style_tree, compute_style};
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
    PropertyLengthSignPolicy, PropertyMetadata, PropertyRegistration, PropertyRegistry,
    PropertySpecifiedValueKind, property_registry,
};
pub use selectors::{
    AncestorElements, AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher,
    AttributeSelector, AttributeValue, ClassSelector, Combinator, CombinedSelector,
    ComplexSelector, CompoundSelector, IdSelector, InvalidSelectorList, InvalidSelectorReason,
    MatchedSelector, NamedTypeSelector, PreviousSiblingElements, SelectorDomElementId,
    SelectorDomElementIter, SelectorDomIndex, SelectorIdent, SelectorList,
    SelectorListMatchBuilder, SelectorListMatchOutcome, SelectorListParseResult, SelectorMatchDom,
    SelectorMatchability, SelectorMatchingContext, SelectorMatchingLimitError,
    SelectorMatchingLimits, SelectorString, SelectorStructureError, Specificity, SubclassSelector,
    TypeSelector, UniversalSelector, UnsupportedSelectorFeature, UnsupportedSelectorHandling,
    UnsupportedSelectorList, parse_selector_list, parse_selector_list_with_limits,
    serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot,
};
pub use specified::{
    SpecifiedColor, SpecifiedColorKeyword, SpecifiedColorSyntax, SpecifiedDisplay,
    SpecifiedDisplayKeyword, SpecifiedHexColor, SpecifiedLength, SpecifiedLengthOrAuto,
    SpecifiedLengthOrNone, SpecifiedLengthUnit, SpecifiedPropertyValue, SpecifiedValue,
    SpecifiedValueLimits, SpecifiedValueParseError, SpecifiedValueParseErrorKind,
    parse_specified_value, parse_specified_value_with_limits,
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

pub use values::{Display, Length, parse_color, parse_length};
