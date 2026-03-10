use crate::config::{
    ParserMode, parse_runtime_parser_mode, parser_mode_from_env_with, resolve_parser_mode,
};

#[test]
fn parser_mode_defaults_to_legacy() {
    let mode = parser_mode_from_env_with(|_| None);
    assert_eq!(mode, ParserMode::Legacy);
}

#[test]
fn parser_mode_parses_known_values() {
    assert_eq!(
        parse_runtime_parser_mode(Some("legacy")),
        Some(ParserMode::Legacy)
    );
    assert_eq!(
        parse_runtime_parser_mode(Some("html5")),
        Some(ParserMode::Html5)
    );
    assert_eq!(
        parse_runtime_parser_mode(Some("LeGaCy")),
        Some(ParserMode::Legacy)
    );
}

#[test]
fn parser_mode_from_env_handles_invalid_value() {
    let mode = parser_mode_from_env_with(|_| Some("unknown".to_string()));
    assert_eq!(mode, ParserMode::Legacy);
}

#[cfg(not(feature = "html5"))]
#[test]
fn resolve_parser_mode_falls_back_without_feature() {
    assert_eq!(resolve_parser_mode(ParserMode::Html5), ParserMode::Legacy);
}

#[cfg(feature = "html5")]
#[test]
fn resolve_parser_mode_allows_html5_with_feature() {
    assert_eq!(resolve_parser_mode(ParserMode::Html5), ParserMode::Html5);
}
