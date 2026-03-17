use html::dom_snapshot::DomSnapshotOptions;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FixtureStatus {
    Active,
    Xfail,
}

pub(super) struct ExpectedDom {
    pub(super) status: FixtureStatus,
    pub(super) reason: Option<String>,
    pub(super) options: DomSnapshotOptions,
    pub(super) lines: Vec<String>,
}

pub(super) struct Fixture {
    pub(super) name: String,
    pub(super) input: String,
    pub(super) expected: ExpectedDom,
}

pub(super) fn load_fixtures() -> Vec<Fixture> {
    let root = fixture_root();
    let mut fixtures = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read fixture root {root:?}: {err}"))
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name != name.trim() {
            panic!("fixture directory has leading/trailing whitespace: '{name}'");
        }
        if name.starts_with('.') {
            continue;
        }
        let input_path = path.join("input.html");
        let dom_path = path.join("dom.txt");
        let input = fs::read_to_string(&input_path)
            .unwrap_or_else(|err| panic!("failed to read input {input_path:?}: {err}"));
        let input = normalize_fixture_input(input);
        let expected = parse_dom_file(&dom_path);
        fixtures.push(Fixture {
            name,
            input,
            expected,
        });
    }

    fixtures
}

pub(super) fn normalize_fixture_input(mut input: String) -> String {
    // Fixture files are text files and commonly end with a formatting newline.
    // Strip one terminal line ending so DOM expectations are not editor-dependent.
    if input.ends_with("\r\n") {
        input.truncate(input.len() - 2);
    } else if input.ends_with('\n') {
        input.pop();
    }
    input
}

pub(super) fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tree_builder")
}

pub(super) fn fixture_dir(name: &str) -> PathBuf {
    fixture_root().join(name)
}

fn parse_dom_file(path: &Path) -> ExpectedDom {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read dom file {path:?}: {err}"));
    let mut lines = Vec::new();
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
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
                if !matches!(
                    key.as_str(),
                    "format" | "status" | "reason" | "ignore_ids" | "ignore_empty_style"
                ) {
                    panic!("unknown header '{key}' in {path:?}");
                }
                if headers.insert(key.clone(), value).is_some() {
                    panic!("duplicate header '{key}' in {path:?}");
                }
            } else {
                lines.push(line.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }

    let format = headers
        .get("format")
        .unwrap_or_else(|| panic!("missing format header in {path:?}"));
    assert_eq!(format, "html5-dom-v1", "unsupported format in {path:?}");

    let status = match headers.get("status").map(|s| s.as_str()) {
        Some("active") | None => FixtureStatus::Active,
        Some("xfail") => FixtureStatus::Xfail,
        Some(other) => panic!("unsupported status '{other}' in {path:?}"),
    };
    let reason = headers.get("reason").cloned();
    if status == FixtureStatus::Xfail && reason.as_deref().unwrap_or("").is_empty() {
        panic!("xfail fixture missing reason in {path:?}");
    }

    let options = DomSnapshotOptions {
        ignore_ids: header_bool(&headers, "ignore_ids", true, path),
        ignore_empty_style: header_bool(&headers, "ignore_empty_style", true, path),
    };

    if lines.is_empty() {
        panic!("dom file {path:?} has no snapshot lines");
    }
    if !lines[0].starts_with("#document") {
        panic!("dom file {path:?} must start with #document");
    }
    for snapshot_line in &lines[1..] {
        if snapshot_line.starts_with('#') {
            panic!(
                "invalid snapshot line starting with '#' for format=html5-dom-v1: {snapshot_line:?} in {path:?}"
            );
        }
    }

    ExpectedDom {
        status,
        reason,
        options,
        lines,
    }
}

fn header_bool(headers: &BTreeMap<String, String>, key: &str, default: bool, path: &Path) -> bool {
    match headers.get(key).map(|s| s.as_str()) {
        None => default,
        Some("true") => true,
        Some("false") => false,
        Some(other) => panic!("invalid boolean '{other}' for {key} in {path:?}"),
    }
}
