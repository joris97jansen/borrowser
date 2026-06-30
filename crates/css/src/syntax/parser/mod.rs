//! Structured CSS stylesheet parser.
//!
//! This module consumes tokenizer output and builds the syntax-layer stylesheet
//! representation used by later CSS milestones.

mod engine;
mod entry;
mod model;
mod support;

#[cfg(test)]
mod tests;

use super::{ParseOptions, StylesheetParse};

pub(crate) use self::model::StructuredDeclarationListParse;
pub use self::model::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclaration, CssDeclarationBlock, CssFunction,
    CssQualifiedRule, CssRule, CssSimpleBlock, CssStylesheet,
};
#[cfg(any(test, feature = "css-fuzzing"))]
pub(crate) use self::support::validate_token_stream_invariants;

pub(super) fn parse_stylesheet_structured(input: &str, options: &ParseOptions) -> StylesheetParse {
    entry::parse_stylesheet_structured(input, options)
}

pub(crate) fn parse_declaration_list_structured(
    input: &str,
    base_offset: usize,
    options: &ParseOptions,
) -> model::StructuredDeclarationListParse {
    entry::parse_declaration_list_structured(input, base_offset, options)
}
