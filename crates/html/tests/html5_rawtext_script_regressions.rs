#![cfg(feature = "html5")]

#[cfg(feature = "dom-snapshot")]
use html::dom_snapshot::DomSnapshotOptions;
use html::html5::{
    DocumentParseContext, Html5Tokenizer, Input, TextModeSpec, Token, TokenFmt, TokenizeResult,
    TokenizerConfig, TokenizerControl,
};
use html::test_harness::ChunkPlan;
use html_test_support::diff_lines;
use html_test_support::wpt_expected::parse_expected_tokens;
#[cfg(feature = "dom-snapshot")]
use html_test_support::wpt_expected::{ParsedExpectedDom, parse_expected_dom};
#[cfg(feature = "dom-snapshot")]
use html_test_support::wpt_tree_builder::{run_tree_builder_chunked, run_tree_builder_whole};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const MIN_REGRESSION_FIXTURES: usize = 1;
const MAX_PUMP_ITERATIONS_BASE: usize = 1024;
const META_FORMAT_V1: &str = "html5-rawtext-script-regression-v1";

#[test]
fn html5_rawtext_script_regression_fixture_contract() {
    let fixtures = load_fixtures();
    assert!(
        fixtures.len() >= MIN_REGRESSION_FIXTURES,
        "rawtext/script regression corpus too small: found {} fixtures, require at least {}",
        fixtures.len(),
        MIN_REGRESSION_FIXTURES
    );

    for fixture in fixtures {
        assert!(
            fixture.name.starts_with("rs-"),
            "fixture '{}' must use the rs-<mode>-<slug>-<date> naming convention",
            fixture.name
        );
        assert!(
            fixture
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "fixture '{}' must be lowercase ASCII kebab-case",
            fixture.name
        );
        assert!(
            fixture.expected_tokens.is_some() || fixture.expected_dom.is_some(),
            "fixture '{}' must declare at least one stable oracle (tokens.txt or dom.txt)",
            fixture.name
        );
        assert!(
            !fixture.meta.guard.trim().is_empty(),
            "fixture '{}' must explain what it guards",
            fixture.name
        );
    }
}

#[test]
fn html5_rawtext_script_regressions_tokens_whole_input() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_tokens) = fixture.expected_tokens.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let actual = run_token_fixture_whole(fixture);
        assert_expected_lines(fixture, expected_tokens, &actual, "tokens whole");
    }
    assert!(
        ran > 0,
        "no rawtext/script token regressions matched filter"
    );
}

#[test]
fn html5_rawtext_script_regressions_tokens_every_boundary_chunked() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_tokens) = fixture.expected_tokens.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let whole = run_token_fixture_whole(fixture);
        let chunked = run_token_fixture_every_boundary(fixture);
        if chunked != whole {
            panic!(
                "chunked token output diverged from whole input in rawtext/script regression '{}' [every-boundary]\npath: {}\nguard: {}\n{}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard,
                diff_lines(&whole, &chunked)
            );
        }
        assert_expected_lines(fixture, expected_tokens, &chunked, "tokens every-boundary");
    }
    assert!(
        ran > 0,
        "no rawtext/script token regressions matched filter"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn html5_rawtext_script_regressions_dom_whole_input() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_dom) = fixture.expected_dom.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let actual = run_dom_fixture_whole(fixture);
        assert_expected_lines(fixture, &expected_dom.lines, &actual, "dom whole");
    }
    assert!(ran > 0, "no rawtext/script DOM regressions matched filter");
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn html5_rawtext_script_regressions_dom_every_boundary_chunked() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures
        .iter()
        .filter(|fixture| filter.matches(&fixture.name))
    {
        let Some(expected_dom) = fixture.expected_dom.as_ref() else {
            continue;
        };
        ran = ran.saturating_add(1);
        let whole = run_dom_fixture_whole(fixture);
        let chunked = run_dom_fixture_every_boundary(fixture);
        if chunked != whole {
            panic!(
                "chunked DOM output diverged from whole input in rawtext/script regression '{}' [every-boundary]\npath: {}\nguard: {}\n{}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard,
                diff_lines(&whole, &chunked)
            );
        }
        assert_expected_lines(fixture, &expected_dom.lines, &chunked, "dom every-boundary");
    }
    assert!(ran > 0, "no rawtext/script DOM regressions matched filter");
}

