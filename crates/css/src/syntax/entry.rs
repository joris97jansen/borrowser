use super::compat;
use super::parser;
use super::{CompatStylesheet, Declaration, DeclarationListParse, ParseOptions, StylesheetParse};

/// Compatibility wrapper for callers that still need `CompatStylesheet`.
///
/// Within `css::syntax`, new parser work must prefer
/// `parse_stylesheet_with_options(...)` and operate on `StylesheetParse`. At
/// the crate root, whole-stylesheet parsing is now model-first through
/// `css::parse_stylesheet_with_options(...)`; this wrapper remains only for
/// explicit syntax-layer migration support.
pub fn parse_stylesheet(input: &str) -> CompatStylesheet {
    parse_stylesheet_with_options(input, &ParseOptions::stylesheet()).to_compat_stylesheet()
}

/// Contract entry point for whole-stylesheet parsing.
///
/// The primary parse result is syntax-layer structured output built on top of
/// tokenizer tokens. Compatibility projection for the current cascade path is
/// available separately.
pub fn parse_stylesheet_with_options(input: &str, options: &ParseOptions) -> StylesheetParse {
    parser::parse_stylesheet_structured(input, options)
}

/// Compatibility wrapper for callers that still need `Vec<Declaration>`.
///
/// New parser work must prefer `parse_declarations_with_options(...)` and
/// consume `DeclarationListParse`. This wrapper exists only as a migration
/// bridge for compatibility-scoped consumers.
pub fn parse_declarations(input: &str) -> Vec<Declaration> {
    parse_declarations_with_options(input, &ParseOptions::style_attribute()).declarations
}

/// Contract entry point for inline declaration lists.
///
/// As with stylesheet parsing, declaration parsing is now token-driven while
/// still returning compatibility-friendly declaration values.
pub fn parse_declarations_with_options(
    input: &str,
    options: &ParseOptions,
) -> DeclarationListParse {
    compat::parse_declarations_compat(input, 0, options)
}
