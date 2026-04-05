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

mod compat;
mod input;
mod parser;
mod token;
mod tokenizer;

use std::fmt::Write;

pub use compat::{CompatRule, CompatSelector, CompatStylesheet};
pub use input::{CssInput, CssInputId, CssPosition, CssSpan};
pub use parser::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclaration, CssDeclarationBlock, CssFunction,
    CssQualifiedRule, CssRule, CssSimpleBlock, CssStylesheet,
};
pub use token::{
    CssDimension, CssHashKind, CssNumber, CssNumericKind, CssToken, CssTokenKind, CssTokenText,
    CssUnicodeRange, serialize_tokens_for_snapshot,
};
pub use tokenizer::{
    CssTokenization, CssTokenizationStats, tokenize_str, tokenize_str_with_options,
};

/// Parsing origin for diagnostics and entry-point-specific limit handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssParseOrigin {
    Stylesheet,
    StyleAttribute,
}

/// Recovery policy for malformed CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryPolicy {
    /// Malformed input is skipped using fixed structural boundaries and without
    /// implementation-defined heuristics.
    Deterministic,
}

/// Resource limits for bounded parser behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxLimits {
    pub max_stylesheet_input_bytes: usize,
    pub max_declaration_list_input_bytes: usize,
    pub max_rules: usize,
    pub max_selectors_per_rule: usize,
    pub max_declarations_per_rule: usize,
    pub max_diagnostics: usize,
}

impl Default for SyntaxLimits {
    fn default() -> Self {
        Self {
            max_stylesheet_input_bytes: 4 * 1024 * 1024,
            max_declaration_list_input_bytes: 64 * 1024,
            max_rules: 16_384,
            max_selectors_per_rule: 256,
            max_declarations_per_rule: 1_024,
            max_diagnostics: 128,
        }
    }
}

/// Options shared by stylesheet and declaration-list entry points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseOptions {
    pub origin: CssParseOrigin,
    pub recovery_policy: RecoveryPolicy,
    pub limits: SyntaxLimits,
    pub collect_diagnostics: bool,
}

impl ParseOptions {
    pub fn stylesheet() -> Self {
        Self {
            origin: CssParseOrigin::Stylesheet,
            recovery_policy: RecoveryPolicy::Deterministic,
            limits: SyntaxLimits::default(),
            collect_diagnostics: true,
        }
    }

    pub fn style_attribute() -> Self {
        Self {
            origin: CssParseOrigin::StyleAttribute,
            ..Self::stylesheet()
        }
    }
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self::stylesheet()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

impl DiagnosticSeverity {
    pub(crate) fn snapshot_label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    UnexpectedEof,
    UnexpectedToken,
    EmptySelectorList,
    InvalidSelector,
    InvalidDeclaration,
    UnterminatedComment,
    UnterminatedString,
    BadUrl,
    LimitExceeded,
}

impl DiagnosticKind {
    pub(crate) fn stable_code(self) -> &'static str {
        match self {
            Self::UnexpectedEof => "unexpected-eof",
            Self::UnexpectedToken => "unexpected-token",
            Self::EmptySelectorList => "empty-selector-list",
            Self::InvalidSelector => "invalid-selector",
            Self::InvalidDeclaration => "invalid-declaration",
            Self::UnterminatedComment => "unterminated-comment",
            Self::UnterminatedString => "unterminated-string",
            Self::BadUrl => "bad-url",
            Self::LimitExceeded => "limit-exceeded",
        }
    }
}

/// Structured parse diagnostic.
///
/// Diagnostics expose a stable byte offset suitable for tokenizer and parser
/// recovery reporting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub severity: DiagnosticSeverity,
    pub kind: DiagnosticKind,
    pub byte_offset: usize,
    pub message: String,
}

/// Parse summary for tests and downstream callers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseStats {
    pub input_bytes: usize,
    pub rules_emitted: usize,
    pub declarations_emitted: usize,
    pub diagnostics_emitted: usize,
    pub hit_limit: bool,
}

/// Compatibility-scoped declaration used by the current cascade path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Default)]
pub struct StylesheetParse {
    pub input: CssInput,
    pub stylesheet: CssStylesheet,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: ParseStats,
}

