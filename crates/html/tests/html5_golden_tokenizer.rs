#![cfg(feature = "html5")]

use html::chunker::{ChunkerConfig, build_chunk_plans, utf8_internal_boundaries};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::{ChunkPlan, shrink_chunk_plan_with_stats};
use html_test_support::diff_lines;
use html_test_support::token_snapshot;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureStatus {
    Active,
    Xfail,
    Skip,
}

struct ExpectedTokens {
    status: FixtureStatus,
    reason: Option<String>,
    lines: Vec<String>,
}

struct Fixture {
    name: String,
    input: String,
    expected: ExpectedTokens,
}

const MIN_TOKENIZER_FIXTURE_COUNT: usize = 20;
const EVERY_BOUNDARY_MAX_BYTES: usize = 256;
const MAX_PUMP_ITERATIONS_BASE: usize = 1024;

#[test]
fn html5_golden_tokenizer_fixture_corpus_contract() {
    let fixtures = load_fixtures();
    assert!(
        fixtures.len() >= MIN_TOKENIZER_FIXTURE_COUNT,
        "tokenizer golden corpus too small: found {} fixtures, require at least {}",
        fixtures.len(),
        MIN_TOKENIZER_FIXTURE_COUNT
    );

    let mut has_tag = false;
    let mut has_attr = false;
    let mut has_comment = false;
    let mut has_doctype = false;
    let mut has_named_charref_input = false;
    let mut has_numeric_charref_input = false;
    let mut has_named_decode_effect = false;
    let mut has_numeric_decode_effect = false;
    for fixture in &fixtures {
        if fixture.expected.status == FixtureStatus::Skip {
            continue;
        }
        let has_amp_input = fixture.input.contains("&amp;");
        let has_lt_input = fixture.input.contains("&lt;");
        let has_gt_input = fixture.input.contains("&gt;");
        // We want `&` evidence that likely comes from decoding `&amp;`, not from
        // unrelated literal ampersands in the original fixture input.
        let has_literal_amp_in_input = fixture.input.replace("&amp;", "").contains('&');
        let has_literal_a_in_input = fixture.input.contains('A');
        has_named_charref_input |= has_amp_input || has_lt_input || has_gt_input;
        let has_numeric_input = fixture.input.contains("&#");
        has_numeric_charref_input |= has_numeric_input;

        let mut has_char_amp = false;
        let mut has_char_amp_literal = false;
        let mut has_char_lt = false;
        let mut has_char_lt_literal = false;
        let mut has_char_gt = false;
        let mut has_char_gt_literal = false;
        let mut has_char_a = false;
        let mut has_char_numeric_literal = false;
        for line in &fixture.expected.lines {
            if line.starts_with("CHAR ") {
                has_char_amp |= line.contains('&');
                has_char_amp_literal |= line.contains("&amp;");
                has_char_lt |= line.contains('<');
                has_char_lt_literal |= line.contains("&lt;");
                has_char_gt |= line.contains('>');
                has_char_gt_literal |= line.contains("&gt;");
                has_char_a |= line.contains("CHAR text=\"") && line.contains('A');
                has_char_numeric_literal |= line.contains("&#") || line.contains("&#x");
            }
            if line.starts_with("START ") || line.starts_with("END ") {
                has_tag = true;
            }
            if line.starts_with("START ") && !line.contains("attrs=[]") {
                has_attr = true;
            }
            if line.starts_with("COMMENT ") {
                has_comment = true;
            }
            if line.starts_with("DOCTYPE ") {
                has_doctype = true;
            }
        }
        if has_amp_input && !has_literal_amp_in_input && has_char_amp && !has_char_amp_literal {
            has_named_decode_effect = true;
        }
        if has_lt_input && has_char_lt && !has_char_lt_literal {
            has_named_decode_effect = true;
        }
        if has_gt_input && has_char_gt && !has_char_gt_literal {
            has_named_decode_effect = true;
        }
        if (fixture.input.contains("&#65;") || fixture.input.contains("&#x41;"))
            && !has_literal_a_in_input
            && has_char_a
            && !has_char_numeric_literal
        {
            has_numeric_decode_effect = true;
        }
    }

    assert!(has_tag, "tokenizer corpus missing tag coverage");
    assert!(has_attr, "tokenizer corpus missing attribute coverage");
    assert!(has_comment, "tokenizer corpus missing comment coverage");
    assert!(has_doctype, "tokenizer corpus missing doctype coverage");
    assert!(
        has_named_charref_input,
        "tokenizer corpus missing named-charref-like input coverage"
    );
    assert!(
        has_numeric_charref_input,
        "tokenizer corpus missing numeric-charref-like input coverage"
    );
    assert!(
        has_named_decode_effect,
        "tokenizer corpus missing named-charref decode effect in expected outputs"
    );
    assert!(
        has_numeric_decode_effect,
        "tokenizer corpus missing numeric-charref decode effect in expected outputs"
    );
}

