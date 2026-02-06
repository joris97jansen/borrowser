#![cfg(feature = "html5")]

use html::dom_snapshot::{DomSnapshot, DomSnapshotOptions};
use html::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::{ChunkPlan, materialize_patch_batches, shrink_chunk_plan_with_stats};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureStatus {
    Active,
    Xfail,
}

struct ExpectedDom {
    status: FixtureStatus,
    reason: Option<String>,
    options: DomSnapshotOptions,
    lines: Vec<String>,
}

struct Fixture {
    name: String,
    input: String,
    expected: ExpectedDom,
}

#[derive(Debug)]
enum RunOutput {
    Ok(Vec<String>),
    Err(String),
}

impl RunOutput {
    fn lines(&self) -> Option<&[String]> {
        match self {
            RunOutput::Ok(lines) => Some(lines.as_slice()),
            RunOutput::Err(_) => None,
        }
    }
}

#[test]
fn html5_golden_tree_builder_whole_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        let actual = run_tree_builder_whole(&fixture);
        enforce_expected(&fixture, &actual, Mode::WholeInput, None);
    }
    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tree_builder_chunked_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let fuzz_runs = env_u64("BORROWSER_HTML5_DOM_FUZZ_RUNS", 4) as usize;
    let fuzz_seed = env_u64("BORROWSER_HTML5_DOM_FUZZ_SEED", 0xC0FFEE);
    let mut ran = 0usize;
    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        let whole = run_tree_builder_whole(&fixture);
        if matches!(fixture.expected.status, FixtureStatus::Active)
            && matches!(whole, RunOutput::Err(_))
        {
            panic!(
                "fixture '{}' failed in whole-input mode: {:?}",
                fixture.name, whole
            );
        }
        let plans = chunk_plans(&fixture.input, fuzz_runs, fuzz_seed);
        for plan in plans {
            let actual = run_tree_builder_chunked(&fixture, &plan.plan, &plan.label);
            if let (Some(whole_lines), Some(actual_lines)) = (whole.lines(), actual.lines())
                && actual_lines != whole_lines
            {
                let (shrunk, stats) =
                    shrink_chunk_plan_with_stats(&fixture.input, &plan.plan, |candidate| {
                        match run_tree_builder_chunked(&fixture, candidate, "shrinking") {
                            RunOutput::Ok(lines) => lines.as_slice() != whole_lines,
                            RunOutput::Err(_) => true,
                        }
                    });
                panic!(
                    "chunked output mismatch in fixture '{}'\nplan: {}\nshrunk: {}\nshrink stats: {:?}\n{}",
                    fixture.name,
                    plan.label,
                    shrunk,
                    stats,
                    diff_lines(whole_lines, actual_lines)
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

fn enforce_expected(fixture: &Fixture, actual: &RunOutput, mode: Mode, plan_label: Option<&str>) {
    let label = match plan_label {
        Some(plan) => format!("{} ({})", mode.label(), plan),
        None => mode.label().to_string(),
    };
    match fixture.expected.status {
        FixtureStatus::Active => match actual {
            RunOutput::Ok(lines) => {
                if lines.as_slice() != fixture.expected.lines.as_slice() {
                    panic!(
                        "DOM mismatch in fixture '{}' [{label}]\npath: {}\n{}",
                        fixture.name,
                        fixture_dir(&fixture.name).display(),
                        diff_lines(&fixture.expected.lines, lines)
                    );
                }
            }
            RunOutput::Err(err) => {
                panic!("fixture '{}' [{label}] failed: {err}", fixture.name);
            }
        },
        FixtureStatus::Xfail => match actual {
            RunOutput::Ok(lines) => {
                if lines.as_slice() == fixture.expected.lines.as_slice() {
                    panic!(
                        "fixture '{}' [{label}] matched expected DOM but is marked xfail; reason: {}\npath: {}",
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
            RunOutput::Err(_) => {
                // Expected to fail; keep xfail until implementation lands.
            }
        },
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
        raw: env::var("BORROWSER_HTML5_DOM_FIXTURE").ok(),
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
        let dom_path = path.join("dom.txt");
        let input = fs::read_to_string(&input_path)
            .unwrap_or_else(|err| panic!("failed to read input {input_path:?}: {err}"));
        let expected = parse_dom_file(&dom_path);
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
        .join("tree_builder")
}

fn fixture_dir(name: &str) -> PathBuf {
    fixture_root().join(name)
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
    assert_eq!(format, "html5-dom-v1", "unsupported format in {path:?}");

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

    if lines.is_empty() {
        panic!("dom file {path:?} has no snapshot lines");
    }
    if !lines[0].starts_with("#document") {
        panic!("dom file {path:?} must start with #document");
    }

    ExpectedDom {
        status,
        reason,
        options,
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

fn run_tree_builder_whole(fixture: &Fixture) -> RunOutput {
    run_tree_builder_impl(fixture, None, None)
}

fn run_tree_builder_chunked(fixture: &Fixture, plan: &ChunkPlan, plan_label: &str) -> RunOutput {
    run_tree_builder_impl(fixture, Some(plan), Some(plan_label))
}

fn run_tree_builder_impl(
    fixture: &Fixture,
    plan: Option<&ChunkPlan>,
    plan_label: Option<&str>,
) -> RunOutput {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;
    let label = plan_label.unwrap_or("<whole>");

    let mut push_and_drain = |chunk: &str| -> Result<(), String> {
        input.push_str(chunk);
        handle_tokenize_result(
            tokenizer.push_input(&mut input),
            fixture,
            plan_label,
            "push_input",
        )?;
        drain_batches(DrainCtx {
            tokenizer: &mut tokenizer,
            input: &mut input,
            builder: &mut builder,
            atoms: &ctx.atoms,
            patch_batches: &mut patch_batches,
            fixture_name: &fixture.name,
            label,
            saw_eof_token: &mut saw_eof_token,
        })
    };

    if let Some(plan) = plan {
        // NOTE: keep this exhaustive with ChunkPlan variants; this harness only supports UTF-8-safe input.
        match plan {
            ChunkPlan::Fixed { policy, .. }
            | ChunkPlan::Sizes { policy, .. }
            | ChunkPlan::Boundaries { policy, .. } => {
                if matches!(policy, html::test_harness::BoundaryPolicy::ByteStream) {
                    let plan = plan_label.unwrap_or("<whole>");
                    return RunOutput::Err(format!(
                        "byte-stream chunking is not supported (fixture '{}' [{plan}])",
                        fixture.name
                    ));
                }
            }
        }
        let mut result = Ok(());
        plan.for_each_chunk(&fixture.input, |chunk| {
            if result.is_err() {
                return;
            }
            let chunk_str = match std::str::from_utf8(chunk) {
                Ok(value) => value,
                Err(_) => {
                    let plan = plan_label.unwrap_or("<whole>");
                    result = Err(format!(
                        "chunk plan produced invalid UTF-8 boundary in fixture '{}' [{plan}]",
                        fixture.name
                    ));
                    return;
                }
            };
            if let Err(err) = push_and_drain(chunk_str) {
                result = Err(err);
            }
        });
        if let Err(err) = result {
            return RunOutput::Err(err);
        }
    } else if let Err(err) = push_and_drain(&fixture.input) {
        return RunOutput::Err(err);
    }

    let finish_result = tokenizer.finish();
    if let Err(err) = handle_tokenize_result(finish_result, fixture, plan_label, "finish") {
        return RunOutput::Err(err);
    }
    if let Err(err) = drain_batches(DrainCtx {
        tokenizer: &mut tokenizer,
        input: &mut input,
        builder: &mut builder,
        atoms: &ctx.atoms,
        patch_batches: &mut patch_batches,
        fixture_name: &fixture.name,
        label,
        saw_eof_token: &mut saw_eof_token,
    }) {
        return RunOutput::Err(err);
    }
    if !saw_eof_token {
        let plan = plan_label.unwrap_or("<whole>");
        return RunOutput::Err(format!(
            "expected EOF token but none was observed in fixture '{}' [{plan}]",
            fixture.name
        ));
    }

    let dom = match materialize_patch_batches(&patch_batches) {
        Ok(dom) => dom,
        Err(err) => return RunOutput::Err(err),
    };
    let snapshot = DomSnapshot::new(&dom, fixture.expected.options);
    RunOutput::Ok(snapshot.as_lines().to_vec())
}

fn drain_batches(d: DrainCtx<'_>) -> Result<(), String> {
    let mut patches = Vec::new();
    loop {
        let batch = d.tokenizer.next_batch(d.input);
        if batch.tokens().is_empty() {
            break;
        }
        patches.clear();
        let resolver = batch.resolver();
        let mut sink = html::html5::tree_builder::VecPatchSink(&mut patches);
        for token in batch.iter() {
            if matches!(token, html::html5::Token::Eof) {
                *d.saw_eof_token = true;
            }
            match d.builder.push_token(token, d.atoms, &resolver, &mut sink) {
                Ok(TreeBuilderStepResult::Continue) => {}
                Ok(TreeBuilderStepResult::Suspend(reason)) => {
                    return Err(format!(
                        "tree builder suspended in fixture '{}' [{}] reason={reason:?}",
                        d.fixture_name, d.label
                    ));
                }
                Err(err) => {
                    return Err(format!(
                        "tree builder error in fixture '{}' [{}] error={err:?}",
                        d.fixture_name, d.label
                    ));
                }
            }
        }
        if !patches.is_empty() {
            d.patch_batches.push(std::mem::take(&mut patches));
        }
    }
    Ok(())
}

struct DrainCtx<'a> {
    tokenizer: &'a mut Html5Tokenizer,
    input: &'a mut Input,
    builder: &'a mut Html5TreeBuilder,
    atoms: &'a html::html5::AtomTable,
    patch_batches: &'a mut Vec<Vec<html::DomPatch>>,
    fixture_name: &'a str,
    label: &'a str,
    saw_eof_token: &'a mut bool,
}

fn handle_tokenize_result(
    result: TokenizeResult,
    fixture: &Fixture,
    plan_label: Option<&str>,
    stage: &str,
) -> Result<(), String> {
    match (stage, result) {
        ("push_input", TokenizeResult::EmittedEof) => {
            let plan = plan_label.unwrap_or("<whole>");
            Err(format!(
                "unexpected EOF while pushing input in fixture '{}' [{plan}]",
                fixture.name
            ))
        }
        (
            "finish",
            TokenizeResult::EmittedEof | TokenizeResult::Progress | TokenizeResult::NeedMoreInput,
        ) => Ok(()),
        ("push_input", TokenizeResult::NeedMoreInput | TokenizeResult::Progress) => Ok(()),
        _ => {
            let plan = plan_label.unwrap_or("<whole>");
            Err(format!(
                "unexpected tokenizer state in fixture '{}' [{plan}] stage={stage} result={result:?}",
                fixture.name
            ))
        }
    }
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
