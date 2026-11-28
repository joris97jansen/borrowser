use css::{ParseOptions, Rule, parse_stylesheet_with_options};

#[test]
fn aj8_escaped_at_rule_names_keep_semantic_classification() {
    let cases = [
        (r"@\69mport 'a.css';", "import"),
        (r"@\6d edia screen {p{color:red}}", "media"),
        (r"@\5c 69mport 'a.css';", r"\69mport"),
        ("@IMPORT 'a.css';", "import"),
    ];
    for (source, name) in cases {
        let parse = parse_stylesheet_with_options(source, &ParseOptions::stylesheet());
        let Rule::At(rule) = &parse.stylesheet.rules[0] else {
            panic!("at-rule")
        };
        assert_eq!(rule.name.as_deref(), Some(name));
        assert_eq!(parse.input.as_str(), source);
        assert_eq!(parse.input.slice(rule.span), Some(source));
    }
}
