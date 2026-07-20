use html::dom_snapshot::DomSnapshotOptions;
use std::collections::BTreeMap;
use std::env;
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
    pub(super) include_parse_errors: bool,
    pub(super) lines: Vec<String>,
}

pub(super) struct Fixture {
    pub(super) name: String,
    pub(super) input: String,
    pub(super) expected: ExpectedDom,
}

pub(super) fn load_fixtures() -> Vec<Fixture> {
    let mut fixtures = Vec::new();
    let mut seen_names = BTreeMap::new();

    for root in fixture_roots() {
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
            if let Some(previous_root) = seen_names.insert(name.clone(), root.clone()) {
                panic!("duplicate DOM fixture name '{name}' in {previous_root:?} and {root:?}");
            }
            let input_path = path.join("input.html");
            let dom_path = path.join("dom.txt");
            if name.starts_with("ae9b-") {
                validate_ae9b_local_provenance(&path, &name);
            }
            if name.starts_with("ae10-") {
                validate_ae10_local_provenance(&path, &name);
            }
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
    }

    fixtures.sort_by(|left, right| left.name.cmp(&right.name));
    fixtures
}

pub(super) fn update_mode() -> bool {
    matches!(
        env::var("BORROWSER_HTML5_DOM_FIXTURE_UPDATE").as_deref(),
        Ok("1")
    )
}

pub(super) fn write_expected_dom_file(fixture: &Fixture, lines: &[String]) {
    let path = fixture_dir(&fixture.name).join("dom.txt");
    let mut out = String::from("# format: html5-dom-v2\n");
    out.push_str("# status: active\n");
    if !fixture.expected.options.ignore_ids {
        out.push_str("# ignore_ids: false\n");
    }
    if !fixture.expected.options.ignore_empty_style {
        out.push_str("# ignore_empty_style: false\n");
    }
    if fixture.expected.include_parse_errors {
        out.push_str("# include_parse_errors: true\n");
    }
    out.push('\n');
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    fs::write(&path, out)
        .unwrap_or_else(|err| panic!("failed to write expected DOM {path:?}: {err}"));
}

fn validate_ae10_local_provenance(path: &Path, name: &str) {
    let provenance_path = path.join("provenance.txt");
    let provenance = fs::read_to_string(&provenance_path).unwrap_or_else(|err| {
        panic!("AE10 local DOM fixture '{name}' missing provenance {provenance_path:?}: {err}")
    });
    assert!(
        provenance.contains("Local WHATWG-derived normative fixture; not a WPT import."),
        "AE10 local DOM fixture '{name}' is mislabeled"
    );
    assert!(provenance.contains("88ae68cb961651f0f92c5d2046049f53ecdfc6cf"));
}

fn validate_ae9b_local_provenance(path: &Path, name: &str) {
    let provenance_path = path.join("provenance.txt");
    let provenance = fs::read_to_string(&provenance_path).unwrap_or_else(|err| {
        panic!("AE9b local DOM fixture '{name}' missing provenance {provenance_path:?}: {err}")
    });
    assert!(
        provenance.contains(
            "Local WHATWG-derived fixture; not an upstream WPT or html5lib-tests import."
        ),
        "AE9b local DOM fixture '{name}' is mislabeled"
    );
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

fn legacy_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tree_builder")
}

fn tables_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tables")
        .join("dom")
}

fn fixture_roots() -> Vec<PathBuf> {
    vec![legacy_fixture_root(), tables_fixture_root()]
}

pub(super) fn fixture_dir(name: &str) -> PathBuf {
    let mut matches = fixture_roots()
        .into_iter()
        .map(|root| root.join(name))
        .filter(|path| path.is_dir());
    let Some(first) = matches.next() else {
        panic!("unknown DOM fixture directory for '{name}'");
    };
    if let Some(second) = matches.next() {
        panic!(
            "ambiguous DOM fixture directory for '{name}': {:?} and {:?}",
            first, second
        );
    }
    first
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
                    "format"
                        | "status"
                        | "reason"
                        | "ignore_ids"
                        | "ignore_empty_style"
                        | "include_parse_errors"
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
    assert_eq!(format, "html5-dom-v2", "unsupported format in {path:?}");

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
    let include_parse_errors = header_bool(&headers, "include_parse_errors", false, path);

    if lines.is_empty() {
        panic!("dom file {path:?} has no snapshot lines");
    }
    if lines.first().map(String::as_str) != Some("#dom-snapshot-v2")
        || !lines
            .get(1)
            .is_some_and(|line| line.starts_with("#document"))
    {
        panic!("dom file {path:?} must start with #dom-snapshot-v2 then #document");
    }
    for snapshot_line in &lines[2..] {
        if snapshot_line.starts_with('#') {
            panic!(
                "invalid snapshot line starting with '#' for format=html5-dom-v2: {snapshot_line:?} in {path:?}"
            );
        }
    }

    ExpectedDom {
        status,
        reason,
        options,
        include_parse_errors,
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
