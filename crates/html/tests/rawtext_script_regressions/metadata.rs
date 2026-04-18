use super::fixtures::{FixtureMeta, TextModeFixtureMode};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const META_FORMAT_V1: &str = "html5-rawtext-script-regression-v1";

/// Metadata parsing and validation for rawtext/script regression fixtures.
pub(super) fn parse_meta_file(path: &Path) -> FixtureMeta {
    let content = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!(
            "failed to read regression metadata {}: {err}",
            path.display()
        )
    });
    let mut headers = BTreeMap::<String, String>::new();
    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        let Some(stripped) = line.strip_prefix('#') else {
            panic!(
                "invalid metadata line in {}: expected '# key: value', got '{}'",
                path.display(),
                line
            );
        };
        let header = stripped.trim();
        if header.is_empty() {
            continue;
        }
        let (key, value) = header
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid metadata header in {}: '{line}'", path.display()));
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if headers.insert(key.clone(), value).is_some() {
            panic!("duplicate metadata header '{key}' in {}", path.display());
        }
    }

    assert_eq!(
        headers.get("format").map(String::as_str),
        Some(META_FORMAT_V1),
        "rawtext/script regression metadata {} must declare # format: {}",
        path.display(),
        META_FORMAT_V1
    );
    let tool = required_header(&headers, "tool", path);
    assert!(
        matches!(
            tool.as_str(),
            "html5_tokenizer_script_data" | "html5_tokenizer_rawtext"
        ),
        "unsupported # tool in {}: '{}'",
        path.display(),
        tool
    );
    let seed = required_header(&headers, "seed", path);
    let date = required_header(&headers, "date", path);
    assert_valid_iso_date(&date, path);
    let issue = required_header(&headers, "issue", path);
    assert_valid_issue_reference(&issue, path);
    let guard = required_header(&headers, "guard", path);
    let mode = TextModeFixtureMode::from_header(&required_header(&headers, "mode", path), path);
    let source = headers.get("source").cloned();

    FixtureMeta {
        tool,
        seed,
        date,
        issue,
        guard,
        source,
        mode,
    }
}

fn required_header(headers: &BTreeMap<String, String>, key: &str, path: &Path) -> String {
    let value = headers.get(key).cloned().unwrap_or_else(|| {
        panic!(
            "missing required metadata header '{key}' in {}",
            path.display()
        )
    });
    assert!(
        !value.trim().is_empty(),
        "metadata header '{key}' in {} must be non-empty",
        path.display()
    );
    value
}

fn assert_valid_iso_date(value: &str, path: &Path) {
    let bytes = value.as_bytes();
    let valid = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(idx, byte)| matches!(idx, 4 | 7) || byte.is_ascii_digit());
    assert!(
        valid,
        "metadata header 'date' in {} must be YYYY-MM-DD, got '{}'",
        path.display(),
        value
    );
}

fn assert_valid_issue_reference(value: &str, path: &Path) {
    assert!(
        is_issue_url(value) || is_stable_issue_id(value),
        "metadata header 'issue' in {} must be an issue URL or a stable issue id (accepted forms: https://..., #1234, BOR-123, Milestone-L/L5), got '{}'",
        path.display(),
        value
    );
}

fn is_issue_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once("://") else {
        return false;
    };
    matches!(scheme, "http" | "https")
        && !rest.is_empty()
        && !rest.starts_with('/')
        && !value.bytes().any(|byte| byte.is_ascii_whitespace())
}

fn is_stable_issue_id(value: &str) -> bool {
    is_hash_issue_id(value) || is_tracker_issue_id(value) || is_scoped_issue_id(value)
}

fn is_hash_issue_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix('#') else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_tracker_issue_id(value: &str) -> bool {
    let Some((prefix, number)) = value.split_once('-') else {
        return false;
    };
    !prefix.is_empty()
        && !number.is_empty()
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        && number.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_scoped_issue_id(value: &str) -> bool {
    let Some((scope, issue)) = value.split_once('/') else {
        return false;
    };
    is_issue_component(scope) && is_issue_component(issue)
}

fn is_issue_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
