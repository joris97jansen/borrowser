#![cfg(feature = "html5")]

use html::html5::{
    AttributeValue, DocumentParseContext, Html5Tokenizer, Input, TextResolver, Token,
    TokenizerConfig,
};
use html::test_harness::{ChunkPlan, shrink_chunk_plan_with_stats};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureStatus {
    Active,
    Xfail,
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
        let actual = run_tokenizer_whole(&fixture);
        enforce_expected(&fixture, &actual, Mode::WholeInput, None);
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tokenizer_chunked_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let fuzz_runs = env_u64("BORROWSER_HTML5_TOKEN_FUZZ_RUNS", 4) as usize;
    let fuzz_seed = env_u64("BORROWSER_HTML5_TOKEN_FUZZ_SEED", 0xC0FFEE);
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        let whole = run_tokenizer_whole(&fixture);
        let plans = chunk_plans(&fixture.input, fuzz_runs, fuzz_seed);
        for plan in plans {
            let actual = run_tokenizer_chunked(&fixture, &plan.plan, &plan.label);
            if actual != whole {
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

struct PlanCase {
    label: String,
    plan: ChunkPlan,
}

fn chunk_plans(input: &str, fuzz_runs: usize, fuzz_seed: u64) -> Vec<PlanCase> {
    let mut plans = Vec::new();
    for size in [1usize, 2, 3, 4, 8, 16, 32, 64] {
        plans.push(PlanCase {
            label: format!("fixed size={size}"),
            plan: ChunkPlan::fixed(size),
        });
    }

    let boundaries = char_boundaries(input);
    if !boundaries.is_empty() && fuzz_runs > 0 {
        let max = boundaries.len().clamp(1, 32);
        for i in 0..fuzz_runs {
            let seed = fuzz_seed.wrapping_add(i as u64);
            let mut rng = Lcg::new(seed);
            let mut picks = boundaries.clone();
            rng.shuffle(&mut picks);
            let count = 1 + rng.gen_range(max);
            picks.truncate(count);
            picks.sort_unstable();
            picks.dedup();
            plans.push(PlanCase {
                label: format!("fuzz boundaries count={} seed=0x{:016x}", picks.len(), seed),
                plan: ChunkPlan::boundaries(picks),
            });
        }
    }

    plans
}

fn char_boundaries(input: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let len = input.len();
    for (idx, _) in input.char_indices() {
        if idx != 0 && idx != len {
            out.push(idx);
        }
    }
    out
}

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.state
    }

    fn gen_range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        (self.next_u64() >> 32) as usize % upper
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        if items.len() < 2 {
            return;
        }
        for i in (1..items.len()).rev() {
            let j = self.gen_range(i + 1);
            items.swap(i, j);
        }
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
        Some(other) => panic!("unsupported status '{other}' in {path:?}"),
    };
    let reason = headers.get("reason").cloned();
    if status == FixtureStatus::Xfail && reason.as_deref().unwrap_or("").is_empty() {
        panic!("xfail fixture missing reason in {path:?}");
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
    handle_tokenize_result(
        tokenizer.push_input(&mut buffer),
        fixture,
        Mode::WholeInput,
        None,
        "push_input",
    );
    let mut out = Vec::new();
    drain_tokens(&mut out, &mut tokenizer, &mut buffer, &ctx, fixture, None);
    handle_tokenize_result(
        tokenizer.finish(),
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
        handle_tokenize_result(
            tokenizer.push_input(&mut buffer),
            fixture,
            Mode::ChunkedInput,
            Some(plan_label),
            "push_input",
        );
        drain_tokens(
            &mut out,
            &mut tokenizer,
            &mut buffer,
            &ctx,
            fixture,
            Some(plan_label),
        );
    });
    handle_tokenize_result(
        tokenizer.finish(),
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

fn drain_tokens(
    out: &mut Vec<String>,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &DocumentParseContext,
    fixture: &Fixture,
    plan_label: Option<&str>,
) {
    let mut index = out.len();
    loop {
        let batch = tokenizer.next_batch(buffer);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        out.extend(format_tokens(
            batch.tokens(),
            &resolver,
            ctx,
            fixture,
            plan_label,
            &mut index,
        ));
    }
}

fn format_tokens(
    tokens: &[Token],
    resolver: &impl TextResolver,
    ctx: &DocumentParseContext,
    fixture: &Fixture,
    plan_label: Option<&str>,
    index: &mut usize,
) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let token_index = *index;
        *index = index.saturating_add(1);
        let line = match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                let name = match name {
                    None => "null".to_string(),
                    Some(id) => ctx
                        .atoms
                        .resolve(*id)
                        .unwrap_or_else(|| panic!("unknown atom id in doctype: {id:?}"))
                        .to_string(),
                };
                let public_id = public_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                let system_id = system_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                format!(
                    "DOCTYPE name={name} public_id={public_id} system_id={system_id} force_quirks={force_quirks}"
                )
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let name = ctx
                    .atoms
                    .resolve(*name)
                    .unwrap_or_else(|| panic!("unknown atom id in start tag: {name:?}"));
                let mut line = String::new();
                line.push_str("START name=");
                line.push_str(name);
                line.push_str(" attrs=[");
                for (attr_index, attr) in attributes.iter().enumerate() {
                    if attr_index > 0 {
                        line.push(' ');
                    }
                    line.push_str(
                        format_attr(
                            attr,
                            resolver,
                            ctx,
                            fixture,
                            plan_label,
                            token_index,
                            attr_index,
                        )
                        .as_str(),
                    );
                }
                line.push_str("] self_closing=");
                line.push_str(if *self_closing { "true" } else { "false" });
                line
            }
            Token::EndTag { name } => {
                let name = ctx
                    .atoms
                    .resolve(*name)
                    .unwrap_or_else(|| panic!("unknown atom id in end tag: {name:?}"));
                format!("END name={name}")
            }
            Token::Comment { text } => {
                let text = resolver.resolve_span(*text).unwrap_or_else(|| {
                    panic!(
                        "invalid text span in fixture '{}' [{}] token #{} (comment)",
                        fixture.name,
                        plan_label.unwrap_or("whole"),
                        token_index
                    )
                });
                format!("COMMENT text=\"{}\"", escape_text(text))
            }
            Token::Character { span } => {
                let text = resolver.resolve_span(*span).unwrap_or_else(|| {
                    panic!(
                        "invalid text span in fixture '{}' [{}] token #{} (char)",
                        fixture.name,
                        plan_label.unwrap_or("whole"),
                        token_index
                    )
                });
                format!("CHAR text=\"{}\"", escape_text(text))
            }
            Token::Eof => "EOF".to_string(),
        };
        out.push(line);
    }
    out
}

