use ring::digest::{SHA256, digest};
use std::fs;

use crate::wpt_manifest::WptCase;

const WPT_COMMIT: &str = "2c705104a295c48053eeddf7fe0170d790a4e853";

struct ExactDatCase {
    id: &'static str,
    source: &'static str,
    data: &'static str,
    sha256: &'static str,
    errors: &'static str,
    document: &'static str,
}

const AE9B_SELECT_CASES: &[ExactDatCase] = &[
    ExactDatCase {
        id: "select-nested-formatting",
        source: "html/syntax/parsing/resources/tests1.dat",
        data: "<select><b><option><select><option></b></select>X",
        sha256: "3bdc2731c4f57fb934769c10bae07732df31ac89b8f90aadb9161ca9cc2b16d7",
        errors: "1:1: ERROR: Expected a doctype token\n1:20: ERROR: Start tag 'select' isn't allowed here. Currently open tags: html, body, select, b, option.\n1:36: ERROR: End tag 'b' isn't allowed here. Currently open tags: html, body, b, select, option.\n1:50: ERROR: Premature end of file. Currently open tags: html, body, b.",
        document: "| <html>\n|   <head>\n|   <body>\n|     <select>\n|       <b>\n|         <option>\n|     <b>\n|       <option>\n|     \"X\"",
    },
    ExactDatCase {
        id: "select-input-recovery",
        source: "html/syntax/parsing/resources/tests7.dat",
        data: "<!doctype html><select><input>X",
        sha256: "44c9d0ea49afb6e5b42d61af6bc8dcd77e168c46b5dff45aab2f701894c42226",
        errors: "1:32: ERROR: Premature end of file. Currently open tags: html, body, select.",
        document: "| <!DOCTYPE html>\n| <html>\n|   <head>\n|   <body>\n|     <select>\n|     <input>\n|     \"X\"",
    },
    ExactDatCase {
        id: "select-nested-simple",
        source: "html/syntax/parsing/resources/tests7.dat",
        data: "<!doctype html><select><select>X",
        sha256: "81fa58555ea62a7dc493cfaab2931ddc191fd8192a8eae46e93bef04d9ceafd3",
        errors: "1:24: ERROR: Start tag 'select' isn't allowed here. Currently open tags: html, body, select.",
        document: "| <!DOCTYPE html>\n| <html>\n|   <head>\n|   <body>\n|     <select>\n|     \"X\"",
    },
    ExactDatCase {
        id: "select-table-foster-option",
        source: "html/syntax/parsing/resources/tables01.dat",
        data: "<table><select><option>3</select></table>",
        sha256: "2540a8c59f9046301e1483f5458a7a44b96a72a1bd16947d9451142c4713994e",
        errors: "1:1: ERROR: Expected a doctype token\n1:8: ERROR: Start tag 'select' isn't allowed here. Currently open tags: html, body, table.\n1:16: ERROR: Start tag 'option' isn't allowed here. Currently open tags: html, body, table, select.\n1:24: ERROR: Character tokens aren't legal here\n1:25: ERROR: End tag 'select' isn't allowed here. Currently open tags: html, body, table, select, option.",
        document: "| <html>\n|   <head>\n|   <body>\n|     <select>\n|       <option>\n|         \"3\"\n|     <table>",
    },
    ExactDatCase {
        id: "select-table-token-open",
        source: "html/syntax/parsing/resources/tables01.dat",
        data: "<table><select><table></table></select></table>",
        sha256: "74bbf12f95de0fb0a8fb12da0c07949a5f5c6e223f7743dba7b34ba68cf5c428",
        errors: "1:1: ERROR: Expected a doctype token\n1:8: ERROR: Start tag 'select' isn't allowed here. Currently open tags: html, body, table.\n1:16: ERROR: Start tag 'table' isn't allowed here. Currently open tags: html, body, table, select.\n1:31: ERROR: End tag 'select' isn't allowed here. Currently open tags: html, body.\n1:40: ERROR: End tag 'table' isn't allowed here. Currently open tags: html, body.",
        document: "| <html>\n|   <head>\n|   <body>\n|     <select>\n|     <table>\n|     <table>",
    },
    ExactDatCase {
        id: "select-table-row-recovery",
        source: "html/syntax/parsing/resources/tables01.dat",
        data: "<table><select><option>A<tr><td>B</td></tr></table>",
        sha256: "9a056bd114975bc358f15fca4e2c54457ef1742b3d507216cad6f5f57817fd77",
        errors: "1:1: ERROR: Expected a doctype token\n1:8: ERROR: Start tag 'select' isn't allowed here. Currently open tags: html, body, table.\n1:16: ERROR: Start tag 'option' isn't allowed here. Currently open tags: html, body, table, select.\n1:24: ERROR: Character tokens aren't legal here",
        document: "| <html>\n|   <head>\n|   <body>\n|     <select>\n|       <option>\n|         \"A\"\n|     <table>\n|       <tbody>\n|         <tr>\n|           <td>\n|             \"B\"",
    },
];

