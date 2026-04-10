use css::syntax::parse_stylesheet_with_options as parse_syntax_stylesheet_with_options;
use css::{
    CssRule, ParseOptions, SelectorListParseResult, parse_selector_list,
    serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot,
};

fn fixture_input(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

fn parse_selector_fixture(selector_source: &str) -> SelectorListParseResult {
    let stylesheet = format!("{selector_source} {{ color: red; }}");
    let parse = parse_syntax_stylesheet_with_options(
        fixture_input(&stylesheet),
        &ParseOptions::stylesheet(),
    );
    let rule = parse.stylesheet.rules.first().expect("style rule");
    let CssRule::Qualified(rule) = rule else {
        panic!("expected qualified rule");
    };
    parse_selector_list(&parse.input, &rule.prelude)
}

#[test]
fn selector_list_snapshot_golden_representative_selector() {
    let result = parse_selector_fixture(fixture_input(include_str!(
        "fixtures/selectors/representative_list.selector"
    )));
    let list = result.parsed().expect("parsed selector list");
    assert_eq!(
        serialize_selector_list_for_snapshot(list),
        include_str!("fixtures/selectors/representative_list.list.snap"),
    );
}

#[test]
fn selector_parse_snapshot_golden_representative_selector() {
    let result = parse_selector_fixture(fixture_input(include_str!(
        "fixtures/selectors/representative_list.selector"
    )));
    assert_eq!(
        serialize_selector_parse_result_for_snapshot(&result),
        include_str!("fixtures/selectors/representative_list.parse.snap"),
    );
}

#[test]
fn selector_parse_snapshot_golden_unsupported_selector() {
    let result = parse_selector_fixture(fixture_input(include_str!(
        "fixtures/selectors/unsupported_features.selector"
    )));
    assert_eq!(
        serialize_selector_parse_result_for_snapshot(&result),
        include_str!("fixtures/selectors/unsupported_features.parse.snap"),
    );
}

#[test]
fn selector_parse_snapshot_golden_invalid_selector() {
    let result = parse_selector_fixture(fixture_input(include_str!(
        "fixtures/selectors/invalid_selector.selector"
    )));
    assert_eq!(
        serialize_selector_parse_result_for_snapshot(&result),
        include_str!("fixtures/selectors/invalid_selector.parse.snap"),
    );
}
