use html_test_support::diff_lines;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixtureStatus {
    Active,
    Xfail,
    Skip,
}

pub(crate) struct ExpectedTokens {
    pub(crate) status: FixtureStatus,
    pub(crate) reason: Option<String>,
    pub(crate) lines: Vec<String>,
}

pub(crate) struct Fixture {
    pub(crate) name: String,
    pub(crate) input: String,
    pub(crate) expected: ExpectedTokens,
}

pub(crate) struct FixtureFilter {
    raw: Option<String>,
}

impl FixtureFilter {
    pub(crate) fn matches(&self, name: &str) -> bool {
        let Some(filter) = &self.raw else {
            return true;
        };
        name.contains(filter)
    }
}

pub(crate) fn fixture_filter() -> FixtureFilter {
    FixtureFilter {
        raw: env::var("BORROWSER_HTML5_TOKEN_FIXTURE").ok(),
    }
}

pub(crate) fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

pub(crate) fn load_fixtures() -> Vec<Fixture> {
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
        let tokens_path = path.join("tokens.txt");
        let input = fs::read_to_string(&input_path)
            .unwrap_or_else(|err| panic!("failed to read input {input_path:?}: {err}"));
        let expected = parse_tokens_file(&tokens_path);
        fixtures.push(Fixture {
            name,
            input,
            expected,
        });
    }

    fixtures
}

pub(crate) fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tokenizer")
}

pub(crate) fn fixture_dir(name: &str) -> PathBuf {
    fixture_root().join(name)
}

fn parse_tokens_file(path: &Path) -> ExpectedTokens {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read tokens file {path:?}: {err}"));
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
            let (key, value) = header
                .split_once(':')
                .unwrap_or_else(|| panic!("invalid header in {path:?}: '{line}'"));
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if headers.insert(key.clone(), value).is_some() {
                panic!("duplicate header '{key}' in {path:?}");
            }
        } else {
            lines.push(line.to_string());
        }
    }

    let format = headers
        .get("format")
        .unwrap_or_else(|| panic!("missing format header in {path:?}"));
    assert_eq!(format, "html5-token-v1", "unsupported format in {path:?}");

    let status = match headers.get("status").map(|s| s.as_str()) {
        Some("active") | None => FixtureStatus::Active,
        Some("xfail") => FixtureStatus::Xfail,
        Some("skip") => FixtureStatus::Skip,
        Some(other) => panic!("unsupported status '{other}' in {path:?}"),
    };
    let reason = headers.get("reason").cloned();
    if matches!(status, FixtureStatus::Xfail | FixtureStatus::Skip)
        && reason.as_deref().unwrap_or("").is_empty()
    {
        panic!("non-active fixture missing reason in {path:?}");
    }
    if lines.is_empty() {
        panic!("tokens file {path:?} has no token lines");
    }
    if lines.last().map(String::as_str) != Some("EOF") {
        panic!("tokens file {path:?} must end with EOF");
    }

    ExpectedTokens {
        status,
        reason,
        lines,
    }
}

#[allow(dead_code)]
fn _keep_diff_lines_import(lines_a: &[String], lines_b: &[String]) -> String {
    diff_lines(lines_a, lines_b)
}