struct Fixture {
    name: String,
    dir: PathBuf,
    input: String,
    meta: FixtureMeta,
    expected_tokens: Option<Vec<String>>,
    #[cfg(feature = "dom-snapshot")]
    expected_dom: Option<ParsedExpectedDom>,
    #[cfg(not(feature = "dom-snapshot"))]
    expected_dom: Option<()>,
}

#[derive(Clone, Debug)]
struct FixtureMeta {
    tool: String,
    seed: String,
    date: String,
    issue: String,
    guard: String,
    source: Option<String>,
    mode: TextModeFixtureMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextModeFixtureMode {
    ScriptData,
    RawtextStyle,
}

impl TextModeFixtureMode {
    fn from_header(value: &str, path: &Path) -> Self {
        match value {
            "script-data" => Self::ScriptData,
            "rawtext-style" => Self::RawtextStyle,
            other => panic!(
                "unsupported text-mode regression mode '{other}' in {}",
                path.display()
            ),
        }
    }

    fn tag_name(self) -> &'static str {
        match self {
            Self::ScriptData => "script",
            Self::RawtextStyle => "style",
        }
    }

    fn spec(self, tag: html::html5::AtomId) -> TextModeSpec {
        match self {
            Self::ScriptData => TextModeSpec::script_data(tag),
            Self::RawtextStyle => TextModeSpec::rawtext_style(tag),
        }
    }
}

struct FixtureFilter {
    raw: Option<String>,
}

impl FixtureFilter {
    fn matches(&self, name: &str) -> bool {
        let Some(filter) = &self.raw else {
            return true;
        };
        name.contains(filter)
    }
}

fn fixture_filter() -> FixtureFilter {
    FixtureFilter {
        raw: env::var("BORROWSER_HTML5_RAWTEXT_SCRIPT_REGRESSION").ok(),
    }
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
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta_path = dir.join("meta.txt");
        let input_path = dir.join("input.html");
        let tokens_path = dir.join("tokens.txt");
        let dom_path = dir.join("dom.txt");
        let input =
            trim_one_trailing_line_ending(&fs::read_to_string(&input_path).unwrap_or_else(|err| {
                panic!("failed to read input {}: {err}", input_path.display())
            }));
        let meta = parse_meta_file(&meta_path);
        let expected_tokens = tokens_path
            .exists()
            .then(|| parse_expected_tokens(&tokens_path));
        #[cfg(feature = "dom-snapshot")]
        let expected_dom = dom_path.exists().then(|| parse_expected_dom(&dom_path));
        #[cfg(not(feature = "dom-snapshot"))]
        let expected_dom = dom_path.exists().then_some(());
        fixtures.push(Fixture {
            name,
            dir,
            input,
            meta,
            expected_tokens,
            expected_dom,
        });
    }
    fixtures
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("regressions")
        .join("html5")
        .join("rawtext_script")
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

fn trim_one_trailing_line_ending(input: &str) -> String {
    if let Some(stripped) = input.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = input.strip_suffix('\n') {
        stripped.to_string()
    } else {
        input.to_string()
    }
}

fn run_token_fixture_whole(fixture: &Fixture) -> Vec<String> {
    run_token_fixture_with_chunks(fixture, std::iter::once(fixture.input.as_str()), false)
}

fn run_token_fixture_every_boundary(fixture: &Fixture) -> Vec<String> {
    let chunks = utf8_boundary_chunks(&fixture.input);
    run_token_fixture_with_chunks(fixture, chunks, true)
}

fn utf8_boundary_chunks(input: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    for (idx, _) in input.char_indices().skip(1) {
        chunks.push(&input[start..idx]);
        start = idx;
    }
    if start < input.len() {
        chunks.push(&input[start..]);
    } else if chunks.is_empty() {
        chunks.push(input);
    }
    chunks
}

