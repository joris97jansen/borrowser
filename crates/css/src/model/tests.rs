use super::{AtRuleBlock, PropertyNameKind, Rule, parse_stylesheet, parse_stylesheet_with_options};
use crate::syntax::{CssComponentValue, CssTokenKind, ParseOptions};

#[test]
fn model_stylesheet_preserves_rule_order_and_supported_rule_kinds() {
    let stylesheet = parse_stylesheet(
        "@MEDIA screen { div { color: red; } } div, span { color: blue; } @unknown foo;",
    );

    assert_eq!(stylesheet.rules.len(), 3);

    let Rule::At(first) = &stylesheet.rules[0] else {
        panic!("expected first rule to be an at-rule");
    };
    assert_eq!(first.name.as_deref(), Some("media"));
    assert!(matches!(first.block, Some(AtRuleBlock::Preserved(_))));

    let Rule::Style(second) = &stylesheet.rules[1] else {
        panic!("expected second rule to be a style rule");
    };
    assert_eq!(second.declarations.declarations.len(), 1);
    assert!(second.selector_source.span.is_some());
    assert!(!second.selector_source.values.is_empty());

    let Rule::At(third) = &stylesheet.rules[2] else {
        panic!("expected third rule to be an at-rule");
    };
    assert_eq!(third.name.as_deref(), Some("unknown"));
    assert!(third.block.is_none());
}

#[test]
fn model_stylesheet_keeps_style_rules_in_source_order_after_syntax_recovery() {
    let parse = parse_stylesheet_with_options(
        "div; span { color: blue; } a { color: red; }",
        &ParseOptions::stylesheet(),
    );

    assert_eq!(parse.stylesheet.rules.len(), 2);
    assert_eq!(parse.diagnostics.len(), 1);

    let Rule::Style(first) = &parse.stylesheet.rules[0] else {
        panic!("expected recovered first surviving rule to be a style rule");
    };
    let Rule::Style(second) = &parse.stylesheet.rules[1] else {
        panic!("expected recovered second surviving rule to be a style rule");
    };

    assert_eq!(
        parse
            .input
            .slice(first.selector_source.span.expect("selector span"))
            .expect("selector source"),
        "span "
    );
    assert_eq!(
        parse
            .input
            .slice(second.selector_source.span.expect("selector span"))
            .expect("selector source"),
        "a "
    );
}

#[test]
fn model_declarations_are_structured_and_preserve_order() {
    let parse = parse_stylesheet_with_options(
        "div { --Brand: red; COLOR: blue !important; font-size: 16px; }",
        &ParseOptions::stylesheet(),
    );

    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected first rule to be a style rule");
    };
    let declarations = &rule.declarations.declarations;

    assert_eq!(declarations.len(), 3);
    assert_eq!(
        declarations
            .iter()
            .map(|declaration| declaration.name.text.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("--Brand"), Some("color"), Some("font-size")]
    );

    assert_eq!(declarations[0].name.kind, PropertyNameKind::Custom);
    assert_eq!(declarations[1].name.kind, PropertyNameKind::Standard);
    assert_eq!(declarations[2].name.kind, PropertyNameKind::Standard);

    let important = declarations[1]
        .important
        .as_ref()
        .expect("important annotation");
    assert_eq!(
        parse
            .input
            .slice(important.span)
            .expect("important annotation slice"),
        "!important"
    );
    assert_eq!(
        parse
            .input
            .slice(declarations[1].value.span)
            .expect("value slice"),
        " blue "
    );
    assert!(!declaration_value_contains_important(
        &parse,
        &declarations[1].value.values
    ));
}

#[test]
fn empty_value_after_important_extraction_uses_zero_length_value_span() {
    let parse =
        parse_stylesheet_with_options("div { color:!important; }", &ParseOptions::stylesheet());

    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected first rule to be a style rule");
    };
    let declaration = &rule.declarations.declarations[0];

    assert!(declaration.value.values.is_empty());
    assert_eq!(declaration.value.span.start, declaration.value.span.end);
    assert_eq!(
        declaration
            .important
            .as_ref()
            .expect("important annotation")
            .span,
        parse.input.span(12, 22).expect("important span")
    );
}

#[test]
fn model_stylesheet_snapshot_is_stable() {
    let parse = parse_stylesheet_with_options(
        "@MEDIA screen { div { color: red; } } div { color: blue; }",
        &ParseOptions::stylesheet(),
    );

    assert_eq!(
        parse.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "model-stylesheet\n",
            "origin: stylesheet\n",
            "rule[0] at(name=\"media\") @0..37\n",
            "  prelude @6..14\n",
            "    - token(whitespace) @6..7\n",
            "    - token(ident(\"screen\")) @7..13\n",
            "    - token(whitespace) @13..14\n",
            "  block(kind=preserved:curly) @14..37\n",
            "    - token(whitespace) @15..16\n",
            "    - token(ident(\"div\")) @16..19\n",
            "    - token(whitespace) @19..20\n",
            "    - simple-block(kind=curly, text=\"{ color: red; }\") @20..35\n",
            "    - token(whitespace) @35..36\n",
            "rule[1] style @38..58\n",
            "  selector @38..42\n",
            "    - token(ident(\"div\")) @38..41\n",
            "    - token(whitespace) @41..42\n",
            "  declarations @42..58\n",
            "    declaration[0] @44..56\n",
            "      name(kind=standard, text=\"color\") @44..49\n",
            "      value @50..55\n",
            "        - token(whitespace) @50..51\n",
            "        - token(ident(\"blue\")) @51..55\n",
            "      important @<none>\n",
            "diagnostics\n",
            "stats\n",
            "  input_bytes: 58\n",
            "  rules_emitted: 2\n",
            "  declarations_emitted: 1\n",
            "  diagnostics_emitted: 0\n",
            "  hit_limit: false\n",
        )
    );
}

fn declaration_value_contains_important(
    parse: &super::StylesheetParse,
    values: &[CssComponentValue],
) -> bool {
    values.iter().any(|value| match value {
        CssComponentValue::PreservedToken(token) => match &token.kind {
            CssTokenKind::Delim('!') => true,
            CssTokenKind::Ident(text) => text
                .resolve(&parse.input)
                .map(|text| text.eq_ignore_ascii_case("important"))
                .unwrap_or(false),
            _ => false,
        },
        _ => false,
    })
}
