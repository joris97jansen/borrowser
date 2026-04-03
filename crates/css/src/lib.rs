pub mod cascade;
pub mod computed;
pub mod syntax;
pub mod values;

// Re-exports so other crates can just use `css::...` nicely.
pub use cascade::{attach_styles, get_inline_style, is_css};
pub use computed::{ComputedStyle, StyledNode, build_style_tree, compute_style};
pub use syntax::{
    CompatRule, CompatSelector, CompatStylesheet, CssParseOrigin, Declaration,
    DeclarationListParse, DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats,
    RecoveryPolicy, StylesheetParse, SyntaxDiagnostic, SyntaxLimits, parse_declarations,
    parse_declarations_with_options, parse_stylesheet, parse_stylesheet_with_options,
    serialize_declarations_for_snapshot, serialize_stylesheet_for_snapshot,
};
pub use values::{Display, Length, parse_color, parse_length};
