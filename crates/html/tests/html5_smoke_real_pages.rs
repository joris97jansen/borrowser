#![cfg(all(feature = "html5", feature = "dom-snapshot"))]

use html::dom_snapshot::DomSnapshotOptions;
use html_test_support::diff_lines;
use html_test_support::wpt_expected::parse_expected_dom;
use html_test_support::wpt_tree_builder::run_tree_builder_whole;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture {
    name: String,
    input: String,
    expected_path: PathBuf,
}

const TABLE_HEAVY_FIXTURE_NAMES: &[&str] = &[
    "21-email-newsletter-table-layout",
    "22-wikipedia-like-infobox",
    "23-legacy-table-fragment",
    "24-report-nested-tables",
];

#[test]
fn html5_smoke_real_pages_minimum_fixture_count() {
    let fixtures = load_fixtures(update_mode());
    assert!(
        fixtures.len() >= 24,
        "smoke corpus too small: expected >=24 fixtures, got {}",
        fixtures.len()
    );
}

#[test]
fn html5_smoke_real_pages_table_heavy_band_is_present() {
    let fixtures = load_fixtures(update_mode());
    for name in TABLE_HEAVY_FIXTURE_NAMES {
        assert!(
            fixtures.iter().any(|fixture| fixture.name == *name),
            "missing table-heavy smoke fixture '{name}'"
        );
    }
}

#[test]
fn html5_smoke_real_pages() {
    let update = update_mode();
    let fixtures = load_fixtures(update);
    let filter = fixture_filter();
    if update && filter.is_some() {
        eprintln!(
            "html5_smoke_real_pages: update mode with BORROWSER_HTML5_SMOKE_FILTER set; only a subset of snapshots will be regenerated"
        );
    }
    let mut ran = 0usize;

    for fixture in fixtures {
        if let Some(filter) = filter.as_deref()
            && !fixture.name.to_ascii_lowercase().contains(filter)
        {
            continue;
        }
        ran += 1;
        assert_fixture_matches(&fixture, update);
    }

    assert!(ran > 0, "no smoke fixtures matched filter");
}

#[test]
fn html5_smoke_real_pages_table_heavy_band() {
    let update = update_mode();
    let fixtures = load_fixtures(update);

    for name in TABLE_HEAVY_FIXTURE_NAMES {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == *name)
            .unwrap_or_else(|| panic!("missing table-heavy smoke fixture '{name}'"));
        assert_fixture_matches(fixture, update);
    }
}

fn load_fixtures(update: bool) -> Vec<Fixture> {
    let root = fixture_root();
    let mut fixtures = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read smoke fixture root {root:?}: {err}"))
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
            panic!("smoke fixture dir has leading/trailing whitespace: '{name}'");
        }
        if name.starts_with('.') {
            continue;
        }

        let input_path = path.join("input.html");
        let expected_path = path.join("dom.txt");
        let input = fs::read_to_string(&input_path)
            .unwrap_or_else(|err| panic!("failed to read smoke input {input_path:?}: {err}"));
        let input = normalize_fixture_input(input);
        if !update && !expected_path.is_file() {
            panic!(
                "missing smoke expected snapshot for '{}': {:?}",
                name, expected_path
            );
        }
        fixtures.push(Fixture {
            name,
            input,
            expected_path,
        });
    }

    fixtures
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("smoke_real_pages")
}

fn normalize_fixture_input(mut input: String) -> String {
    if input.ends_with("\r\n") {
        input.truncate(input.len() - 2);
    } else if input.ends_with('\n') {
        input.pop();
    }
    input
}

fn fixture_filter() -> Option<String> {
    env::var("BORROWSER_HTML5_SMOKE_FILTER")
        .ok()
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_ascii_lowercase())
            }
        })
}

fn update_mode() -> bool {
    matches!(env::var("BORROWSER_HTML5_SMOKE_UPDATE").as_deref(), Ok("1"))
}

fn assert_fixture_matches(fixture: &Fixture, update: bool) {
    let actual =
        run_tree_builder_whole(&fixture.input, &fixture.name, DomSnapshotOptions::default())
            .unwrap_or_else(|err| {
                panic!(
                    "failed to parse smoke fixture '{}' ({:?}): {err}",
                    fixture.name, fixture.expected_path
                )
            });
    if update {
        write_expected_dom(&fixture.expected_path, &actual);
        return;
    }
    let expected = parse_expected_dom(&fixture.expected_path);
    if actual.as_slice() != expected.lines.as_slice() {
        panic!(
            "smoke DOM mismatch in fixture '{}'\npath: {}\n{}",
            fixture.name,
            fixture.expected_path.display(),
            diff_lines(&expected.lines, &actual)
        );
    }
}

fn write_expected_dom(path: &Path, lines: &[String]) {
    let mut out = String::new();
    out.push_str("# format: html5-dom-v1\n\n");
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    fs::write(path, out)
        .unwrap_or_else(|err| panic!("failed to write smoke snapshot {path:?}: {err}"));
}

#[test]
fn smoke_fixture_input_normalization_strips_single_terminal_lf() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\n".to_string()),
        "<div>ok</div>"
    );
}

#[test]
fn smoke_fixture_input_normalization_strips_single_terminal_crlf() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\r\n".to_string()),
        "<div>ok</div>"
    );
}

#[test]
fn smoke_fixture_input_normalization_strips_exactly_one_terminal_line_ending() {
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\n\n".to_string()),
        "<div>ok</div>\n"
    );
    assert_eq!(
        normalize_fixture_input("<div>ok</div>\r\n\r\n".to_string()),
        "<div>ok</div>\r\n"
    );
}