fn run_token_fixture_with_chunks<'a, I>(
    fixture: &Fixture,
    chunks: I,
    expect_token_granular_batches: bool,
) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut ctx = DocumentParseContext::new();
    let host = ctx
        .atoms
        .intern_ascii_folded(fixture.meta.mode.tag_name())
        .unwrap_or_else(|err| panic!("failed to intern host atom for '{}': {err:?}", fixture.name));
    let mut tokenizer = Html5Tokenizer::new(
        TokenizerConfig {
            emit_eof: true,
            ..TokenizerConfig::default()
        },
        &mut ctx,
    );
    let mut buffer = Input::new();
    let mut out = Vec::new();
    let mut text_mode_active = false;

    for chunk in chunks {
        buffer.push_str(chunk);
        pump_until_blocked(
            fixture,
            &mut tokenizer,
            &mut buffer,
            &mut ctx,
            &mut out,
            &mut text_mode_active,
            host,
            expect_token_granular_batches,
            "chunk",
        );
    }

    handle_tokenize_result(
        tokenizer.finish(&buffer),
        fixture,
        if expect_token_granular_batches {
            "every-boundary"
        } else {
            "whole"
        },
        "finish",
    );
    let _ = drain_tokens(
        fixture,
        &mut tokenizer,
        &mut buffer,
        &ctx,
        &mut out,
        &mut text_mode_active,
        host,
        false,
        if expect_token_granular_batches {
            "every-boundary"
        } else {
            "whole"
        },
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn pump_until_blocked(
    fixture: &Fixture,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &mut DocumentParseContext,
    out: &mut Vec<String>,
    text_mode_active: &mut bool,
    host: html::html5::AtomId,
    expect_token_granular_batches: bool,
    label: &str,
) {
    let mut iterations = 0usize;
    let mut stalled_progress_pumps = 0usize;
    let max_iterations = buffer
        .as_str()
        .len()
        .saturating_add(MAX_PUMP_ITERATIONS_BASE);

    loop {
        iterations = iterations.saturating_add(1);
        assert!(
            iterations <= max_iterations,
            "tokenizer pumping exceeded iteration budget in rawtext/script regression '{}' [{label}]\npath: {}\nguard: {}",
            fixture.name,
            fixture.dir.display(),
            fixture.meta.guard
        );
        let stats_before = tokenizer.stats();
        let out_len_before = out.len();
        let result = tokenizer.push_input_until_token(buffer, ctx);
        handle_tokenize_result(result, fixture, label, "push_input");
        let drained = drain_tokens(
            fixture,
            tokenizer,
            buffer,
            ctx,
            out,
            text_mode_active,
            host,
            expect_token_granular_batches,
            label,
        );
        let stats_after = tokenizer.stats();
        let consumed = stats_after.bytes_consumed != stats_before.bytes_consumed;
        let emitted = out.len() != out_len_before;
        if result == TokenizeResult::Progress {
            if consumed || emitted {
                stalled_progress_pumps = 0;
            } else {
                stalled_progress_pumps = stalled_progress_pumps.saturating_add(1);
                assert!(
                    stalled_progress_pumps <= 8,
                    "tokenizer repeatedly reported Progress without observable progress in rawtext/script regression '{}' [{label}]\npath: {}\nguard: {}",
                    fixture.name,
                    fixture.dir.display(),
                    fixture.meta.guard
                );
            }
        }
        if result == TokenizeResult::NeedMoreInput {
            if consumed || emitted {
                stalled_progress_pumps = 0;
                continue;
            }
            let buffer_len = buffer.as_str().len() as u64;
            if stats_after.bytes_consumed >= buffer_len && !drained {
                break;
            }
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drain_tokens(
    fixture: &Fixture,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &DocumentParseContext,
    out: &mut Vec<String>,
    text_mode_active: &mut bool,
    host: html::html5::AtomId,
    expect_token_granular_batches: bool,
    label: &str,
) -> bool {
    let mut saw_any = false;
    loop {
        let batch = tokenizer.next_batch(buffer);
        if batch.tokens().is_empty() {
            break;
        }
        saw_any = true;
        if expect_token_granular_batches {
            assert_eq!(
                batch.tokens().len(),
                1,
                "rawtext/script regression harness must observe exactly one token per pump in '{}' [{label}]\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
        let pending_control = {
            let resolver = batch.resolver();
            let fmt = TokenFmt::new(&ctx.atoms, &resolver);
            for token in batch.iter() {
                out.push(fmt.format_token(token).expect(
                    "token formatting in rawtext/script regression harness must be deterministic",
                ));
            }
            batch.tokens().first().and_then(|token| match token {
                Token::StartTag { name, .. } if *name == host && !*text_mode_active => {
                    *text_mode_active = true;
                    Some(TokenizerControl::EnterTextMode(
                        fixture.meta.mode.spec(host),
                    ))
                }
                Token::EndTag { name } if *name == host && *text_mode_active => {
                    *text_mode_active = false;
                    Some(TokenizerControl::ExitTextMode)
                }
                _ => None,
            })
        };
        if let Some(control) = pending_control {
            tokenizer.apply_control(control);
        }
    }
    saw_any
}

fn handle_tokenize_result(result: TokenizeResult, fixture: &Fixture, label: &str, stage: &str) {
    match (stage, result) {
        ("push_input", TokenizeResult::NeedMoreInput | TokenizeResult::Progress) => {}
        ("finish", TokenizeResult::EmittedEof) => {}
        ("push_input", TokenizeResult::EmittedEof) => {
            panic!(
                "unexpected EOF while pushing input in rawtext/script regression '{}' [{label}]\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
        ("finish", other) => {
            panic!(
                "finish must emit EOF in rawtext/script regression '{}' [{label}], got {other:?}\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
        _ => {
            panic!(
                "unexpected tokenizer state in rawtext/script regression '{}' [{label}] stage={stage} result={result:?}\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
    }
}

#[cfg(feature = "dom-snapshot")]
fn run_dom_fixture_whole(fixture: &Fixture) -> Vec<String> {
    let options = dom_options(fixture);
    run_tree_builder_whole(&fixture.input, &fixture.name, options).unwrap_or_else(|err| {
        panic!(
            "tree-builder whole-input run failed for rawtext/script regression '{}' [whole]\npath: {}\nguard: {}\n{err}",
            fixture.name,
            fixture.dir.display(),
            fixture.meta.guard
        )
    })
}

#[cfg(feature = "dom-snapshot")]
fn run_dom_fixture_every_boundary(fixture: &Fixture) -> Vec<String> {
    let options = dom_options(fixture);
    let plan = ChunkPlan::boundaries(
        fixture
            .input
            .char_indices()
            .skip(1)
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>(),
    );
    run_tree_builder_chunked(&fixture.input, &fixture.name, &plan, "every-boundary", options)
        .unwrap_or_else(|err| {
            panic!(
                "tree-builder every-boundary run failed for rawtext/script regression '{}'\npath: {}\nguard: {}\n{err}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            )
        })
}

#[cfg(feature = "dom-snapshot")]
fn dom_options(fixture: &Fixture) -> DomSnapshotOptions {
    let expected = fixture
        .expected_dom
        .as_ref()
        .expect("dom_options requires dom expectation");
    DomSnapshotOptions {
        ignore_ids: expected.ignore_ids,
        ignore_empty_style: expected.ignore_empty_style,
    }
}

fn assert_expected_lines(fixture: &Fixture, expected: &[String], actual: &[String], label: &str) {
    if actual != expected {
        let source = fixture.meta.source.as_deref().unwrap_or("<none>");
        panic!(
            "snapshot mismatch in rawtext/script regression '{}' [{label}]\npath: {}\ntool: {}\nseed: {}\ndate: {}\nissue: {}\nsource: {}\nguard: {}\n{}",
            fixture.name,
            fixture.dir.display(),
            fixture.meta.tool,
            fixture.meta.seed,
            fixture.meta.date,
            fixture.meta.issue,
            source,
            fixture.meta.guard,
            diff_lines(expected, actual)
        );
    }
}
