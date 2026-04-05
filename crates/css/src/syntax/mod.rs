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
mod serialize;
mod token;
mod tokenizer;

pub use compat::{CompatRule, CompatSelector, CompatStylesheet};
pub use input::{CssInput, CssInputId, CssPosition, CssSpan};
pub use parser::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclaration, CssDeclarationBlock, CssFunction,
    CssQualifiedRule, CssRule, CssSimpleBlock, CssStylesheet,
};
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
    pub max_lexical_tokens: usize,
    pub max_rules: usize,
    pub max_selectors_per_rule: usize,
    pub max_declarations_per_rule: usize,
    pub max_component_nesting_depth: usize,
    pub max_diagnostics: usize,
}

impl Default for SyntaxLimits {
    fn default() -> Self {
        Self {
            max_stylesheet_input_bytes: 4 * 1024 * 1024,
            max_declaration_list_input_bytes: 64 * 1024,
            max_lexical_tokens: 262_144,
            max_rules: 16_384,
            max_selectors_per_rule: 256,
            max_declarations_per_rule: 1_024,
            max_component_nesting_depth: 256,
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
    InvariantViolation,
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
            Self::InvariantViolation => "invariant-violation",
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
        serialize_stylesheet_parse_for_snapshot(self)
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
        serialize_declaration_list_parse_for_snapshot(self)
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

#[cfg(test)]
mod tests {
    use super::{
        CompatSelector, CssRule, DiagnosticKind, ParseOptions, SyntaxLimits,
        parse_declarations_with_options, parse_stylesheet_with_options, tokenize_str_with_options,
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
                "version: 1\n",
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
                "      - token(dimension(kind=integer, value=\"12\", unit=\"px\")) @36..40\n",
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

    #[test]
    fn tokenizer_token_limit_is_enforced() {
        let options = ParseOptions {
            limits: SyntaxLimits {
                max_lexical_tokens: 4,
                ..SyntaxLimits::default()
            },
            ..ParseOptions::stylesheet()
        };
        let tokenization = tokenize_str_with_options("a,b,c,d,e", &options);

        assert!(tokenization.stats.hit_limit);
        assert!(tokenization.tokens.len() <= 5);
        assert!(matches!(
            tokenization.tokens.last().map(|token| &token.kind),
            Some(super::CssTokenKind::Eof)
        ));
        assert!(
            tokenization
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::LimitExceeded)
        );
    }

    #[test]
    fn parser_component_nesting_limit_is_enforced() {
        let options = ParseOptions {
            limits: SyntaxLimits {
                max_component_nesting_depth: 1,
                ..SyntaxLimits::default()
            },
            ..ParseOptions::stylesheet()
        };
        let parse = parse_stylesheet_with_options(
            "div { color: calc(calc(calc(1px))); width: 10px; }",
            &options,
        );
        let compat = parse.to_compat_stylesheet();

        assert!(parse.stats.hit_limit);
        assert_eq!(parse.stylesheet.rules.len(), 1);
        assert_eq!(compat.rules.len(), 1);
        assert_eq!(compat.rules[0].declarations.len(), 2);
        assert!(
            parse
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::LimitExceeded)
        );
    }

    #[test]
    fn malformed_qualified_rule_recovers_at_semicolon_and_preserves_later_rule() {
        let parse = parse_stylesheet_with_options(
            "div; span { color: blue; }",
            &ParseOptions::stylesheet(),
        );
        let compat = parse.to_compat_stylesheet();

        assert_eq!(parse.stylesheet.rules.len(), 1);
        assert_eq!(compat.rules.len(), 1);
        assert_eq!(
            compat.rules[0].selectors,
            vec![CompatSelector::Type("span".to_string())]
        );
        assert!(
            parse
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::UnexpectedToken)
        );
    }

    #[test]
    fn malformed_at_rule_recovers_at_right_brace_and_preserves_later_rule() {
        let parse = parse_stylesheet_with_options(
            "@media screen } span { color: blue; }",
            &ParseOptions::stylesheet(),
        );
        let compat = parse.to_compat_stylesheet();

        assert_eq!(parse.stylesheet.rules.len(), 2);
        assert!(matches!(parse.stylesheet.rules[0], CssRule::At(_)));
        assert_eq!(compat.rules.len(), 1);
        assert_eq!(
            compat.rules[0].selectors,
            vec![CompatSelector::Type("span".to_string())]
        );
        assert!(
            parse
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::UnexpectedToken)
        );
    }

    #[test]
    fn declaration_recovery_resyncs_at_next_declaration_start_without_semicolon() {
        let parse = parse_stylesheet_with_options(
            "div { color red width: 10px; height: 20px; }",
            &ParseOptions::stylesheet(),
        );
        let compat = parse.to_compat_stylesheet();

        assert_eq!(compat.rules.len(), 1);
        assert_eq!(compat.rules[0].declarations.len(), 2);
        assert_eq!(compat.rules[0].declarations[0].name, "width");
        assert_eq!(compat.rules[0].declarations[0].value, "10px");
        assert_eq!(compat.rules[0].declarations[1].name, "height");
        assert_eq!(compat.rules[0].declarations[1].value, "20px");
        assert!(
            parse
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::InvalidDeclaration)
        );
    }

    #[test]
    fn declaration_recovery_preserves_progress_after_invalid_at_rule_like_input() {
        let parse = parse_stylesheet_with_options(
            "div { @media x { } width: 1px; height: 2px; }",
            &ParseOptions::stylesheet(),
        );
        let compat = parse.to_compat_stylesheet();

        assert_eq!(compat.rules.len(), 1);
        assert_eq!(compat.rules[0].declarations.len(), 2);
        assert_eq!(compat.rules[0].declarations[0].name, "width");
        assert_eq!(compat.rules[0].declarations[0].value, "1px");
        assert_eq!(compat.rules[0].declarations[1].name, "height");
        assert_eq!(compat.rules[0].declarations[1].value, "2px");
        assert!(
            parse
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::InvalidDeclaration)
        );
    }
}
