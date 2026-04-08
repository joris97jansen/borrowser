use super::{AtRuleBlock, Rule, parse_stylesheet, parse_stylesheet_with_options};
use crate::syntax::ParseOptions;

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
            "    declaration[0] \"color\" @44..56\n",
            "      - token(whitespace) @50..51\n",
            "      - token(ident(\"blue\")) @51..55\n",
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
