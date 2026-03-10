use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixtureStatus {
    Active,
    Xfail,
}

pub(crate) struct ExpectedPatches {
    pub(crate) status: FixtureStatus,
    pub(crate) reason: Option<String>,
    pub(crate) lines: Vec<String>,
}

pub(crate) struct Fixture {
    pub(crate) name: String,
    pub(crate) input: String,
    pub(crate) expected: ExpectedPatches,
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
        raw: env::var("BORROWSER_HTML5_PATCH_FIXTURE").ok(),
    }
}

pub(crate) fn update_mode() -> bool {
    matches!(
        env::var("BORROWSER_HTML5_PATCH_FIXTURE_UPDATE").as_deref(),
        Ok("1")
    )
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
        let patches_path = path.join("patches.txt");
        let input = fs::read_to_string(&input_path)
            .unwrap_or_else(|err| panic!("failed to read input {input_path:?}: {err}"));
        let input = normalize_fixture_input(input);
        let expected = parse_patch_file(&patches_path);
        fixtures.push(Fixture {
            name,
            input,
            expected,
        });
    }

    fixtures
}

fn normalize_fixture_input(mut input: String) -> String {
    if input.ends_with("\r\n") {
        input.truncate(input.len() - 2);
    } else if input.ends_with('\n') {
        input.pop();
    }
    input
}

pub(crate) fn fixture_dir(name: &str) -> PathBuf {
    fixture_root().join(name)
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tree_builder_patches")
}

fn parse_patch_file(path: &Path) -> ExpectedPatches {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read patch file {path:?}: {err}"));
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
                if !matches!(key.as_str(), "format" | "status" | "reason") {
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
    assert_eq!(
        format, "html5-dompatch-v1",
        "unsupported format in {path:?}"
    );

    let status = match headers.get("status").map(|s| s.as_str()) {
        Some("active") | None => FixtureStatus::Active,
        Some("xfail") => FixtureStatus::Xfail,
        Some(other) => panic!("unsupported status '{other}' in {path:?}"),
    };
    let reason = headers.get("reason").cloned();
    if status == FixtureStatus::Xfail && reason.as_deref().unwrap_or("").is_empty() {
        panic!("xfail fixture missing reason in {path:?}");
    }

    if lines.is_empty() {
        panic!("patch file {path:?} has no patch lines");
    }

    ExpectedPatches {
        status,
        reason,
        lines,
    }
}

pub(crate) fn write_expected_patch_file(fixture: &Fixture, lines: &[String]) {
    let path = fixture_dir(&fixture.name).join("patches.txt");
    let mut out = String::new();
    out.push_str("# format: html5-dompatch-v1\n");
    out.push_str("# status: active\n\n");
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    fs::write(&path, out)
        .unwrap_or_else(|err| panic!("failed to write expected patches {path:?}: {err}"));
}

#[test]
fn patch_fixture_input_normalization_strips_single_terminal_lf() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\n".to_string()),
        "<div>ok</div>"
    );
}

#[test]
fn patch_fixture_input_normalization_strips_single_terminal_crlf() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\r\n".to_string()),
        "<div>ok</div>"
    );
}

#[test]
fn patch_fixture_input_normalization_strips_exactly_one_terminal_line_ending() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\n\n".to_string()),
        "<div>ok</div>\n"
    );
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\r\n\r\n".to_string()),
        "<div>ok</div>\r\n"
    );
}
