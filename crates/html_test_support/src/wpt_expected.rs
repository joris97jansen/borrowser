use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::wpt_formats::{EXPECTED_DOM_FORMAT_V1, EXPECTED_TOKEN_FORMAT_V1};

pub struct ParsedExpectedDom {
    pub ignore_ids: bool,
    pub ignore_empty_style: bool,
    pub lines: Vec<String>,
}

pub fn parse_expected_tokens(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read expected tokens file {path:?}: {err}"));
    let (headers, lines) =
        parse_headers_and_lines(&content, path, &["format"], InputKind::TokenSnapshot);
    let format = headers.get("format").expect("format header validated");
    assert_eq!(
        format, EXPECTED_TOKEN_FORMAT_V1,
        "unsupported format in {path:?}"
    );
    assert!(
        !lines.is_empty(),
        "expected tokens file {path:?} has no token lines"
    );
    assert!(
        lines.last().map(String::as_str) == Some("EOF"),
        "expected tokens file {path:?} must end with EOF"
    );
    lines
}

pub fn parse_expected_dom(path: &Path) -> ParsedExpectedDom {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read expected DOM file {path:?}: {err}"));
    let (headers, lines) = parse_headers_and_lines(
        &content,
        path,
        &["format", "ignore_ids", "ignore_empty_style"],
        InputKind::DomSnapshot,
    );
    let format = headers.get("format").expect("format header validated");
    assert_eq!(
        format, EXPECTED_DOM_FORMAT_V1,
        "unsupported format in {path:?}"
    );
    assert!(
        !lines.is_empty(),
        "expected DOM file {path:?} has no snapshot lines"
    );
    assert!(
        is_document_root_line(&lines[0]),
        "expected DOM file {path:?} must start with #document"
    );

    ParsedExpectedDom {
        ignore_ids: header_bool(&headers, "ignore_ids", true, path),
        ignore_empty_style: header_bool(&headers, "ignore_empty_style", true, path),
        lines,
    }
}

enum InputKind {
    DomSnapshot,
    TokenSnapshot,
}

fn parse_headers_and_lines(
    content: &str,
    path: &Path,
    supported_headers: &[&str],
    kind: InputKind,
) -> (BTreeMap<String, String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut headers = BTreeMap::<String, String>::new();

    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        if matches!(kind, InputKind::DomSnapshot) && is_document_root_line(line) {
            lines.push(line.to_string());
            continue;
        }
        if let Some(stripped) = line.strip_prefix('#') {
            let header = stripped.trim();
            if header.is_empty() {
                continue;
            }
            if let Some((key, value)) = header.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                assert!(
                    supported_headers.contains(&key.as_str()),
                    "unsupported header '{key}' in {path:?}"
                );
                if headers.is_empty() {
                    assert_eq!(
                        key, "format",
                        "first header must be 'format' in {path:?}, found '{key}'"
                    );
                }
                if headers.insert(key.clone(), value).is_some() {
                    panic!("duplicate header '{key}' in {path:?}");
                }
            } else {
                continue;
            }
        } else {
            lines.push(line.to_string());
        }
    }

    assert!(
        headers.contains_key("format"),
        "missing required 'format' header in {path:?}"
    );
    (headers, lines)
}

fn header_bool(headers: &BTreeMap<String, String>, key: &str, default: bool, path: &Path) -> bool {
    match headers.get(key).map(|s| s.as_str()) {
        None => default,
        Some("true") => true,
        Some("false") => false,
        Some(other) => panic!("invalid boolean '{other}' for {key} in {path:?}"),
    }
}

fn is_document_root_line(line: &str) -> bool {
    line == "#document" || line.starts_with("#document ")
}
