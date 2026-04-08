use css::ParseOptions;
use css::model::{parse_stylesheet_with_options, serialize_stylesheet_parse_for_snapshot};

fn fixture_input(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

#[test]
fn model_snapshot_golden_representative_stylesheet() {
    let parse = parse_stylesheet_with_options(
        fixture_input(include_str!("fixtures/model/representative_stylesheet.css")),
        &ParseOptions::stylesheet(),
    );
    assert_eq!(
        serialize_stylesheet_parse_for_snapshot(&parse),
        include_str!("fixtures/model/representative_stylesheet.snap"),
    );
}

#[test]
fn model_snapshot_golden_malformed_recovery() {
    let parse = parse_stylesheet_with_options(
        fixture_input(include_str!("fixtures/model/malformed_recovery.css")),
        &ParseOptions::stylesheet(),
    );
    assert_eq!(
        serialize_stylesheet_parse_for_snapshot(&parse),
        include_str!("fixtures/model/malformed_recovery.snap"),
    );
}
