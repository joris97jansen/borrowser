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
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  <!doctype html>".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"p\" attrs=[]".to_string(),
            "      element ns=html local=\"table\" attrs=[]".to_string(),
        ]
    );
}

#[test]
fn parity_contract_quirks_table_keeps_open_p() {
    assert_eq!(
        dom_lines("<!doctype foo><p><table>"),
        vec![
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  <!doctype foo>".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"p\" attrs=[]".to_string(),
            "        element ns=html local=\"table\" attrs=[]".to_string(),
        ]
    );
}

#[test]
fn parity_contract_stray_end_tag_recovery_is_stable() {
    assert_eq!(
        dom_lines("</div><p>ok</p>"),
        vec![
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"p\" attrs=[]".to_string(),
            "        \"ok\"".to_string(),
        ]
    );
}
