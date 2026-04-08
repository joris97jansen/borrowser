pub mod cascade;
pub mod computed;
pub mod model;
pub mod syntax;
pub mod values;

// Re-exports so other crates can just use `css::...` nicely.
pub use cascade::{attach_styles, get_inline_style, is_css};
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
pub use syntax::{
    CompatRule, CompatSelector, CompatStylesheet, CssAtRule, CssBlockKind, CssComponentValue,
    CssDeclaration, CssDeclarationBlock, CssDimension, CssFunction, CssHashKind, CssInput,
    CssInputId, CssNumber, CssNumericKind, CssParseOrigin, CssPosition, CssQualifiedRule, CssRule,
    CssSimpleBlock, CssSpan, CssStylesheet, CssToken, CssTokenKind, CssTokenText, CssTokenization,
    CssTokenizationStats, CssUnicodeRange, Declaration as CompatDeclaration, DeclarationListParse,
    DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats, RecoveryPolicy,
    StylesheetParse as SyntaxStylesheetParse, SyntaxDiagnostic, SyntaxLimits, parse_declarations,
    parse_declarations_with_options, parse_stylesheet as parse_syntax_stylesheet,
    parse_stylesheet_with_options as parse_syntax_stylesheet_with_options,
    serialize_compat_stylesheet_for_snapshot, serialize_declaration_list_parse_for_snapshot,
    serialize_declarations_for_snapshot,
    serialize_stylesheet_for_snapshot as serialize_syntax_stylesheet_for_snapshot,
    serialize_stylesheet_parse_for_snapshot as serialize_syntax_stylesheet_parse_for_snapshot,
    serialize_tokenization_for_snapshot, serialize_tokens_for_snapshot, tokenize_str,
    tokenize_str_with_options,
};
pub use values::{Display, Length, parse_color, parse_length};