#[test]
fn html5_golden_tokenizer_whole_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        if fixture.expected.status == FixtureStatus::Skip {
            continue;
        }
        let actual = run_tokenizer_whole(&fixture);
        enforce_expected(&fixture, &actual, Mode::WholeInput, None);
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tokenizer_chunked_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let mut fuzz_runs = env_u64("BORROWSER_HTML5_TOKEN_FUZZ_RUNS", 4) as usize;
    if env::var("CI").is_ok() && fuzz_runs == 0 {
        fuzz_runs = 1;
    }
    let fuzz_seed = env_u64("BORROWSER_HTML5_TOKEN_FUZZ_SEED", 0xC0FFEE);
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        if fixture.expected.status == FixtureStatus::Skip {
            continue;
        }
        let whole = run_tokenizer_whole(&fixture);
        let plans = build_tokenizer_chunk_plans(&fixture.input, fuzz_runs, fuzz_seed);
        for plan in plans {
            let actual = run_tokenizer_chunked(&fixture, &plan.plan, &plan.label);
            if fixture.expected.status == FixtureStatus::Active && actual != whole {
                let (shrunk, stats) =
                    shrink_chunk_plan_with_stats(&fixture.input, &plan.plan, |candidate| {
                        run_tokenizer_chunked(&fixture, candidate, "shrinking") != whole
                    });
                panic!(
                    "chunked output mismatch in fixture '{}'\nplan: {}\nshrunk: {}\nshrink stats: {:?}\n{}",
                    fixture.name,
                    plan.label,
                    shrunk,
                    stats,
                    diff_lines(&whole, &actual)
                );
            }
            enforce_expected(&fixture, &actual, Mode::ChunkedInput, Some(&plan.label));
        }
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tokenizer_chunk_plan_generation_is_seed_deterministic() {
    let input = "<div a=\"x\">Tom&amp;Jerry</div>";
    let seed = 0xC0FFEE_u64;
    let runs = 4usize;
    let a = build_tokenizer_chunk_plans(input, runs, seed);
    let b = build_tokenizer_chunk_plans(input, runs, seed);
    assert_eq!(a.len(), b.len(), "chunk plan count must be deterministic");
    for (left, right) in a.iter().zip(b.iter()) {
        assert_eq!(left.label, right.label, "chunk plan labels must match");
        assert_eq!(left.plan, right.plan, "chunk plan definitions must match");
    }
}

fn build_tokenizer_chunk_plans(
    input: &str,
    fuzz_runs: usize,
    fuzz_seed: u64,
) -> Vec<html::chunker::ChunkPlanCase> {
    let mut plans = build_chunk_plans(input, fuzz_runs, fuzz_seed, ChunkerConfig::utf8());
    if let Some(plan) = every_boundary_plan_for_small_input(input) {
        plans.push(plan);
    }
    plans
}

fn every_boundary_plan_for_small_input(input: &str) -> Option<html::chunker::ChunkPlanCase> {
    if input.len() <= 1 || input.len() > EVERY_BOUNDARY_MAX_BYTES {
        return None;
    }
    let boundaries = utf8_internal_boundaries(input);
    if boundaries.is_empty() {
        return None;
    }
    Some(html::chunker::ChunkPlanCase {
        label: format!("every-boundary utf8 count={}", boundaries.len()),
        plan: ChunkPlan::boundaries(boundaries),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    WholeInput,
    ChunkedInput,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::WholeInput => "whole",
            Mode::ChunkedInput => "chunked",
        }
    }
}

