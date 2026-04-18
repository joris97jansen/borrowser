use super::super::{
    AtRuleBlock, Declaration, DeclarationValue, PreservedBlock, PreservedComponentList, Rule,
    Stylesheet,
};
use super::labels::{
    block_kind_label, indexed_label, origin_label, property_name_kind_label, quoted_raw, span_label,
};
use super::syntax::component_value_snapshot;
use super::values::value_component_snapshot;
use crate::selectors::write_selector_parse_result_snapshot_body;
use crate::syntax::CssInput;
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
