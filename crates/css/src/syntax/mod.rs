//! Stable CSS syntax contract surface.
//!
//! This module owns parser-facing options, diagnostics, decoded-input
//! primitives, source-bound spans, explicit token definitions, the structured
//! stylesheet parser, and the CSS tokenizer/parser entry points for the syntax
//! layer.
//!
//! The current compatibility-scoped stylesheet shape remains available only
//! through the private `compat` adapter module below. That adapter now projects
//! structured syntax-layer output into the existing cascade-facing
//! representation during rollout, but it is not normative for the long-term
//! tokenizer/parser architecture.
//!
//! The downstream engine-facing CSS rule/value model is defined separately by
//! the Milestone O contract in `docs/css/o1-rule-value-model-architecture.md`.
//! This module remains purely syntax-layer: it must not absorb selector
//! matching, cascade semantics, or long-lived stylesheet ownership concerns.
//!
//! `Compat*` re-exports remain public only as migration bridges for the
//! current cascade path. New engine-facing CSS work must not treat them as the
//! permanent rule/value contract.

mod compat;
mod diagnostics;
mod entry;
mod input;
mod options;
mod parser;
mod results;
mod serialize;
mod token;
mod tokenizer;
mod util;

#[cfg(test)]
mod tests;

/// Migration-only compatibility re-exports for the current cascade path.
///
/// These are not the permanent engine-facing stylesheet/rule/selector model.
pub use compat::{CompatRule, CompatSelector, CompatStylesheet};
pub use diagnostics::{DiagnosticKind, DiagnosticSeverity, SyntaxDiagnostic};
pub use entry::{
    parse_declarations, parse_declarations_with_options, parse_stylesheet,
    parse_stylesheet_with_options,
};
pub use input::{CssInput, CssInputId, CssPosition, CssSpan};
pub use options::{CssParseOrigin, ParseOptions, RecoveryPolicy, SyntaxLimits};
pub use parser::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclaration, CssDeclarationBlock, CssFunction,
    CssQualifiedRule, CssRule, CssSimpleBlock, CssStylesheet,
};
pub use results::{Declaration, DeclarationListParse, ParseStats, StylesheetParse};
pub use serialize::{
    serialize_compat_stylesheet_for_snapshot, serialize_declaration_list_parse_for_snapshot,
    serialize_declarations_for_snapshot, serialize_stylesheet_for_snapshot,
    serialize_stylesheet_parse_for_snapshot, serialize_tokenization_for_snapshot,
    serialize_tokens_for_snapshot,
};
pub use token::{
    CssDimension, CssHashKind, CssNumber, CssNumericKind, CssToken, CssTokenKind, CssTokenText,
    CssUnicodeRange,
};
pub use tokenizer::{
    CssTokenization, CssTokenizationStats, tokenize_str, tokenize_str_with_options,
};

pub(crate) use util::{append_diagnostics, push_diagnostic, truncate_to_limit};
