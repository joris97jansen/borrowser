use super::{tokenize_str, tokenize_str_with_options};
use crate::syntax::{DiagnosticKind, ParseOptions};

#[test]
fn tokenizer_emits_core_stylesheet_tokens_deterministically() {
    let first = tokenize_str("div, #hero { color: 10px; }");
    let second = tokenize_str("div, #hero { color: 10px; }");

    assert_eq!(first.diagnostics, second.diagnostics);
    assert_eq!(first.to_debug_snapshot(), second.to_debug_snapshot());
    assert_eq!(
        first.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "tokens\n",
            "token[0] ident(\"div\") @0..3\n",
            "token[1] comma @3..4\n",
            "token[2] whitespace @4..5\n",
            "token[3] hash(kind=id, value=\"hero\") @5..10\n",
            "token[4] whitespace @10..11\n",
            "token[5] left-curly-bracket @11..12\n",
            "token[6] whitespace @12..13\n",
            "token[7] ident(\"color\") @13..18\n",
            "token[8] colon @18..19\n",
            "token[9] whitespace @19..20\n",
            "token[10] dimension(kind=integer, value=\"10\", unit=\"px\") @20..24\n",
            "token[11] semicolon @24..25\n",
            "token[12] whitespace @25..26\n",
            "token[13] right-curly-bracket @26..27\n",
            "token[14] eof @27..27\n",
            "diagnostics\n",
            "stats\n",
            "  input_bytes: 27\n",
            "  tokens_emitted: 15\n",
            "  diagnostics_emitted: 0\n",
            "  hit_limit: false\n",
        )
    );
}

#[test]
fn tokenizer_handles_comments_strings_and_url_tokens() {
    let tokenization = tokenize_str("/*x*/ a { background: url(icon.svg); content: \"hi\"; }");
    let snapshot = tokenization.to_debug_snapshot();

    assert!(snapshot.contains("comment(\"x\") @0..5"));
    assert!(snapshot.contains("url(\"icon.svg\")"));
    assert!(snapshot.contains("string(\"hi\")"));
}

#[test]
fn tokenizer_reports_malformed_lexical_input_deterministically() {
    let tokenization = tokenize_str("a { content: \"unterminated\n url(bad\"x) } /*");
    let snapshot = tokenization.to_debug_snapshot();

    assert!(snapshot.contains("bad-string"));
    assert!(snapshot.contains("bad-url"));
    assert!(snapshot.contains("warning unterminated-string"));
    assert!(snapshot.contains("warning bad-url"));
    assert!(snapshot.contains("warning unterminated-comment"));
}

#[test]
fn tokenizer_uses_origin_specific_input_limits() {
    let options = ParseOptions::style_attribute();
    let tokenization = tokenize_str_with_options(&"x".repeat(70_000), &options);

    assert!(tokenization.stats.hit_limit);
    assert!(
        tokenization
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::LimitExceeded)
    );
}