impl StylesheetParse {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = serialize_stylesheet_for_snapshot(&self.input, &self.stylesheet);
        serialize_diagnostics_for_snapshot(&mut out, &self.diagnostics);
        serialize_stats_for_snapshot(&mut out, &self.stats);
        out
    }

    pub fn to_compat_stylesheet(&self) -> CompatStylesheet {
        compat::project_stylesheet_to_compat(&self.input, &self.stylesheet)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeclarationListParse {
    pub declarations: Vec<Declaration>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: ParseStats,
}

impl DeclarationListParse {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = serialize_declarations_for_snapshot(&self.declarations);
        serialize_diagnostics_for_snapshot(&mut out, &self.diagnostics);
        serialize_stats_for_snapshot(&mut out, &self.stats);
        out
    }
}

/// Compatibility entry point used by the current engine.
///
/// The return type is intentionally named `CompatStylesheet` to make clear that
/// the existing selector/rule representation is an adapter for today's cascade
/// path, not the long-term CSS syntax tree.
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

/// Compatibility entry point used by the current cascade layer for `style=""`
/// attributes.
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

pub fn serialize_stylesheet_for_snapshot(input: &CssInput, sheet: &CssStylesheet) -> String {
    let mut out = String::new();
    writeln!(&mut out, "stylesheet").expect("write stylesheet header");
    for (rule_index, rule) in sheet.rules.iter().enumerate() {
        match rule {
            CssRule::Qualified(rule) => {
                writeln!(
                    &mut out,
                    "rule[{rule_index}] qualified @{}..{}",
                    rule.span.start, rule.span.end
                )
                .expect("write qualified rule header");
                writeln!(&mut out, "  prelude").expect("write prelude header");
                for value in &rule.prelude {
                    writeln!(&mut out, "    - {}", component_value_snapshot(input, value))
                        .expect("write prelude snapshot");
                }
                writeln!(
                    &mut out,
                    "  block @{}..{}",
                    rule.block.span.start, rule.block.span.end
                )
                .expect("write block header");
                for (declaration_index, declaration) in rule.block.declarations.iter().enumerate() {
                    writeln!(
                        &mut out,
                        "    declaration[{declaration_index}] {} @{}..{}",
                        quoted_text(input, &declaration.name),
                        declaration.span.start,
                        declaration.span.end
                    )
                    .expect("write declaration header");
                    for value in &declaration.value {
                        writeln!(
                            &mut out,
                            "      - {}",
                            component_value_snapshot(input, value)
                        )
                        .expect("write declaration value snapshot");
                    }
                }
            }
            CssRule::At(rule) => {
                writeln!(
                    &mut out,
                    "rule[{rule_index}] at({}) @{}..{}",
                    quoted_text(input, &rule.name),
                    rule.span.start,
                    rule.span.end
                )
                .expect("write at-rule header");
                writeln!(&mut out, "  prelude").expect("write at-rule prelude header");
                for value in &rule.prelude {
                    writeln!(&mut out, "    - {}", component_value_snapshot(input, value))
                        .expect("write at-rule prelude snapshot");
                }
                if let Some(block) = &rule.block {
                    writeln!(
                        &mut out,
                        "  block(kind={}) @{}..{}",
                        block_kind_label(block.kind),
                        block.span.start,
                        block.span.end
                    )
                    .expect("write at-rule block header");
                    for value in &block.value {
                        writeln!(&mut out, "    - {}", component_value_snapshot(input, value))
                            .expect("write at-rule block snapshot");
                    }
                }
            }
        }
    }
    out
}

pub fn serialize_compat_stylesheet_for_snapshot(sheet: &CompatStylesheet) -> String {
    let mut out = String::new();
    writeln!(&mut out, "stylesheet").expect("write stylesheet header");
    for (rule_index, rule) in sheet.rules.iter().enumerate() {
        writeln!(&mut out, "rule[{rule_index}]").expect("write rule header");
        writeln!(&mut out, "  selectors").expect("write selectors header");
        for selector in &rule.selectors {
            writeln!(&mut out, "    - {}", compat::selector_snapshot(selector))
                .expect("write selector snapshot");
        }
        writeln!(&mut out, "  declarations").expect("write declarations header");
        for declaration in &rule.declarations {
            writeln!(
                &mut out,
                "    - {}: {}",
                declaration.name, declaration.value
            )
            .expect("write declaration snapshot");
        }
    }
    out
}

