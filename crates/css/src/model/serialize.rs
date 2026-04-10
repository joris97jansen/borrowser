//! Stable snapshot serializer for the engine-facing CSS model.
//!
//! The snapshot surface is intentionally aligned with the model layer rather
//! than authored CSS text. It is deterministic, versioned, and suitable for
//! regression fixtures.

use super::{
    AtRuleBlock, Declaration, DeclarationValue, PreservedBlock, PreservedComponentList,
    PropertyNameKind, Rule, Stylesheet, StylesheetParse, ValueBlock, ValueComponent, ValueFunction,
    ValueSymbol, ValueText, ValueToken,
};
use crate::selectors::write_selector_parse_result_snapshot_body;
use crate::syntax::{
    CssBlockKind, CssComponentValue, CssInput, CssNumericKind, CssParseOrigin, CssTokenKind,
    CssTokenText, ParseStats, SyntaxDiagnostic,
};
use std::fmt::Write;

const SNAPSHOT_VERSION: u32 = 1;
const SNAPSHOT_KIND_STYLESHEET: &str = "model-stylesheet";

pub fn serialize_stylesheet_for_snapshot(input: &CssInput, sheet: &Stylesheet) -> String {
    let mut out = String::new();
    writeln!(out, "version: {SNAPSHOT_VERSION}").expect("write snapshot version");
    writeln!(out, "{SNAPSHOT_KIND_STYLESHEET}").expect("write snapshot kind");
    writeln!(out, "origin: {}", origin_label(sheet.origin)).expect("write origin");
    writeln!(out, "span: {}", span_label(sheet.debug_span())).expect("write stylesheet span");

    for (rule_index, rule) in sheet.rules.iter().enumerate() {
        write_rule(&mut out, input, rule, Some(rule_index), 0);
    }

    out
}

/// Serialize one engine-facing rule using the stable model snapshot grammar.
pub fn serialize_rule_for_snapshot(input: &CssInput, rule: &Rule) -> String {
    let mut out = String::new();
    write_rule(&mut out, input, rule, None, 0);
    out
}

/// Serialize one engine-facing declaration using the stable model snapshot grammar.
pub fn serialize_declaration_for_snapshot(input: &CssInput, declaration: &Declaration) -> String {
    let mut out = String::new();
    write_declaration(&mut out, input, declaration, None, 0);
    out
}

/// Serialize one engine-facing declaration value using the stable model snapshot grammar.
pub fn serialize_value_for_snapshot(input: &CssInput, value: &DeclarationValue) -> String {
    let mut out = String::new();
    write_declaration_value(&mut out, input, value, 0);
    out
}

pub fn serialize_stylesheet_parse_for_snapshot(parse: &StylesheetParse) -> String {
    let mut out = serialize_stylesheet_for_snapshot(&parse.input, &parse.stylesheet);
    serialize_diagnostics_for_snapshot(&mut out, &parse.diagnostics);
    serialize_stats_for_snapshot(&mut out, &parse.stats);
    out
}

fn write_rule(
    out: &mut String,
    input: &CssInput,
    rule: &Rule,
    index: Option<usize>,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);

    match rule {
        Rule::Style(rule) => {
            writeln!(
                out,
                "{indent_str}{}style @{}..{}",
                indexed_label("rule", index),
                rule.span.start,
                rule.span.end
            )
            .expect("write style rule header");
            writeln!(out, "{}  selectors", indent_str).expect("write selectors header");
            write_selector_parse_result_snapshot_body(out, &rule.selectors, indent + 4);
            writeln!(
                out,
                "{}  declarations @{}..{}",
                indent_str, rule.declarations.span.start, rule.declarations.span.end
            )
            .expect("write declaration block header");
            for (declaration_index, declaration) in
                rule.declarations.declarations.iter().enumerate()
            {
                write_declaration(out, input, declaration, Some(declaration_index), indent + 4);
            }
        }
        Rule::At(rule) => {
            writeln!(
                out,
                "{indent_str}{}at(name={}) @{}..{}",
                indexed_label("rule", index),
                rule.name
                    .as_deref()
                    .map(quoted_raw)
                    .unwrap_or_else(|| "<invalid-name>".to_string()),
                rule.span.start,
                rule.span.end
            )
            .expect("write at-rule header");
            write_component_list(out, input, "prelude", &rule.prelude, indent + 2);
            match &rule.block {
                Some(AtRuleBlock::Preserved(block)) => {
                    write_preserved_block(out, input, block, indent + 2)
                }
                None => writeln!(out, "{}  block @<none>", indent_str).expect("write absent block"),
            }
        }
    }
}