fn enforce_expected(fixture: &Fixture, actual: &[String], mode: Mode, plan_label: Option<&str>) {
    let mismatch = actual != fixture.expected.lines;
    let label = match plan_label {
        Some(plan) => format!("{} ({})", mode.label(), plan),
        None => mode.label().to_string(),
    };
    match fixture.expected.status {
        FixtureStatus::Active => {
            if mismatch {
                panic!(
                    "token mismatch in fixture '{}' [{label}]\npath: {}\n{}",
                    fixture.name,
                    fixture_dir(&fixture.name).display(),
                    diff_lines(&fixture.expected.lines, actual)
                );
            }
        }
        FixtureStatus::Xfail => {
            if !mismatch {
                panic!(
                    "fixture '{}' [{label}] matched expected tokens but is marked xfail; reason: {}\npath: {}",
                    fixture.name,
                    fixture
                        .expected
                        .reason
                        .as_deref()
                        .unwrap_or("<missing reason>"),
                    fixture_dir(&fixture.name).display()
                );
            }
        }
        FixtureStatus::Skip => {}
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
        raw: env::var("BORROWSER_HTML5_TOKEN_FIXTURE").ok(),
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn load_fixtures() -> Vec<Fixture> {
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

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tokenizer")
}

fn fixture_dir(name: &str) -> PathBuf {
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

fn run_tokenizer_whole(fixture: &Fixture) -> Vec<String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut buffer = Input::new();
    buffer.push_str(&fixture.input);
    let mut out = Vec::new();
    pump_until_blocked(
        &mut tokenizer,
        &mut buffer,
        &mut ctx,
        fixture,
        Mode::WholeInput,
        None,
        &mut out,
    );
    handle_tokenize_result(
        tokenizer.finish(&buffer),
        fixture,
        Mode::WholeInput,
        None,
        "finish",
    );
    drain_tokens(&mut out, &mut tokenizer, &mut buffer, &ctx, fixture, None);
    out
}

fn run_tokenizer_chunked(fixture: &Fixture, plan: &ChunkPlan, plan_label: &str) -> Vec<String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut buffer = Input::new();
    let mut out = Vec::new();
    // NOTE: keep this exhaustive with ChunkPlan variants; this harness only supports UTF-8-safe input.
    match plan {
        ChunkPlan::Fixed { policy, .. }
        | ChunkPlan::Sizes { policy, .. }
        | ChunkPlan::Boundaries { policy, .. } => {
            if matches!(policy, html::test_harness::BoundaryPolicy::ByteStream) {
                panic!(
                    "byte-stream chunking is not supported for HTML5 tokenizer harness (fixture '{}' [{plan_label}])",
                    fixture.name
                );
            }
        }
    }
    plan.for_each_chunk(&fixture.input, |chunk| {
        let chunk_str = std::str::from_utf8(chunk).unwrap_or_else(|_| {
            panic!(
                "chunk plan produced invalid UTF-8 boundary in fixture '{}' [{plan_label}]",
                fixture.name
            )
        });
        buffer.push_str(chunk_str);
        pump_until_blocked(
            &mut tokenizer,
            &mut buffer,
            &mut ctx,
            fixture,
            Mode::ChunkedInput,
            Some(plan_label),
            &mut out,
        );
    });
    handle_tokenize_result(
        tokenizer.finish(&buffer),
        fixture,
        Mode::ChunkedInput,
        Some(plan_label),
        "finish",
    );
    drain_tokens(
        &mut out,
        &mut tokenizer,
        &mut buffer,
        &ctx,
        fixture,
        Some(plan_label),
    );
    out
}

fn pump_until_blocked(
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &mut DocumentParseContext,
    fixture: &Fixture,
    mode: Mode,
    plan_label: Option<&str>,
    out: &mut Vec<String>,
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
            "tokenizer pumping exceeded iteration budget in fixture '{}' [{}]",
            fixture.name,
            plan_label.unwrap_or(mode.label())
        );
        let stats_before = tokenizer.stats();
        let out_len_before = out.len();
        let result = tokenizer.push_input(buffer, ctx);
        handle_tokenize_result(result, fixture, mode, plan_label, "push_input");
        drain_tokens(out, tokenizer, buffer, ctx, fixture, plan_label);
        let stats_after = tokenizer.stats();
        let out_len_after = out.len();
        let consumed = stats_after.bytes_consumed != stats_before.bytes_consumed;
        let emitted = out_len_after != out_len_before;
        if result == TokenizeResult::Progress {
            if consumed || emitted {
                stalled_progress_pumps = 0;
            } else {
                stalled_progress_pumps = stalled_progress_pumps.saturating_add(1);
                assert!(
                    stalled_progress_pumps <= 8,
                    "tokenizer repeatedly reported Progress without observable progress in fixture '{}' [{}] (stalled_progress_pumps={} bytes_before={} bytes_after={} out_before={} out_after={})",
                    fixture.name,
                    plan_label.unwrap_or(mode.label()),
                    stalled_progress_pumps,
                    stats_before.bytes_consumed,
                    stats_after.bytes_consumed,
                    out_len_before,
                    out_len_after
                );
            }
        }
        if result == TokenizeResult::NeedMoreInput {
            // If we consumed or emitted, we clearly progressed and should pump again.
            if consumed || emitted {
                stalled_progress_pumps = 0;
                continue;
            }
            let buf_len = buffer.as_str().len() as u64;

            // Legitimate blocking point: no progress and already at end-of-buffer.
            if stats_after.bytes_consumed >= buf_len {
                break;
            }

            // Some tokenizer states may legally block mid-buffer while waiting
            // for additional disambiguating bytes. Enable strict mode to assert
            // on repeated mid-buffer stalls while debugging.
            let strict_midbuffer_stall = env::var("BORROWSER_HTML5_STRICT_MIDBUFFER_STALL")
                .ok()
                .as_deref()
                == Some("1");
            if strict_midbuffer_stall {
                stalled_progress_pumps = stalled_progress_pumps.saturating_add(1);
                assert!(
                    stalled_progress_pumps <= 8,
                    "tokenizer returned NeedMoreInput before end-of-buffer without progress in fixture '{}' [{}] (stalled_progress_pumps={} bytes_consumed={} buf_len={} out_before={} out_after={})",
                    fixture.name,
                    plan_label.unwrap_or(mode.label()),
                    stalled_progress_pumps,
                    stats_after.bytes_consumed,
                    buf_len,
                    out_len_before,
                    out_len_after
                );
                continue;
            }
            break;
        }
    }
}