fn format_attr(
    attr: &html::html5::Attribute,
    resolver: &impl TextResolver,
    ctx: &DocumentParseContext,
    fixture: &Fixture,
    plan_label: Option<&str>,
    token_index: usize,
    attr_index: usize,
) -> String {
    let name = ctx
        .atoms
        .resolve(attr.name)
        .unwrap_or_else(|| panic!("unknown atom id in attribute: {:?}", attr.name));
    match &attr.value {
        None => name.to_string(),
        Some(AttributeValue::Span(span)) => {
            let value = resolver.resolve_span(*span).unwrap_or_else(|| {
                panic!(
                    "invalid attribute value span in fixture '{}' [{}] token_seq #{} attr #{} ({})",
                    fixture.name,
                    plan_label.unwrap_or("whole"),
                    token_index,
                    attr_index,
                    name
                )
            });
            format!("{name}=\"{}\"", escape_text(value))
        }
        Some(AttributeValue::Owned(value)) => {
            format!("{name}=\"{}\"", escape_text(value))
        }
    }
}

fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => {
                use std::fmt::Write;
                let _ = write!(&mut out, "\\u{{{:02X}}}", ch as u32);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn diff_lines(expected: &[String], actual: &[String]) -> String {
    let max = expected.len().max(actual.len());
    let mut out = String::new();
    use std::fmt::Write;
    let mut mismatch = None;
    for i in 0..max {
        let left = expected.get(i).map(String::as_str).unwrap_or("<none>");
        let right = actual.get(i).map(String::as_str).unwrap_or("<none>");
        if left != right {
            mismatch = Some(i);
            break;
        }
    }
    if let Some(i) = mismatch {
        let start = i.saturating_sub(2);
        let end = (i + 3).min(max);
        let _ = writeln!(
            &mut out,
            "first mismatch at line {} (showing {}..={}):",
            i + 1,
            start + 1,
            end
        );
        for line_idx in start..end {
            let left = expected
                .get(line_idx)
                .map(String::as_str)
                .unwrap_or("<none>");
            let right = actual.get(line_idx).map(String::as_str).unwrap_or("<none>");
            let marker = if line_idx == i { ">" } else { " " };
            let _ = writeln!(&mut out, "{marker} {:>4}  expected: {left}", line_idx + 1);
            let _ = writeln!(&mut out, "{marker} {:>4}    actual: {right}", line_idx + 1);
        }
    }
    if expected.len() != actual.len() && mismatch.is_none() {
        let _ = writeln!(
            &mut out,
            "prefix matched but lengths differ (expected {} lines, actual {} lines)",
            expected.len(),
            actual.len()
        );
    }
    let _ = writeln!(
        &mut out,
        "expected {} lines, actual {} lines",
        expected.len(),
        actual.len()
    );
    out
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
