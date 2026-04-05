use css::{
    ParseOptions, parse_declarations_with_options, parse_stylesheet_with_options,
    serialize_declaration_list_parse_for_snapshot, serialize_stylesheet_parse_for_snapshot,
    serialize_tokenization_for_snapshot, tokenize_str,
};

fn fixture_input(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

#[test]
fn tokenizer_snapshot_golden_basic_stylesheet() {
    let tokenization = tokenize_str(fixture_input(include_str!(
        "fixtures/tokenizer/basic_stylesheet.css"
    )));
    assert_eq!(
        serialize_tokenization_for_snapshot(&tokenization),
        include_str!("fixtures/tokenizer/basic_stylesheet.snap"),
    );
}

#[test]
fn tokenizer_snapshot_golden_malformed_comment() {
    let tokenization = tokenize_str(fixture_input(include_str!(
        "fixtures/tokenizer/malformed_comment.css"
    )));
    assert_eq!(
        serialize_tokenization_for_snapshot(&tokenization),
        include_str!("fixtures/tokenizer/malformed_comment.snap"),
    );
}

#[test]
fn stylesheet_snapshot_golden_structured_valid() {
    let parse = parse_stylesheet_with_options(
        fixture_input(include_str!("fixtures/parser/structured_valid.css")),
        &ParseOptions::stylesheet(),
    );
    assert_eq!(
        serialize_stylesheet_parse_for_snapshot(&parse),
        include_str!("fixtures/parser/structured_valid.snap"),
    );
}

#[test]
fn stylesheet_snapshot_golden_malformed_recovery() {
    let parse = parse_stylesheet_with_options(
        fixture_input(include_str!("fixtures/parser/malformed_recovery.css")),
        &ParseOptions::stylesheet(),
    );
    assert_eq!(
        serialize_stylesheet_parse_for_snapshot(&parse),
        include_str!("fixtures/parser/malformed_recovery.snap"),
    );
}

#[test]
fn declaration_list_snapshot_golden_malformed_recovery() {
    let parse = parse_declarations_with_options(
        fixture_input(include_str!("fixtures/declarations/malformed_recovery.css")),
        &ParseOptions::style_attribute(),
    );
    assert_eq!(
        serialize_declaration_list_parse_for_snapshot(&parse),
        include_str!("fixtures/declarations/malformed_recovery.snap"),
    );
}
