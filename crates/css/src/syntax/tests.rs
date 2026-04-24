use super::{
    CompatSelector, CssRule, DiagnosticKind, ParseOptions, SyntaxLimits, parse_declarations,
    parse_declarations_with_options, parse_stylesheet, parse_stylesheet_with_options,
    tokenize_str_with_options,
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
fn parser_component_container_limit_bounds_declaration_values() {
    let options = ParseOptions {
        limits: SyntaxLimits {
            max_component_values_per_container: 2,
            ..SyntaxLimits::default()
        },
        ..ParseOptions::stylesheet()
    };
    let parse =
        parse_stylesheet_with_options("div { color:red blue green; width:10px; }", &options);

    assert!(parse.stats.hit_limit);
    assert!(
        parse
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::LimitExceeded)
    );
    let CssRule::Qualified(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected qualified rule");
    };
    assert_eq!(rule.block.declarations.len(), 2);
    assert!(rule.block.declarations[0].value.len() <= 2);
    assert_eq!(
        rule.block.declarations[1]
            .name
            .resolve(&parse.input)
            .as_deref(),
        Some("width")
    );
}

#[test]
fn parser_selector_prelude_limit_recovers_at_rule_boundary() {
    let options = ParseOptions {
        limits: SyntaxLimits {
            max_selector_component_values: 2,
            ..SyntaxLimits::default()
        },
        ..ParseOptions::stylesheet()
    };
    let parse =
        parse_stylesheet_with_options("div span { color:red; } p { width:10px; }", &options);
    let compat = parse.to_compat_stylesheet();

    assert!(parse.stats.hit_limit);
    assert_eq!(parse.stylesheet.rules.len(), 1);
    assert_eq!(compat.rules.len(), 1);
    assert_eq!(
        compat.rules[0].selectors,
        vec![CompatSelector::Type("p".to_string())]
    );
    assert!(
        parse
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::LimitExceeded)
    );
}

#[test]
fn malformed_qualified_rule_recovers_at_semicolon_and_preserves_later_rule() {
    let parse =
        parse_stylesheet_with_options("div; span { color: blue; }", &ParseOptions::stylesheet());
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

#[test]
fn compatibility_stylesheet_wrapper_is_only_a_projection_of_structured_parse_output() {
    let input = "div { color: red; }";
    let structured = parse_stylesheet_with_options(input, &ParseOptions::stylesheet());

    assert_eq!(parse_stylesheet(input), structured.to_compat_stylesheet());
}

#[test]
fn compatibility_declaration_wrapper_is_only_a_projection_of_structured_parse_output() {
    let input = "color: red; width: 10px;";
    let structured = parse_declarations_with_options(input, &ParseOptions::style_attribute());

    assert_eq!(parse_declarations(input), structured.declarations);
}