fn drain_tokens(
    out: &mut Vec<String>,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &DocumentParseContext,
    fixture: &Fixture,
    plan_label: Option<&str>,
) {
    let mut index = out.len();
    let context = token_snapshot::TokenFormatContext {
        case_id: &fixture.name,
        mode: plan_label.unwrap_or("whole"),
    };
    loop {
        let batch = tokenizer.next_batch(buffer);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        out.extend(
            token_snapshot::format_tokens(
                batch.tokens(),
                &resolver,
                ctx,
                &context,
                &mut index,
                None,
            )
            .unwrap_or_else(|err| panic!("{err}")),
        );
    }
}

fn handle_tokenize_result(
    result: html::html5::TokenizeResult,
    fixture: &Fixture,
    mode: Mode,
    plan_label: Option<&str>,
    stage: &str,
) {
    match (stage, result) {
        ("push_input", html::html5::TokenizeResult::EmittedEof) => {
            panic!(
                "unexpected EOF while pushing input in fixture '{}' [{}]",
                fixture.name,
                plan_label.unwrap_or(mode.label())
            );
        }
        ("finish", html::html5::TokenizeResult::EmittedEof) => {}
        ("finish", other) => {
            panic!(
                "finish must emit EOF in fixture '{}' [{}], got {other:?}",
                fixture.name,
                plan_label.unwrap_or(mode.label())
            );
        }
        ("push_input", html::html5::TokenizeResult::NeedMoreInput)
        | ("push_input", html::html5::TokenizeResult::Progress) => {}
        _ => {
            panic!(
                "unexpected tokenizer state in fixture '{}' [{}] stage={stage} result={result:?}",
                fixture.name,
                plan_label.unwrap_or(mode.label())
            );
        }
    }
}
