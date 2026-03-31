use html::html5::{
    DocumentParseContext, Html5Tokenizer, Input, TextModeSpec, Token, TokenFmt, TokenizeResult,
    TokenizerConfig, TokenizerControl,
};
use html_test_support::diff_lines;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const MIN_SCRIPT_REGRESSION_FIXTURES: usize = 8;
const MAX_PUMP_ITERATIONS_BASE: usize = 1024;

#[test]
fn html5_script_regressions_fixture_contract() {
    let fixtures = load_fixtures();
    assert!(
        fixtures.len() >= MIN_SCRIPT_REGRESSION_FIXTURES,
        "script regression corpus too small: found {} fixtures, require at least {}",
        fixtures.len(),
        MIN_SCRIPT_REGRESSION_FIXTURES
    );
    for fixture in fixtures {
        assert!(
            !fixture.guard.trim().is_empty(),
            "fixture '{}' must explain what it guards",
            fixture.name
        );
    }
}

#[test]
fn html5_script_regressions_whole_input() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran = ran.saturating_add(1);
        let actual = run_script_fixture_whole(&fixture);
        assert_expected(&fixture, &actual, "whole");
    }
    assert!(ran > 0, "no script regression fixtures matched filter");
}

#[test]
fn html5_script_regressions_every_boundary_chunked() {
    let filter = fixture_filter();
    let fixtures = load_fixtures();
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran = ran.saturating_add(1);
        let whole = run_script_fixture_whole(&fixture);
        let chunked = run_script_fixture_every_boundary(&fixture);
        if chunked != whole {
            panic!(
                "chunked output diverged from whole input in script regression '{}' [every-boundary]\npath: {}\nguard: {}\n{}",
                fixture.name,
                fixture.dir.display(),
                fixture.guard,
                diff_lines(&whole, &chunked)
            );
        }
        assert_expected(&fixture, &chunked, "every-boundary");
    }
    assert!(ran > 0, "no script regression fixtures matched filter");
}

struct Fixture {
    name: String,
    dir: PathBuf,
    guard: String,
    input: String,
    expected: Vec<String>,
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
        raw: env::var("BORROWSER_HTML5_SCRIPT_REGRESSION_FIXTURE").ok(),
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
        let input_path = dir.join("input.html");
        let tokens_path = dir.join("tokens.txt");
        let input =
            trim_one_trailing_line_ending(&fs::read_to_string(&input_path).unwrap_or_else(|err| {
                panic!("failed to read input {}: {err}", input_path.display())
            }));
        let parsed = parse_tokens_file(&tokens_path);
        fixtures.push(Fixture {
            name,
            dir,
            guard: parsed.guard,
            input,
            expected: parsed.lines,
        });
    }
    fixtures
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("script_regressions")
}

struct ParsedTokens {
    guard: String,
    lines: Vec<String>,
}

fn parse_tokens_file(path: &Path) -> ParsedTokens {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read tokens file {}: {err}", path.display()));
    let mut headers = BTreeMap::<String, String>::new();
    let mut lines = Vec::<String>::new();

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
                .unwrap_or_else(|| panic!("invalid header in {}: '{line}'", path.display()));
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if headers.insert(key.clone(), value).is_some() {
                panic!("duplicate header '{key}' in {}", path.display());
            }
        } else {
            lines.push(line.to_string());
        }
    }

    assert_eq!(
        headers.get("format").map(String::as_str),
        Some("html5-token-v1"),
        "script regression fixture {} must declare # format: html5-token-v1",
        path.display()
    );
    let guard = headers.get("guard").cloned().unwrap_or_else(|| {
        panic!(
            "script regression fixture {} missing # guard header",
            path.display()
        )
    });
    assert!(
        !guard.trim().is_empty(),
        "script regression fixture {} must have non-empty # guard header",
        path.display()
    );
    assert!(
        !lines.is_empty(),
        "script regression fixture {} has no token lines",
        path.display()
    );
    assert_eq!(
        lines.last().map(String::as_str),
        Some("EOF"),
        "script regression fixture {} must end with EOF",
        path.display()
    );

    ParsedTokens { guard, lines }
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

fn run_script_fixture_whole(fixture: &Fixture) -> Vec<String> {
    run_script_fixture_with_chunks(fixture, std::iter::once(fixture.input.as_str()), false)
}

fn run_script_fixture_every_boundary(fixture: &Fixture) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    for (idx, _) in fixture.input.char_indices().skip(1) {
        chunks.push(&fixture.input[start..idx]);
        start = idx;
    }
    if start < fixture.input.len() {
        chunks.push(&fixture.input[start..]);
    } else if chunks.is_empty() {
        chunks.push(fixture.input.as_str());
    }
    run_script_fixture_with_chunks(fixture, chunks, true)
}

fn run_script_fixture_with_chunks<'a, I>(
    fixture: &Fixture,
    chunks: I,
    expect_token_granular_batches: bool,
) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut ctx = DocumentParseContext::new();
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
        .expect("script atom interning must succeed in script regression harness");
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
            script,
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
        script,
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
    script: html::html5::AtomId,
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
            "tokenizer pumping exceeded iteration budget in script regression '{}' [{label}]\npath: {}\nguard: {}",
            fixture.name,
            fixture.dir.display(),
            fixture.guard
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
            script,
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
                    "tokenizer repeatedly reported Progress without observable progress in script regression '{}' [{label}]\npath: {}\nguard: {}",
                    fixture.name,
                    fixture.dir.display(),
                    fixture.guard
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
    script: html::html5::AtomId,
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
                "script regression harness must observe exactly one token per pump in '{}' [{label}]\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.guard
            );
        }
        let pending_control =
            {
                let resolver = batch.resolver();
                let fmt = TokenFmt::new(&ctx.atoms, &resolver);
                for token in batch.iter() {
                    out.push(fmt.format_token(token).expect(
                        "token formatting in script regression harness must be deterministic",
                    ));
                }
                batch.tokens().first().and_then(|token| match token {
                    Token::StartTag { name, .. } if *name == script && !*text_mode_active => {
                        *text_mode_active = true;
                        Some(TokenizerControl::EnterTextMode(TextModeSpec::script_data(
                            script,
                        )))
                    }
                    Token::EndTag { name } if *name == script && *text_mode_active => {
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
                "unexpected EOF while pushing input in script regression '{}' [{label}]\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.guard
            );
        }
        ("finish", other) => {
            panic!(
                "finish must emit EOF in script regression '{}' [{label}], got {other:?}\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.guard
            );
        }
        _ => {
            panic!(
                "unexpected tokenizer state in script regression '{}' [{label}] stage={stage} result={result:?}\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.guard
            );
        }
    }
}

fn assert_expected(fixture: &Fixture, actual: &[String], label: &str) {
    if actual != fixture.expected {
        panic!(
            "token mismatch in script regression '{}' [{label}]\npath: {}\nguard: {}\n{}",
            fixture.name,
            fixture.dir.display(),
            fixture.guard,
            diff_lines(&fixture.expected, actual)
        );
    }
}
