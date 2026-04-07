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

pub use self::model::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclaration, CssDeclarationBlock, CssFunction,
    CssQualifiedRule, CssRule, CssSimpleBlock, CssStylesheet,
};

pub(super) fn parse_stylesheet_structured(input: &str, options: &ParseOptions) -> StylesheetParse {
    entry::parse_stylesheet_structured(input, options)
}

pub(super) fn parse_declaration_list_structured(
    input: &str,
    base_offset: usize,
    options: &ParseOptions,
) -> model::StructuredDeclarationListParse {
    entry::parse_declaration_list_structured(input, base_offset, options)
}
