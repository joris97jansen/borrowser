#![cfg(all(feature = "html5", feature = "dom-snapshot"))]

use html::dom_snapshot::{DomSnapshot, DomSnapshotOptions};
use ring::digest::{SHA256, digest};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const EXPECTED_PROFILE: &str = "AE12-WHATWG-processing-instruction-profile-v1";
const EXPECTED_HTML_COMMIT: &str = "24c5e48bf66ea61bc199ec6338c81258275ba9c6";
const EXPECTED_DOM_COMMIT: &str = "8a5f57c61ca1de8dc21b7e114501b1b57882e935";
const EXPECTED_WPT_COMMIT: &str = "4809b72f863e05ab1df710d3390547dd86694239";
const EXPECTED_HTML_SOURCE_SHA256: &str =
    "b160c424aacc4116168174b90ae91b29df6a48af25be660ceac3862daef495fa";
const EXPECTED_DOM_SOURCE_SHA256: &str =
    "435d8941a603e0d7d0ac138ff26377113c02bd4d9797d5ca31834b27d28521f9";
const EXPECTED_WPT_SOURCE_SHA256: &str =
    "c408d94b644156590ff5ee552b5f53423fea5aba2fd69e22b8aa0689d627b5b9";
const EXPECTED_WPT_SOURCE: &str = "html/syntax/parsing/resources/processing-instructions.dat";

#[test]
fn ae12_supported_profile_uses_immutable_upstream_revisions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let provenance_path = root.join("tests/wpt/provenance/ae12-supported-profile.provenance.txt");
    let provenance = fs::read_to_string(&provenance_path)
        .unwrap_or_else(|error| panic!("failed to read {provenance_path:?}: {error}"));
    let fields = parse_fields(&provenance);

    assert_eq!(fields.get("profile"), Some(&EXPECTED_PROFILE));
    assert_eq!(
        fields.get("whatwg-html-commit"),
        Some(&EXPECTED_HTML_COMMIT)
    );
    assert_eq!(fields.get("whatwg-dom-commit"), Some(&EXPECTED_DOM_COMMIT));
    assert_eq!(fields.get("wpt-commit"), Some(&EXPECTED_WPT_COMMIT));
    for field in ["whatwg-html-commit", "whatwg-dom-commit", "wpt-commit"] {
        assert_full_sha(field, fields[field]);
    }
    assert_eq!(fields.get("whatwg-html-source"), Some(&"source"));
    assert_eq!(
        fields.get("whatwg-html-source-sha256"),
        Some(&EXPECTED_HTML_SOURCE_SHA256)
    );
    assert_eq!(fields.get("whatwg-dom-source"), Some(&"dom.bs"));
    assert_eq!(
        fields.get("whatwg-dom-source-sha256"),
        Some(&EXPECTED_DOM_SOURCE_SHA256)
    );
    assert_eq!(fields.get("wpt-source"), Some(&EXPECTED_WPT_SOURCE));
    assert_eq!(
        fields.get("wpt-source-sha256"),
        Some(&EXPECTED_WPT_SOURCE_SHA256)
    );
    for field in [
        "whatwg-html-source-sha256",
        "whatwg-dom-source-sha256",
        "wpt-source-sha256",
    ] {
        assert_sha256(field, fields[field]);
    }
    for field in [
        "extracted-cases",
        "adaptation",
        "hardening-policy",
        "deviation-dom-api",
        "deviation-fragment-parsing",
    ] {
        assert!(fields.get(field).is_some_and(|value| !value.is_empty()));
    }

    let contract_path = root.join("docs/html5/ae12-processing-instruction-contract.md");
    let contract = fs::read_to_string(&contract_path)
        .unwrap_or_else(|error| panic!("failed to read {contract_path:?}: {error}"));
    for required in [
        EXPECTED_PROFILE,
        EXPECTED_HTML_COMMIT,
        EXPECTED_DOM_COMMIT,
        EXPECTED_WPT_COMMIT,
        "processing instruction questionable state",
        "resource-hardening",
        "prefix-first",
        "adjusted insertion location",
    ] {
        assert!(
            contract.contains(required),
            "AE12 contract missing {required:?}"
        );
    }
}