pub(crate) fn validate_ae9b_select_case(case: &WptCase) {
    let exact = AE9B_SELECT_CASES
        .iter()
        .find(|exact| exact.id == case.id)
        .unwrap_or_else(|| panic!("missing exact AE9b provenance oracle for '{}'", case.id));
    let provenance_path = case
        .provenance
        .as_ref()
        .unwrap_or_else(|| panic!("AE9b case '{}' requires provenance", case.id));
    let input = fs::read(&case.path)
        .unwrap_or_else(|err| panic!("failed reading AE9b input {:?}: {err}", case.path));
    assert_eq!(
        input,
        exact.data.as_bytes(),
        "AE9b adapted input '{}' differs from the exact pinned #data bytes",
        case.id
    );
    assert_ne!(
        input.last(),
        Some(&b'\n'),
        "AE9b input must not add a newline"
    );
    assert_eq!(sha256_hex(&input), exact.sha256, "AE9b input SHA-256");

    let provenance = fs::read_to_string(provenance_path).unwrap_or_else(|err| {
        panic!(
            "failed reading AE9b provenance {:?}: {err}",
            provenance_path
        )
    });
    assert!(provenance.contains("format: wpt-dat-case-provenance-v1\n"));
    assert!(provenance.contains(&format!("wpt-commit: {WPT_COMMIT}\n")));
    assert!(provenance.contains(&format!("source: {}\n", exact.source)));
    assert!(provenance.contains("context: full-document\n"));
    assert!(provenance.contains("scripting: not-applicable\n"));
    assert!(provenance.contains(&format!("data-sha256: {}\n", exact.sha256)));
    assert!(provenance.contains("adaptation: exact #data bytes extracted without a terminal newline; upstream #document translated only into html5-dom-v1 representation\n"));

    let (_, sections) = provenance
        .split_once("#data\n")
        .unwrap_or_else(|| panic!("AE9b provenance '{}' missing #data", case.id));
    let (data, sections) = sections
        .split_once("\n#errors\n")
        .unwrap_or_else(|| panic!("AE9b provenance '{}' missing #errors", case.id));
    let (errors, document) = sections
        .split_once("\n#document\n")
        .unwrap_or_else(|| panic!("AE9b provenance '{}' missing #document", case.id));
    assert_eq!(data, exact.data, "AE9b provenance #data for '{}'", case.id);
    assert_eq!(
        errors, exact.errors,
        "AE9b provenance #errors for '{}'",
        case.id
    );
    assert_eq!(
        document.trim_end_matches('\n'),
        exact.document,
        "AE9b provenance #document for '{}'",
        case.id
    );

    let expected = fs::read_to_string(&case.expected)
        .unwrap_or_else(|err| panic!("failed reading AE9b expected {:?}: {err}", case.expected));
    assert_eq!(
        expected.trim_end_matches('\n'),
        translate_upstream_document(exact.document),
        "AE9b expected DOM '{}' must be only a representation translation of pinned WPT",
        case.id
    );
}

pub(crate) fn is_ae9b_select_case(id: &str) -> bool {
    AE9B_SELECT_CASES.iter().any(|exact| exact.id == id)
}

fn translate_upstream_document(document: &str) -> String {
    let mut translated = String::from("# format: html5-dom-v1\n#document");
    for line in document.lines() {
        let line = line
            .strip_prefix("| ")
            .expect("pinned WPT #document line must begin with '| '");
        translated.push('\n');
        translated.push_str("  ");
        if line == "<!DOCTYPE html>" {
            translated.push_str("<!doctype html>");
        } else {
            translated.push_str(line);
        }
    }
    translated
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
