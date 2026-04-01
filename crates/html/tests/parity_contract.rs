#![cfg(all(feature = "html5", feature = "dom-snapshot"))]

use html::html5::serialize_dom_for_test;
use html::{HtmlParseOptions, parse_document};

fn dom_lines(input: &str) -> Vec<String> {
    let output = parse_document(input, HtmlParseOptions::default()).expect("parse should work");
    serialize_dom_for_test(&output.document)
}

#[test]
fn parity_contract_no_quirks_table_closes_open_p() {
    assert_eq!(
        dom_lines("<!doctype html><p><table>"),
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <p>".to_string(),
            "      <table>".to_string(),
        ]
    );
}

#[test]
fn parity_contract_quirks_table_keeps_open_p() {
    assert_eq!(
        dom_lines("<!doctype foo><p><table>"),
        vec![
            "#document doctype=\"foo\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <p>".to_string(),
            "        <table>".to_string(),
        ]
    );
}

#[test]
fn parity_contract_stray_end_tag_recovery_is_stable() {
    assert_eq!(
        dom_lines("</div><p>ok</p>"),
        vec![
            "#document".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <p>".to_string(),
            "        \"ok\"".to_string(),
        ]
    );
}
