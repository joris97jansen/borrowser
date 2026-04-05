pub mod cascade;
pub mod computed;
pub mod syntax;
pub mod values;

// Re-exports so other crates can just use `css::...` nicely.
pub use cascade::{attach_styles, get_inline_style, is_css};
pub use computed::{ComputedStyle, StyledNode, build_style_tree, compute_style};
pub use syntax::{
    CompatRule, CompatSelector, CompatStylesheet, CssAtRule, CssBlockKind, CssComponentValue,
    CssDeclaration, CssDeclarationBlock, CssDimension, CssFunction, CssHashKind, CssInput,
    CssInputId, CssNumber, CssNumericKind, CssParseOrigin, CssPosition, CssQualifiedRule, CssRule,
    CssSimpleBlock, CssSpan, CssStylesheet, CssToken, CssTokenKind, CssTokenText, CssTokenization,
    CssTokenizationStats, CssUnicodeRange, Declaration, DeclarationListParse, DiagnosticKind,
    DiagnosticSeverity, ParseOptions, ParseStats, RecoveryPolicy, StylesheetParse,
    SyntaxDiagnostic, SyntaxLimits, parse_declarations, parse_declarations_with_options,
    parse_stylesheet, parse_stylesheet_with_options, serialize_compat_stylesheet_for_snapshot,
    serialize_declaration_list_parse_for_snapshot, serialize_declarations_for_snapshot,
    serialize_stylesheet_for_snapshot, serialize_stylesheet_parse_for_snapshot,
    serialize_tokenization_for_snapshot, serialize_tokens_for_snapshot, tokenize_str,
    tokenize_str_with_options,
};
pub use values::{Display, Length, parse_color, parse_length};