fn write_declaration(
    out: &mut String,
    input: &CssInput,
    declaration: &Declaration,
    index: Option<usize>,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);

    writeln!(
        out,
        "{indent_str}{}@{}..{}",
        indexed_label("declaration", index),
        declaration.span.start,
        declaration.span.end
    )
    .expect("write declaration header");
    writeln!(
        out,
        "{}  name(kind={}, text={}) {}",
        indent_str,
        property_name_kind_label(declaration.name.kind),
        declaration
            .name
            .text
            .as_deref()
            .map(quoted_raw)
            .unwrap_or_else(|| "<invalid-name>".to_string()),
        span_label(declaration.name.span),
    )
    .expect("write property name");
    write_declaration_value(out, input, &declaration.value, indent + 2);
    writeln!(
        out,
        "{}  important {}",
        indent_str,
        declaration
            .important
            .as_ref()
            .map(|important| format!("@{}..{}", important.span.start, important.span.end))
            .unwrap_or_else(|| "@<none>".to_string())
    )
    .expect("write important annotation");
}

fn write_declaration_value(
    out: &mut String,
    input: &CssInput,
    value: &DeclarationValue,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);

    writeln!(
        out,
        "{indent_str}value @{}..{}",
        value.span.start, value.span.end
    )
    .expect("write declaration value span");
    for component in &value.components {
        writeln!(
            out,
            "{}  - {}",
            indent_str,
            value_component_snapshot(input, component)
        )
        .expect("write declaration value");
    }
}

fn write_component_list(
    out: &mut String,
    input: &CssInput,
    label: &str,
    list: &PreservedComponentList,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    writeln!(out, "{indent_str}{label} {}", span_label(list.span))
        .expect("write component list header");
    for value in &list.values {
        writeln!(
            out,
            "{indent_str}  - {}",
            component_value_snapshot(input, value)
        )
        .expect("write component list value");
    }
}

fn indexed_label(label: &str, index: Option<usize>) -> String {
    match index {
        Some(index) => format!("{label}[{index}] "),
        None => format!("{label} "),
    }
}

fn write_preserved_block(
    out: &mut String,
    input: &CssInput,
    block: &PreservedBlock,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    writeln!(
        out,
        "{indent_str}block(kind=preserved:{}) @{}..{}",
        block_kind_label(block.kind),
        block.span.start,
        block.span.end
    )
    .expect("write preserved block header");
    for value in &block.values {
        writeln!(
            out,
            "{indent_str}  - {}",
            component_value_snapshot(input, value)
        )
        .expect("write preserved block value");
    }
}