pub fn serialize_declarations_for_snapshot(declarations: &[Declaration]) -> String {
    let mut out = String::new();
    writeln!(&mut out, "declarations").expect("write declarations header");
    for declaration in declarations {
        writeln!(&mut out, "  - {}: {}", declaration.name, declaration.value)
            .expect("write declaration snapshot");
    }
    out
}

pub(crate) fn append_diagnostics(
    options: &ParseOptions,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    incoming: Vec<SyntaxDiagnostic>,
) {
    if !options.collect_diagnostics || diagnostics.len() >= options.limits.max_diagnostics {
        return;
    }
    let remaining = options.limits.max_diagnostics - diagnostics.len();
    diagnostics.extend(incoming.into_iter().take(remaining));
}

pub(crate) fn push_diagnostic(
    options: &ParseOptions,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    stats: &mut ParseStats,
    severity: DiagnosticSeverity,
    kind: DiagnosticKind,
    byte_offset: usize,
    message: impl Into<String>,
) {
    stats.diagnostics_emitted += 1;
    if !options.collect_diagnostics || diagnostics.len() >= options.limits.max_diagnostics {
        return;
    }
    diagnostics.push(SyntaxDiagnostic {
        severity,
        kind,
        byte_offset,
        message: message.into(),
    });
}

pub(crate) fn truncate_to_limit(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }

    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

fn serialize_diagnostics_for_snapshot(out: &mut String, diagnostics: &[SyntaxDiagnostic]) {
    writeln!(out, "diagnostics").expect("write diagnostics header");
    for diagnostic in diagnostics {
        writeln!(
            out,
            "  - {} {} @{}",
            diagnostic.severity.snapshot_label(),
            diagnostic.kind.stable_code(),
            diagnostic.byte_offset,
        )
        .expect("write diagnostic snapshot");
    }
}

fn component_value_snapshot(input: &CssInput, value: &CssComponentValue) -> String {
    match value {
        CssComponentValue::PreservedToken(token) => format!(
            "token({}) @{}..{}",
            token_kind_snapshot(input, &token.kind),
            token.span.start,
            token.span.end
        ),
        CssComponentValue::SimpleBlock(block) => format!(
            "simple-block(kind={}, text={:?}) @{}..{}",
            block_kind_label(block.kind),
            input.slice(block.span).unwrap_or(""),
            block.span.start,
            block.span.end
        ),
        CssComponentValue::Function(function) => format!(
            "function(name={}, text={:?}) @{}..{}",
            quoted_text(input, &function.name),
            input.slice(function.span).unwrap_or(""),
            function.span.start,
            function.span.end
        ),
    }
}

fn block_kind_label(kind: CssBlockKind) -> &'static str {
    match kind {
        CssBlockKind::Curly => "curly",
        CssBlockKind::Square => "square",
        CssBlockKind::Parenthesis => "parenthesis",
    }
}

fn quoted_text(input: &CssInput, text: &CssTokenText) -> String {
    match text.resolve(input) {
        Some(text) => format!("{text:?}"),
        None => "<invalid-span>".to_string(),
    }
}

