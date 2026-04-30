use std::{
    env, fs,
    path::{Path, PathBuf},
};

use css::{ParseOptions, compute_document_styles, parse_stylesheet_with_options};
use html::{HtmlParseOptions, parse_document};

struct PageFixture {
    name: String,
    input_html: String,
    author_css: String,
    expected_path: PathBuf,
    guard: String,
}

const REQUIRED_FIXTURES: &[&str] = &[
    "commerce-product",
    "dashboard-cards",
    "docs-article",
    "marketing-landing",
];

#[test]
fn representative_page_fixture_count_and_required_band() {
    let fixtures = load_fixtures(update_mode());
    assert!(
        fixtures.len() >= REQUIRED_FIXTURES.len(),
        "representative CSS page corpus too small: expected >= {}, got {}",
        REQUIRED_FIXTURES.len(),
        fixtures.len()
    );

    for required in REQUIRED_FIXTURES {
        assert!(
            fixtures.iter().any(|fixture| fixture.name == *required),
            "missing representative CSS page fixture '{required}'"
        );
    }
}

#[test]
fn representative_page_fixtures_have_guard_metadata() {
    for fixture in load_fixtures(update_mode()) {
        assert!(
            !fixture.guard.trim().is_empty(),
            "representative CSS page fixture '{}' must have a non-empty guard",
            fixture.name
        );
    }
}

#[test]
fn representative_pages_compute_deterministic_styles() {
    let update = update_mode();
    let fixtures = load_fixtures(update);
    let filter = fixture_filter();
    if update && filter.is_some() {
        eprintln!(
            "representative_pages: update mode with BORROWSER_CSS_REPRESENTATIVE_FILTER set; only a subset of snapshots will be regenerated"
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

    assert!(ran > 0, "no representative CSS fixtures matched filter");
}

fn assert_fixture_matches(fixture: &PageFixture, update: bool) {
    let actual = representative_computed_snapshot(fixture);
    if update {
        write_snapshot(&fixture.expected_path, &actual);
        return;
    }

    let expected = fs::read_to_string(&fixture.expected_path).unwrap_or_else(|err| {
        panic!(
            "failed to read representative CSS snapshot for '{}' at {}: {err}",
            fixture.name,
            fixture.expected_path.display()
        )
    });
    let expected = normalize_fixture_input(expected);
    assert_eq!(
        actual,
        expected,
        "representative CSS page fixture '{}' mismatch\npath: {}\nguard: {}\n{}",
        fixture.name,
        fixture.expected_path.display(),
        fixture.guard,
        first_difference(&actual, &expected)
    );
}

fn representative_computed_snapshot(fixture: &PageFixture) -> String {
    let document = parse_document(&fixture.input_html, HtmlParseOptions::default())
        .unwrap_or_else(|err| panic!("failed to parse HTML fixture '{}': {err}", fixture.name))
        .document;
    let stylesheet =
        parse_stylesheet_with_options(&fixture.author_css, &ParseOptions::stylesheet());
    assert!(
        stylesheet.diagnostics.is_empty(),
        "representative CSS fixture '{}' produced parse diagnostics: {:?}",
        fixture.name,
        stylesheet.diagnostics
    );

    let computed = compute_document_styles(&document, &[stylesheet])
        .unwrap_or_else(|err| panic!("failed to compute styles for '{}': {err}", fixture.name));

    let mut snapshot = String::new();
    snapshot.push_str("version: 1\n");
    snapshot.push_str("css-representative-page\n");
    snapshot.push_str(&format!("fixture: {}\n", fixture.name));
    snapshot.push_str(&format!("guard: {}\n", fixture.guard.trim()));
    snapshot.push_str(&format!("elements: {}\n", computed.entries().len()));
    snapshot.push('\n');
    snapshot.push_str(&computed.to_debug_snapshot());
    snapshot
}

fn load_fixtures(update: bool) -> Vec<PageFixture> {
    let root = fixture_root();
    let mut entries: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|err| {
            panic!("failed to read representative CSS fixture root {root:?}: {err}")
        })
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|entry| entry.file_name());

    let mut fixtures = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name != name.trim() {
            panic!("representative CSS fixture dir has leading/trailing whitespace: '{name}'");
        }
        if name.starts_with('.') {
            continue;
        }

        let input_path = path.join("input.html");
        let css_path = path.join("author.css");
        let meta_path = path.join("meta.txt");
        let expected_path = path.join("computed.snap");

        let input_html = read_normalized(&input_path);
        let author_css = read_normalized(&css_path);
        let guard = read_guard(&meta_path);
        if !update && !expected_path.is_file() {
            panic!(
                "missing representative CSS snapshot for '{}': {}",
                name,
                expected_path.display()
            );
        }

        fixtures.push(PageFixture {
            name,
            input_html,
            author_css,
            expected_path,
            guard,
        });
    }

    fixtures
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("representative_pages")
}

fn read_normalized(path: &Path) -> String {
    normalize_fixture_input(
        fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("failed to read fixture file {}: {err}", path.display())),
    )
}

fn normalize_fixture_input(input: String) -> String {
    let mut normalized = input.replace("\r\n", "\n").replace('\r', "\n");

    if normalized.ends_with('\n') {
        normalized.pop();
    }

    normalized
}

fn read_guard(path: &Path) -> String {
    let meta = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read fixture metadata {}: {err}", path.display()));

    for line in meta.lines() {
        if let Some(guard) = line.strip_prefix("# guard:") {
            return guard.trim().to_string();
        }
    }

    panic!(
        "fixture metadata {} must contain '# guard:'",
        path.display()
    );
}

fn fixture_filter() -> Option<String> {
    env::var("BORROWSER_CSS_REPRESENTATIVE_FILTER")
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
    matches!(
        env::var("BORROWSER_CSS_REPRESENTATIVE_UPDATE").as_deref(),
        Ok("1")
    )
}

fn write_snapshot(path: &Path, snapshot: &str) {
    fs::write(path, format!("{snapshot}\n"))
        .unwrap_or_else(|err| panic!("failed to write snapshot {}: {err}", path.display()));
}

fn first_difference(actual: &str, expected: &str) -> String {
    let actual_lines: Vec<_> = actual.lines().collect();
    let expected_lines: Vec<_> = expected.lines().collect();
    let max = actual_lines.len().max(expected_lines.len());

    for index in 0..max {
        let actual = actual_lines.get(index).copied().unwrap_or("<missing>");
        let expected = expected_lines.get(index).copied().unwrap_or("<missing>");
        if actual != expected {
            return format!(
                "first differing line {}:\nexpected: {expected}\nactual:   {actual}",
                index + 1
            );
        }
    }

    "no line difference found".to_string()
}

#[test]
fn representative_fixture_input_normalization_strips_single_terminal_lf() {
    assert_eq!(
        normalize_fixture_input("<main></main>\n".to_string()),
        "<main></main>"
    );
}

#[test]
fn representative_fixture_input_normalization_strips_single_terminal_crlf() {
    assert_eq!(
        normalize_fixture_input("<main></main>\r\n".to_string()),
        "<main></main>"
    );
}

#[test]
fn representative_fixture_input_normalization_normalizes_internal_crlf() {
    assert_eq!(
        normalize_fixture_input("line one\r\nline two\r\n".to_string()),
        "line one\nline two"
    );
}

#[test]
fn representative_fixture_input_normalization_normalizes_lone_cr() {
    assert_eq!(
        normalize_fixture_input("line one\rline two\r".to_string()),
        "line one\nline two"
    );
}
