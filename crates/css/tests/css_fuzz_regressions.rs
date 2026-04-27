#![cfg(feature = "css-fuzzing")]

use css::fuzz_regressions::{
    CssFuzzRegressionProfile, CssFuzzRegressionTool,
    render_css_fuzz_regression_summary_with_profile,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const META_FORMAT_V1: &str = "css-fuzz-regression-v1";

#[test]
fn css_fuzz_regression_fixtures_match_committed_summaries() {
    let fixtures = load_fixtures();
    assert!(
        !fixtures.is_empty(),
        "expected at least one css fuzz regression fixture"
    );

    let mut seen_tools = BTreeSet::new();
    for fixture in fixtures {
        seen_tools.insert(fixture.tool.as_str().to_string());
        let bytes = fs::read(&fixture.input_path).unwrap_or_else(|err| {
            panic!(
                "failed to read regression input {}: {err}",
                fixture.input_path.display()
            )
        });
        let rendered = render_css_fuzz_regression_summary_with_profile(
            fixture.tool,
            fixture.profile,
            &bytes,
            fixture.seed,
        )
        .unwrap_or_else(|err| {
            panic!(
                "failed to render css fuzz regression summary for fixture {}: {err}",
                fixture.dir.display()
            )
        });
        let expected = fs::read_to_string(&fixture.summary_path).unwrap_or_else(|err| {
            panic!(
                "failed to read regression summary {}: {err}",
                fixture.summary_path.display()
            )
        });
        assert_eq!(
            rendered.trim_end(),
            expected.trim_end(),
            "css fuzz regression fixture {} drifted; regenerate {} from {}",
            fixture.dir.display(),
            fixture.summary_path.display(),
            fixture.input_path.display()
        );
    }

    let expected_tools = BTreeSet::from([
        "css_tokenizer".to_string(),
        "css_parser".to_string(),
        "css_selector_parser".to_string(),
        "css_selector_matching".to_string(),
        "css_cascade".to_string(),
        "css_values".to_string(),
    ]);
    assert_eq!(
        seen_tools, expected_tools,
        "promoted css fuzz regressions should cover every css fuzz tool"
    );
}

struct Fixture {
    dir: PathBuf,
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    seed: u64,
    input_path: PathBuf,
    summary_path: PathBuf,
}

fn load_fixtures() -> Vec<Fixture> {
    let root = fixture_root();
    let mut entries = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read fixture root {}: {err}", root.display()))
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    let mut fixtures = Vec::new();
    for entry in entries {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        assert_valid_fixture_dir_name(&dir);
        let meta_path = dir.join("meta.txt");
        let input_path = dir.join("input.bin");
        let summary_path = dir.join("summary.txt");
        let meta = parse_meta_file(&meta_path);
        assert!(
            input_path.is_file(),
            "css fuzz regression fixture {} is missing input.bin",
            dir.display()
        );
        assert!(
            summary_path.is_file(),
            "css fuzz regression fixture {} is missing summary.txt",
            dir.display()
        );
        if let Some(source) = &meta.source {
            let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../")
                .join(source);
            assert!(
                source_path.is_file(),
                "css fuzz regression fixture {} references missing source {}",
                dir.display(),
                source_path.display()
            );
        }
        fixtures.push(Fixture {
            dir,
            tool: meta.tool,
            profile: meta.profile,
            seed: meta.seed,
            input_path,
            summary_path,
        });
    }
    fixtures
}

struct FixtureMeta {
    tool: CssFuzzRegressionTool,
    profile: CssFuzzRegressionProfile,
    seed: u64,
    source: Option<String>,
}

fn parse_meta_file(path: &Path) -> FixtureMeta {
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
        "css fuzz regression metadata {} must declare # format: {}",
        path.display(),
        META_FORMAT_V1
    );
    let tool = CssFuzzRegressionTool::parse(&required_header(&headers, "tool", path))
        .unwrap_or_else(|| panic!("unsupported css fuzz regression tool in {}", path.display()));
    let profile = CssFuzzRegressionProfile::parse(&required_header(&headers, "profile", path))
        .unwrap_or_else(|| {
            panic!(
                "unsupported css fuzz regression profile in {}",
                path.display()
            )
        });
    let seed = required_header(&headers, "seed", path)
        .parse::<u64>()
        .unwrap_or_else(|err| panic!("invalid numeric seed in {}: {err}", path.display()));
    let date = required_header(&headers, "date", path);
    assert_valid_iso_date(&date, path);
    let issue = required_header(&headers, "issue", path);
    assert_valid_issue_reference(&issue, path);
    let guard = required_header(&headers, "guard", path);
    assert!(
        !guard.trim().is_empty(),
        "metadata header 'guard' in {} must be non-empty",
        path.display()
    );

    FixtureMeta {
        tool,
        profile,
        seed,
        source: headers.get("source").cloned(),
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("regressions")
        .join("css_fuzz")
}

fn assert_valid_fixture_dir_name(path: &Path) {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_else(|| panic!("invalid fixture directory name {}", path.display()));
    assert!(
        name.starts_with("rg-"),
        "css fuzz regression fixture directory {} must start with 'rg-'",
        path.display()
    );

    let parts = name.split('-').collect::<Vec<_>>();
    assert!(
        parts.len() >= 6,
        "css fuzz regression fixture directory {} must match rg-<tool>-<slug>-YYYY-MM-DD",
        path.display()
    );
    assert_eq!(
        parts[0],
        "rg",
        "css fuzz regression fixture directory {} must match rg-<tool>-<slug>-YYYY-MM-DD",
        path.display()
    );
    assert!(
        parts[1..parts.len() - 3].iter().all(|part| !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())),
        "css fuzz regression fixture directory {} must use lowercase ASCII kebab-case segments before the date",
        path.display()
    );

    let date = parts[parts.len() - 3..].join("-");
    assert_valid_iso_date(&date, path);
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
        "metadata header 'issue' in {} must be an issue URL or a stable issue id (accepted forms: https://..., #1234, BOR-123, Milestone-T/T6), got '{}'",
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