fn token_kind_snapshot(input: &CssInput, kind: &CssTokenKind) -> String {
    match kind {
        CssTokenKind::Whitespace => "whitespace".to_string(),
        CssTokenKind::Comment(text) => format!("comment({})", quoted_text(input, text)),
        CssTokenKind::Ident(text) => format!("ident({})", quoted_text(input, text)),
        CssTokenKind::Function(text) => format!("function({})", quoted_text(input, text)),
        CssTokenKind::AtKeyword(text) => format!("at-keyword({})", quoted_text(input, text)),
        CssTokenKind::Hash { value, kind } => format!(
            "hash(kind={}, value={})",
            match kind {
                CssHashKind::Id => "id",
                CssHashKind::Unrestricted => "unrestricted",
            },
            quoted_text(input, value)
        ),
        CssTokenKind::String(text) => format!("string({})", quoted_text(input, text)),
        CssTokenKind::BadString => "bad-string".to_string(),
        CssTokenKind::Url(text) => format!("url({})", quoted_text(input, text)),
        CssTokenKind::BadUrl => "bad-url".to_string(),
        CssTokenKind::Delim(ch) => format!("delim({ch:?})"),
        CssTokenKind::Number(number) => format!("number({})", quoted_text(input, &number.repr)),
        CssTokenKind::Percentage(number) => {
            format!("percentage({})", quoted_text(input, &number.repr))
        }
        CssTokenKind::Dimension(dimension) => format!(
            "dimension(value={}, unit={})",
            quoted_text(input, &dimension.number.repr),
            quoted_text(input, &dimension.unit)
        ),
        CssTokenKind::UnicodeRange(range) => {
            format!("unicode-range(U+{:X}-U+{:X})", range.start(), range.end())
        }
        CssTokenKind::Colon => "colon".to_string(),
        CssTokenKind::Semicolon => "semicolon".to_string(),
        CssTokenKind::Comma => "comma".to_string(),
        CssTokenKind::LeftSquareBracket => "left-square-bracket".to_string(),
        CssTokenKind::RightSquareBracket => "right-square-bracket".to_string(),
        CssTokenKind::LeftParenthesis => "left-parenthesis".to_string(),
        CssTokenKind::RightParenthesis => "right-parenthesis".to_string(),
        CssTokenKind::LeftCurlyBracket => "left-curly-bracket".to_string(),
        CssTokenKind::RightCurlyBracket => "right-curly-bracket".to_string(),
        CssTokenKind::IncludeMatch => "include-match".to_string(),
        CssTokenKind::DashMatch => "dash-match".to_string(),
        CssTokenKind::PrefixMatch => "prefix-match".to_string(),
        CssTokenKind::SuffixMatch => "suffix-match".to_string(),
        CssTokenKind::SubstringMatch => "substring-match".to_string(),
        CssTokenKind::Column => "column".to_string(),
        CssTokenKind::Cdo => "cdo".to_string(),
        CssTokenKind::Cdc => "cdc".to_string(),
        CssTokenKind::Eof => "eof".to_string(),
    }
}

fn serialize_stats_for_snapshot(out: &mut String, stats: &ParseStats) {
    writeln!(out, "stats").expect("write stats header");
    writeln!(out, "  input_bytes: {}", stats.input_bytes).expect("write input_bytes");
    writeln!(out, "  rules_emitted: {}", stats.rules_emitted).expect("write rules_emitted");
    writeln!(
        out,
        "  declarations_emitted: {}",
        stats.declarations_emitted
    )
    .expect("write declarations_emitted");
    writeln!(out, "  diagnostics_emitted: {}", stats.diagnostics_emitted)
        .expect("write diagnostics_emitted");
    writeln!(out, "  hit_limit: {}", stats.hit_limit).expect("write hit_limit");
}

#[cfg(test)]
mod tests {
    use super::{
        CompatSelector, CssRule, DiagnosticKind, ParseOptions, SyntaxLimits,
        parse_declarations_with_options, parse_stylesheet_with_options,
    };

    #[test]
    fn stylesheet_parse_snapshot_is_stable() {
        let parse = parse_stylesheet_with_options(
            "div, #hero { color: red; font-size: 12px; }",
            &ParseOptions::stylesheet(),
        );

        assert_eq!(
            parse.to_debug_snapshot(),
            concat!(
                "stylesheet\n",
                "rule[0] qualified @0..43\n",
                "  prelude\n",
                "    - token(ident(\"div\")) @0..3\n",
                "    - token(comma) @3..4\n",
                "    - token(whitespace) @4..5\n",
                "    - token(hash(kind=id, value=\"hero\")) @5..10\n",
                "    - token(whitespace) @10..11\n",
                "  block @11..43\n",
                "    declaration[0] \"color\" @13..24\n",
                "      - token(whitespace) @19..20\n",
                "      - token(ident(\"red\")) @20..23\n",
                "    declaration[1] \"font-size\" @25..41\n",
                "      - token(whitespace) @35..36\n",
                "      - token(dimension(value=\"12\", unit=\"px\")) @36..40\n",
                "diagnostics\n",
                "stats\n",
                "  input_bytes: 43\n",
                "  rules_emitted: 1\n",
                "  declarations_emitted: 2\n",
                "  diagnostics_emitted: 0\n",
                "  hit_limit: false\n",
            )
        );
    }

