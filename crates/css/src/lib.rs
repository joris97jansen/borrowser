//! Public CSS crate surface.
//!
//! The crate root is model-first for whole-stylesheet parsing:
//! `parse_stylesheet(...)` and `parse_stylesheet_with_options(...)` produce the
//! engine-facing `css::model` parse result.
//!
//! Syntax-layer and compatibility-scoped APIs remain available explicitly for
//! parser work, migration support, and golden tests, but they are no longer the
//! preferred crate-root contract for new engine-facing CSS code.

pub mod cascade;
pub mod computed;
pub mod model;
pub mod selectors;
pub mod syntax;
pub mod values;

// Model-first crate-root surface for engine-facing stylesheet work.
pub use cascade::{
    CascadeDeclarationSource, CascadeImportance, CascadeInheritance, CascadeOrigin,
    CascadeOriginBand, CascadePriority, CascadePropertyId, CascadePropertyMetadata,
    CascadeRuleMatch, CascadeSpecificity, CascadeSpecifiedValue, CascadeWinner, InitialStyleValue,
    InlineStyleDeclarationRef, ResolvedStyle, ResolvedStyleBuildError, ResolvedStyleBuilder,
    ResolvedStyleEntry, ResolvedValueSource, StylesheetDeclarationRef, attach_styles,
    get_inline_style, is_css,
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
pub use selectors::{
    AncestorElements, AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher,
    AttributeSelector, AttributeValue, ClassSelector, Combinator, CombinedSelector,
    ComplexSelector, CompoundSelector, IdSelector, InvalidSelectorList, InvalidSelectorReason,
    MatchedSelector, NamedTypeSelector, PreviousSiblingElements, SelectorDomElementId,
    SelectorDomElementIter, SelectorDomIndex, SelectorIdent, SelectorList,
    SelectorListMatchBuilder, SelectorListMatchOutcome, SelectorListParseResult, SelectorMatchDom,
    SelectorMatchability, SelectorMatchingContext, SelectorString, SelectorStructureError,
    Specificity, SubclassSelector, TypeSelector, UniversalSelector, UnsupportedSelectorFeature,
    UnsupportedSelectorHandling, UnsupportedSelectorList, parse_selector_list,
    serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot,
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