fn component_value_snapshot(input: &CssInput, value: &CssComponentValue) -> String {
    match value {
        CssComponentValue::PreservedToken(token) => {
            format!(
                "token({}) @{}..{}",
                token_kind_snapshot(input, &token.kind),
                token.span.start,
                token.span.end
            )
        }
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

fn value_component_snapshot(input: &CssInput, value: &ValueComponent) -> String {
    match value {
        ValueComponent::Token(token) => value_token_snapshot(input, token),
        ValueComponent::SimpleBlock(block) => value_block_snapshot(input, block),
        ValueComponent::Function(function) => value_function_snapshot(input, function),
    }
}

fn value_token_snapshot(_input: &CssInput, token: &ValueToken) -> String {
    match token {
        ValueToken::Whitespace { span } => format!("whitespace @{}..{}", span.start, span.end),
        ValueToken::Comment { span, text } => format!(
            "comment({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Ident { span, text } => format!(
            "ident({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::AtKeyword { span, text } => format!(
            "at-keyword({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Hash { span, kind, text } => format!(
            "hash(kind={}, text={}) @{}..{}",
            match kind {
                crate::syntax::CssHashKind::Id => "id",
                crate::syntax::CssHashKind::Unrestricted => "unrestricted",
            },
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::String { span, text } => format!(
            "string({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::BadString { span } => format!("bad-string @{}..{}", span.start, span.end),
        ValueToken::Url { span, text } => format!(
            "url({}) @{}..{}",
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::BadUrl { span } => format!("bad-url @{}..{}", span.start, span.end),
        ValueToken::Delim { span, value } => format!(
            "delim({}) @{}..{}",
            quoted_raw(&value.to_string()),
            span.start,
            span.end
        ),
        ValueToken::Number { span, kind, text } => format!(
            "number(kind={}, text={}) @{}..{}",
            numeric_kind_label(*kind),
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Percentage { span, kind, text } => format!(
            "percentage(kind={}, text={}) @{}..{}",
            numeric_kind_label(*kind),
            value_text_snapshot(text),
            span.start,
            span.end
        ),
        ValueToken::Dimension {
            span,
            kind,
            number,
            unit,
        } => format!(
            "dimension(kind={}, number={}, unit={}) @{}..{}",
            numeric_kind_label(*kind),
            value_text_snapshot(number),
            value_text_snapshot(unit),
            span.start,
            span.end
        ),
        ValueToken::UnicodeRange { span, range } => format!(
            "unicode-range(U+{:X}-{:X}) @{}..{}",
            range.start(),
            range.end(),
            span.start,
            span.end
        ),
        ValueToken::Symbol { span, kind } => format!(
            "symbol({}) @{}..{}",
            value_symbol_label(*kind),
            span.start,
            span.end
        ),
    }
}

fn value_block_snapshot(input: &CssInput, block: &ValueBlock) -> String {
    let nested = block
        .components
        .iter()
        .map(|value| value_component_snapshot(input, value))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "simple-block(kind={}, components=[{}]) @{}..{}",
        block_kind_label(block.kind),
        nested,
        block.span.start,
        block.span.end
    )
}

fn value_function_snapshot(input: &CssInput, function: &ValueFunction) -> String {
    let nested = function
        .components
        .iter()
        .map(|value| value_component_snapshot(input, value))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "function(name={}, components=[{}]) @{}..{}",
        value_text_snapshot(&function.name),
        nested,
        function.span.start,
        function.span.end
    )
}

fn value_text_snapshot(text: &ValueText) -> String {
    match &text.text {
        Some(text) => quoted_raw(text),
        None => "<invalid-text>".to_string(),
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
                crate::syntax::CssHashKind::Id => "id",
                crate::syntax::CssHashKind::Unrestricted => "unrestricted",
            },
            quoted_text(input, value)
        ),
        CssTokenKind::String(text) => format!("string({})", quoted_text(input, text)),
        CssTokenKind::BadString => "bad-string".to_string(),
        CssTokenKind::Url(text) => format!("url({})", quoted_text(input, text)),
        CssTokenKind::BadUrl => "bad-url".to_string(),
        CssTokenKind::Delim(ch) => format!("delim({})", quoted_raw(&ch.to_string())),
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
            format!("unicode-range(U+{:X}-{:X})", range.start(), range.end())
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

fn origin_label(origin: CssParseOrigin) -> &'static str {
    match origin {
        CssParseOrigin::Stylesheet => "stylesheet",
        CssParseOrigin::StyleAttribute => "style-attribute",
    }
}

fn property_name_kind_label(kind: PropertyNameKind) -> &'static str {
    match kind {
        PropertyNameKind::Standard => "standard",
        PropertyNameKind::Custom => "custom",
        PropertyNameKind::Invalid => "invalid",
    }
}

fn value_symbol_label(kind: ValueSymbol) -> &'static str {
    match kind {
        ValueSymbol::Colon => "colon",
        ValueSymbol::Semicolon => "semicolon",
        ValueSymbol::Comma => "comma",
        ValueSymbol::LeftSquareBracket => "left-square-bracket",
        ValueSymbol::RightSquareBracket => "right-square-bracket",
        ValueSymbol::LeftParenthesis => "left-parenthesis",
        ValueSymbol::RightParenthesis => "right-parenthesis",
        ValueSymbol::LeftCurlyBracket => "left-curly-bracket",
        ValueSymbol::RightCurlyBracket => "right-curly-bracket",
        ValueSymbol::IncludeMatch => "include-match",
        ValueSymbol::DashMatch => "dash-match",
        ValueSymbol::PrefixMatch => "prefix-match",
        ValueSymbol::SuffixMatch => "suffix-match",
        ValueSymbol::SubstringMatch => "substring-match",
        ValueSymbol::Column => "column",
        ValueSymbol::Cdo => "cdo",
        ValueSymbol::Cdc => "cdc",
    }
}

fn span_label(span: Option<crate::syntax::CssSpan>) -> String {
    match span {
        Some(span) => format!("@{}..{}", span.start, span.end),
        None => "@<none>".to_string(),
    }
}

fn numeric_kind_label(kind: CssNumericKind) -> &'static str {
    match kind {
        CssNumericKind::Integer => "integer",
        CssNumericKind::Number => "number",
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
        Some(text) => quoted_raw(&text),
        None => "<invalid-span>".to_string(),
    }
}

fn quoted_raw(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
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