    #[test]
    fn snapshot_contract_uses_stable_diagnostic_fields_only() {
        let parse = parse_declarations_with_options("color red;", &ParseOptions::style_attribute());
        let snapshot = parse.to_debug_snapshot();

        assert!(snapshot.contains("warning invalid-declaration @0"));
        assert!(!snapshot.contains("ignored declaration without `:` delimiter"));
    }

    #[test]
    fn declaration_list_reports_invalid_entries_deterministically() {
        let parse = parse_declarations_with_options(
            "color: red; broken; : nope; width: 10px;",
            &ParseOptions::style_attribute(),
        );

        assert_eq!(parse.declarations.len(), 2);
        assert_eq!(parse.diagnostics.len(), 2);
        assert_eq!(
            parse.diagnostics[0].kind,
            DiagnosticKind::InvalidDeclaration
        );
        assert_eq!(
            parse.diagnostics[1].kind,
            DiagnosticKind::InvalidDeclaration
        );
    }

    #[test]
    fn declaration_lists_do_not_split_on_semicolons_inside_strings() {
        let parse = parse_declarations_with_options(
            "content: \";\"; color: red;",
            &ParseOptions::style_attribute(),
        );

        assert_eq!(parse.declarations.len(), 2);
        assert_eq!(parse.declarations[0].name, "content");
        assert_eq!(parse.declarations[0].value, "\";\"");
        assert_eq!(parse.declarations[1].name, "color");
        assert_eq!(parse.declarations[1].value, "red");
    }

    #[test]
    fn stylesheet_parsing_does_not_split_on_braces_inside_strings() {
        let parse = parse_stylesheet_with_options(
            "div { content: \"}\"; color: red; }",
            &ParseOptions::stylesheet(),
        );
        let compat = parse.to_compat_stylesheet();

        assert_eq!(parse.stylesheet.rules.len(), 1);
        assert_eq!(compat.rules.len(), 1);
        assert_eq!(compat.rules[0].declarations.len(), 2);
        assert_eq!(compat.rules[0].declarations[0].name, "content");
        assert_eq!(compat.rules[0].declarations[0].value, "\"}\"");
        assert_eq!(compat.rules[0].declarations[1].name, "color");
        assert_eq!(compat.rules[0].declarations[1].value, "red");
    }

    #[test]
    fn compat_empty_id_and_class_selectors_are_rejected() {
        let parse = parse_stylesheet_with_options(
            "# { color: red; } . { color: blue; } div { color: green; }",
            &ParseOptions::stylesheet(),
        );
        let compat = parse.to_compat_stylesheet();

        assert_eq!(parse.stylesheet.rules.len(), 3);
        assert_eq!(compat.rules.len(), 1);
        assert_eq!(
            compat.rules[0].selectors,
            vec![CompatSelector::Type("div".to_string())]
        );
    }

    #[test]
    fn structured_stylesheet_represents_at_rules_and_qualified_rules() {
        let parse = parse_stylesheet_with_options(
            "@media screen { color: red; } div { color: blue; }",
            &ParseOptions::stylesheet(),
        );

        assert_eq!(parse.stylesheet.rules.len(), 2);
        assert!(matches!(parse.stylesheet.rules[0], CssRule::At(_)));
        assert!(matches!(parse.stylesheet.rules[1], CssRule::Qualified(_)));
    }

    #[test]
    fn stylesheet_limits_are_enforced() {
        let options = ParseOptions {
            limits: SyntaxLimits {
                max_rules: 1,
                ..SyntaxLimits::default()
            },
            ..ParseOptions::stylesheet()
        };
        let parse =
            parse_stylesheet_with_options("div { color: red; } span { color: blue; }", &options);

        assert_eq!(parse.stylesheet.rules.len(), 1);
        assert!(parse.stats.hit_limit);
        assert!(
            parse
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::LimitExceeded)
        );
    }
}