#[test]
fn pinned_wpt_derived_full_document_cases_match_the_ae12_profile() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("tests/wpt/provenance/ae12-processing-instructions.cases");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
    let extracted = parse_extracted_cases(&source);

    assert_eq!(extracted.header["format"], "ae12-wpt-extracted-cases-v1");
    assert_eq!(extracted.header["wpt-commit"], EXPECTED_WPT_COMMIT);
    assert_eq!(extracted.header["source"], EXPECTED_WPT_SOURCE);
    assert_eq!(
        extracted.header["source-sha256"],
        EXPECTED_WPT_SOURCE_SHA256
    );
    assert_eq!(extracted.cases.len(), 9);

    let expected_identities = [3usize, 8, 9, 6, 110, 112, 114, 120, 71];
    for (case, expected_identity) in extracted.cases.iter().zip(expected_identities) {
        assert_eq!(
            case.fields["source-case"].parse::<usize>().unwrap(),
            expected_identity,
            "one-based WPT case identity for {}",
            case.fields["case"]
        );
        assert_eq!(
            case.fields["scripting"],
            "upstream-both; borrowser-disabled"
        );
        assert_eq!(
            case.fields["adaptation"],
            "exact #data bytes; full-document parse; no terminal newline"
        );
        assert_eq!(
            case.fields["expected-output-provenance"],
            "exact pinned upstream #document below; representation-only translation"
        );
        assert_sha256("case data-sha256", case.fields["data-sha256"]);
        assert_eq!(
            sha256_hex(case.data.as_bytes()),
            case.fields["data-sha256"],
            "exact #data hash for {}",
            case.fields["case"]
        );

        let document = html::parse_document(case.data, html::HtmlParseOptions::default())
            .unwrap_or_else(|error| panic!("case {} failed: {error:?}", case.fields["case"]))
            .document;
        let actual = DomSnapshot::new(&document, DomSnapshotOptions::default()).render();
        assert_eq!(
            actual.trim_end_matches('\n'),
            translate_upstream_document(case.document),
            "pinned expected output for {}",
            case.fields["case"]
        );
    }
}

struct ExtractedProfile<'a> {
    header: BTreeMap<&'a str, &'a str>,
    cases: Vec<ExtractedCase<'a>>,
}

struct ExtractedCase<'a> {
    fields: BTreeMap<&'a str, &'a str>,
    data: &'a str,
    document: &'a str,
}

fn parse_extracted_cases(input: &str) -> ExtractedProfile<'_> {
    let (header, records) = input
        .split_once("\n\ncase: ")
        .expect("extracted AE12 cases require a header and records");
    let mut cases = Vec::new();
    for record in records
        .split("\n#end\n")
        .filter(|record| !record.is_empty())
    {
        let (case_line, record) = record.split_once('\n').expect("case identity line");
        let case_id = case_line.strip_prefix("case: ").unwrap_or(case_line);
        let (metadata, sections) = record.split_once("\n#data\n").expect("case #data");
        let (data, document) = sections
            .split_once("\n#document\n")
            .expect("case #document");
        let mut fields = parse_fields(metadata);
        assert!(fields.insert("case", case_id).is_none());
        cases.push(ExtractedCase {
            fields,
            data,
            document,
        });
    }
    ExtractedProfile {
        header: parse_fields(header),
        cases,
    }
}

fn translate_upstream_document(document: &str) -> String {
    let mut out = String::from("#dom-snapshot-v2\n#document");
    for raw in document.lines() {
        let line = raw
            .strip_prefix("| ")
            .expect("pinned WPT #document line begins with '| '");
        let content = line.trim_start();
        let indent = &line[..line.len() - content.len()];
        out.push('\n');
        out.push_str("  ");
        out.push_str(indent);
        if content == "content" {
            out.push_str("#template-contents");
        } else if let Some(pi) = content
            .strip_prefix("<?")
            .and_then(|value| value.strip_suffix("?>"))
        {
            let (target, data) = pi
                .split_once(' ')
                .expect("pinned WPT PI output separates target and data");
            out.push_str("processing-instruction target=\"");
            out.push_str(target);
            out.push_str("\" data=\"");
            out.push_str(data);
            out.push('"');
        } else if let Some(comment) = content
            .strip_prefix("<!-- ")
            .and_then(|value| value.strip_suffix(" -->"))
        {
            out.push_str("<!-- ");
            out.push_str(comment);
            out.push_str(" -->");
        } else if let Some(local) = content
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
        {
            out.push_str("element ns=html local=\"");
            out.push_str(local);
            out.push_str("\" attrs=[]");
        } else {
            out.push_str(content);
        }
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn parse_fields(input: &str) -> BTreeMap<&str, &str> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_once(": ")
                .unwrap_or_else(|| panic!("invalid provenance line: {line:?}"))
        })
        .collect()
}

fn assert_full_sha(label: &str, value: &str) {
    assert_eq!(value.len(), 40, "{label} must be a complete SHA");
    assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

fn assert_sha256(label: &str, value: &str) {
    assert_eq!(value.len(), 64, "{label} must be a complete SHA-256");
    assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
}
