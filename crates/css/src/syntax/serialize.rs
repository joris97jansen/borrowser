use super::compat::{self, CompatStylesheet};
use super::input::CssInput;
use super::parser::{CssBlockKind, CssComponentValue, CssRule, CssStylesheet};
use super::token::{CssHashKind, CssNumericKind, CssToken, CssTokenKind, CssTokenText};
use super::{
    CssTokenization, CssTokenizationStats, Declaration, DeclarationListParse, ParseStats,
    StylesheetParse, SyntaxDiagnostic,
};
use std::fmt::Write;

// Bump this only when the stable snapshot format changes in a way that
// invalidates existing golden files. Changes to parser/tokenizer behavior that
// merely produce different content under the same serialization grammar should
// update fixtures without changing the version.
const SNAPSHOT_VERSION: u32 = 1;

pub fn serialize_tokens_for_snapshot(input: &CssInput, tokens: &[CssToken]) -> String {
    let mut out = String::new();
    write_snapshot_header(&mut out, "tokens");
    for (index, token) in tokens.iter().enumerate() {
        writeln!(
            &mut out,
            "token[{index}] {} @{}..{}",
            token_kind_snapshot(input, &token.kind),
            token.span.start,
            token.span.end,
        )
        .expect("write token snapshot");
    }
    out
}

pub fn serialize_stylesheet_for_snapshot(input: &CssInput, sheet: &CssStylesheet) -> String {
    let mut out = String::new();
    write_snapshot_header(&mut out, "stylesheet");
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
    write_snapshot_header(&mut out, "stylesheet");
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
    write_snapshot_header(&mut out, "declarations");
    for declaration in declarations {
        writeln!(&mut out, "  - {}: {}", declaration.name, declaration.value)
            .expect("write declaration snapshot");
    }
    out
}

pub fn serialize_tokenization_for_snapshot(tokenization: &CssTokenization) -> String {
    let mut out = serialize_tokens_for_snapshot(&tokenization.input, &tokenization.tokens);
    serialize_diagnostics_for_snapshot(&mut out, &tokenization.diagnostics);
    serialize_tokenization_stats_for_snapshot(&mut out, &tokenization.stats);
    out
}

pub fn serialize_stylesheet_parse_for_snapshot(parse: &StylesheetParse) -> String {
    let mut out = serialize_stylesheet_for_snapshot(&parse.input, &parse.stylesheet);
    serialize_diagnostics_for_snapshot(&mut out, &parse.diagnostics);
    serialize_stats_for_snapshot(&mut out, &parse.stats);
    out
}

pub fn serialize_declaration_list_parse_for_snapshot(parse: &DeclarationListParse) -> String {
    let mut out = serialize_declarations_for_snapshot(&parse.declarations);
    serialize_diagnostics_for_snapshot(&mut out, &parse.diagnostics);
    serialize_stats_for_snapshot(&mut out, &parse.stats);
    out
}

pub(crate) fn serialize_diagnostics_for_snapshot(
    out: &mut String,
    diagnostics: &[SyntaxDiagnostic],
) {
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

pub(crate) fn serialize_tokenization_stats_for_snapshot(
    out: &mut String,
    stats: &CssTokenizationStats,
) {
    writeln!(out, "stats").expect("write stats header");
    writeln!(out, "  input_bytes: {}", stats.input_bytes).expect("write input_bytes");
    writeln!(out, "  tokens_emitted: {}", stats.tokens_emitted).expect("write tokens_emitted");
    writeln!(out, "  diagnostics_emitted: {}", stats.diagnostics_emitted)
        .expect("write diagnostics_emitted");
    writeln!(out, "  hit_limit: {}", stats.hit_limit).expect("write hit_limit");
}

pub(crate) fn serialize_stats_for_snapshot(out: &mut String, stats: &ParseStats) {
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

fn write_snapshot_header(out: &mut String, kind: &str) {
    writeln!(out, "version: {SNAPSHOT_VERSION}").expect("write snapshot version");
    writeln!(out, "{kind}").expect("write snapshot kind");
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
            "simple-block(kind={}, text={}) @{}..{}",
            block_kind_label(block.kind),
            quoted_raw(input.slice(block.span).unwrap_or("")),
            block.span.start,
            block.span.end
        ),
        CssComponentValue::Function(function) => format!(
            "function(name={}, text={}) @{}..{}",
            quoted_text(input, &function.name),
            quoted_raw(input.slice(function.span).unwrap_or("")),
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
        CssTokenKind::Delim(ch) => format!("delim({})", quoted_char(*ch)),
        CssTokenKind::Number(number) => format!(
            "number(kind={}, value={})",
            numeric_kind_label(number.kind),
            quoted_text(input, &number.repr)
        ),
        CssTokenKind::Percentage(number) => format!(
            "percentage(kind={}, value={})",
            numeric_kind_label(number.kind),
            quoted_text(input, &number.repr)
        ),
        CssTokenKind::Dimension(dimension) => format!(
            "dimension(kind={}, value={}, unit={})",
            numeric_kind_label(dimension.number.kind),
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

fn numeric_kind_label(kind: CssNumericKind) -> &'static str {
    match kind {
        CssNumericKind::Integer => "integer",
        CssNumericKind::Number => "number",
    }
}

fn quoted_text(input: &CssInput, text: &CssTokenText) -> String {
    match text.resolve(input) {
        Some(text) => quoted_raw(&text),
        None => "<invalid-span>".to_string(),
    }
}

fn quoted_raw(text: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{000C}' => out.push_str("\\f"),
            ch if ch.is_control() => {
                write!(&mut out, "\\u{{{:X}}}", ch as u32).expect("write control escape")
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn quoted_char(ch: char) -> String {
    let mut out = String::new();
    out.push('\'');
    match ch {
        '\\' => out.push_str("\\\\"),
        '\'' => out.push_str("\\'"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{000C}' => out.push_str("\\f"),
        ch if ch.is_control() => {
            write!(&mut out, "\\u{{{:X}}}", ch as u32).expect("write control char escape")
        }
        ch => out.push(ch),
    }
    out.push('\'');
    out
}
