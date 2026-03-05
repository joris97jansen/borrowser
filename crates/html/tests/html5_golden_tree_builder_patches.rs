#![cfg(feature = "html5")]

use html::chunker::{ChunkerConfig, build_chunk_plans};
use html::html5::tree_builder::{
    Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult, VecPatchSink,
};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::shrink_chunk_plan_with_stats;
use html_test_support::{diff_lines, escape_text};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureStatus {
    Active,
    Xfail,
}

struct ExpectedPatches {
    status: FixtureStatus,
    reason: Option<String>,
    lines: Vec<String>,
}

struct Fixture {
    name: String,
    input: String,
    expected: ExpectedPatches,
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

const BATCH_MARKER_PREFIX: &str = "Batch index=";

fn parse_batch_marker_line(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix(BATCH_MARKER_PREFIX)?;
    let (index_str, size_str) = rest.split_once(" size=")?;
    let index = index_str.parse::<usize>().ok()?;
    let size = size_str.parse::<usize>().ok()?;
    Some((index, size))
}

fn is_batch_marker_line(line: &str) -> bool {
    parse_batch_marker_line(line).is_some()
}

fn batch_markers_filtered(lines: &[String]) -> impl Iterator<Item = &str> {
    lines
        .iter()
        .map(String::as_str)
        .filter(|line| !is_batch_marker_line(line))
}

fn filtered_lines_for_diff(lines: &[String]) -> Vec<String> {
    batch_markers_filtered(lines)
        .map(std::borrow::ToOwned::to_owned)
        .collect()
}

fn batch_partition_summary(lines: &[String]) -> String {
    let mut parts = Vec::new();
    for line in lines {
        if let Some((index, size)) = parse_batch_marker_line(line) {
            parts.push(format!("{index}:{size}"));
        }
    }
    if parts.is_empty() {
        "<none>".to_string()
    } else {
        parts.join(", ")
    }
}

fn lines_match(mode: Mode, actual: &[String], expected: &[String]) -> bool {
    if mode == Mode::WholeInput {
        actual == expected
    } else {
        batch_markers_filtered(actual).eq(batch_markers_filtered(expected))
    }
}

#[test]
fn html5_golden_tree_builder_patches_whole_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let update = update_mode();
    let mut ran = 0usize;

    for fixture in fixtures {
        if !filter.matches(&fixture.name) {
            continue;
        }
        ran += 1;
        let actual = run_tree_builder_whole(&fixture);
        enforce_expected(&fixture, &actual, Mode::WholeInput, None, update);
    }

    assert!(ran > 0, "no fixtures matched filter");
}

#[test]
fn html5_golden_tree_builder_patches_chunked_input() {
    let fixtures = load_fixtures();
    let filter = fixture_filter();
    let update = update_mode();
    if update {
        return;
    }
    let mut fuzz_runs = env_u64("BORROWSER_HTML5_PATCH_FUZZ_RUNS", 4) as usize;
    if env::var("CI").is_ok() && fuzz_runs == 0 {
        fuzz_runs = 1;
    }
    let fuzz_seed = env_u64("BORROWSER_HTML5_PATCH_FUZZ_SEED", 0xC0FFEE);
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

        let plans = build_chunk_plans(&fixture.input, fuzz_runs, fuzz_seed, ChunkerConfig::utf8());
        for plan in plans {
            let actual = run_tree_builder_chunked(&fixture, &plan.plan, &plan.label);
            if let (Some(whole_lines), Some(actual_lines)) = (whole.lines(), actual.lines())
                && !lines_match(Mode::ChunkedInput, actual_lines, whole_lines)
            {
                let (shrunk, stats) =
                    shrink_chunk_plan_with_stats(&fixture.input, &plan.plan, |candidate| {
                        match run_tree_builder_chunked(&fixture, candidate, "shrinking") {
                            RunOutput::Ok(lines) => {
                                !lines_match(Mode::ChunkedInput, lines.as_slice(), whole_lines)
                            }
                            RunOutput::Err(_) => true,
                        }
                    });
                let whole_filtered = filtered_lines_for_diff(whole_lines);
                let actual_filtered = filtered_lines_for_diff(actual_lines);
                let diff = diff_lines(&whole_filtered, &actual_filtered);
                let whole_batches = batch_partition_summary(whole_lines);
                let actual_batches = batch_partition_summary(actual_lines);
                panic!(
                    "chunked patch mismatch in fixture '{}'\nplan: {}\nshrunk: {}\nshrink stats: {:?}\nwhole batches: [{}]\nchunked batches: [{}]\n{}",
                    fixture.name, plan.label, shrunk, stats, whole_batches, actual_batches, diff
                );
            }
            enforce_expected(
                &fixture,
                &actual,
                Mode::ChunkedInput,
                Some(&plan.label),
                update,
            );
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

fn enforce_expected(
    fixture: &Fixture,
    actual: &RunOutput,
    mode: Mode,
    plan_label: Option<&str>,
    update: bool,
) {
    let label = match plan_label {
        Some(plan) => format!("{} ({})", mode.label(), plan),
        None => mode.label().to_string(),
    };

    if update && mode == Mode::WholeInput && plan_label.is_none() {
        if fixture.expected.status == FixtureStatus::Xfail {
            panic!(
                "refusing to update xfail fixture '{}' in update mode; resolve status first\npath: {}",
                fixture.name,
                fixture_dir(&fixture.name).display()
            );
        }
        match actual {
            RunOutput::Ok(lines) => {
                write_expected_patch_file(fixture, lines);
                return;
            }
            RunOutput::Err(err) => {
                panic!(
                    "refusing to update fixture '{}' because run failed: {err}\npath: {}",
                    fixture.name,
                    fixture_dir(&fixture.name).display()
                );
            }
        }
    }

    match fixture.expected.status {
        FixtureStatus::Active => match actual {
            RunOutput::Ok(lines) => {
                let matches_expected =
                    lines_match(mode, lines.as_slice(), fixture.expected.lines.as_slice());
                if !matches_expected {
                    panic!(
                        "patch mismatch in fixture '{}' [{label}]\npath: {}\n{}",
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
                let matches_expected =
                    lines_match(mode, lines.as_slice(), fixture.expected.lines.as_slice());
                if matches_expected {
                    panic!(
                        "fixture '{}' [{label}] matched expected patches but is marked xfail; reason: {}\npath: {}",
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
        raw: env::var("BORROWSER_HTML5_PATCH_FIXTURE").ok(),
    }
}

fn update_mode() -> bool {
    matches!(
        env::var("BORROWSER_HTML5_PATCH_FIXTURE_UPDATE").as_deref(),
        Ok("1")
    )
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
    // Strip one terminal line ending so fixture semantics are not editor-dependent.
    if input.ends_with("\r\n") {
        input.truncate(input.len() - 2);
    } else if input.ends_with('\n') {
        input.pop();
    }
    input
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

#[test]
fn batch_marker_parsing_accepts_exact_numeric_shape() {
    assert_eq!(
        parse_batch_marker_line("Batch index=0 size=13"),
        Some((0, 13))
    );
    assert!(is_batch_marker_line("Batch index=9 size=0"));
}

#[test]
fn batch_marker_parsing_rejects_non_contract_shapes() {
    assert_eq!(parse_batch_marker_line("Batch index=foo size=13"), None);
    assert_eq!(parse_batch_marker_line("Batch index=1 size=bar"), None);
    assert_eq!(parse_batch_marker_line("Batch index=1 size=13 extra"), None);
    assert_eq!(
        parse_batch_marker_line("CreateText key=1 text=\"Batch index=1 size=13\""),
        None
    );
    assert!(!is_batch_marker_line("Batch index=1 size=13 extra"));
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("html5")
        .join("tree_builder_patches")
}

fn fixture_dir(name: &str) -> PathBuf {
    fixture_root().join(name)
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

fn write_expected_patch_file(fixture: &Fixture, lines: &[String]) {
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

fn run_tree_builder_whole(fixture: &Fixture) -> RunOutput {
    run_tree_builder_impl(fixture, None, None)
}

fn run_tree_builder_chunked(
    fixture: &Fixture,
    plan: &html::test_harness::ChunkPlan,
    plan_label: &str,
) -> RunOutput {
    run_tree_builder_impl(fixture, Some(plan), Some(plan_label))
}

fn run_tree_builder_impl(
    fixture: &Fixture,
    plan: Option<&html::test_harness::ChunkPlan>,
    plan_label: Option<&str>,
) -> RunOutput {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder = match Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx) {
        Ok(builder) => builder,
        Err(err) => return RunOutput::Err(format!("failed to init tree builder: {err:?}")),
    };
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;
    let label = plan_label.unwrap_or("<whole>");

    let mut push_and_drain = |chunk: &str| -> Result<(), String> {
        input.push_str(chunk);
        handle_tokenize_result(
            tokenizer.push_input(&mut input, &mut ctx),
            fixture,
            plan_label,
            "push_input",
        )?;
        drain_batches(DrainBatchesCtx {
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
        match plan {
            html::test_harness::ChunkPlan::Fixed { policy, .. }
            | html::test_harness::ChunkPlan::Sizes { policy, .. }
            | html::test_harness::ChunkPlan::Boundaries { policy, .. } => {
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

    let finish_result = tokenizer.finish(&input);
    if let Err(err) = handle_tokenize_result(finish_result, fixture, plan_label, "finish") {
        return RunOutput::Err(err);
    }
    if let Err(err) = drain_batches(DrainBatchesCtx {
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

    for batch_index in 0..patch_batches.len() {
        if let Err(err) =
            html::test_harness::materialize_patch_batches(&patch_batches[..=batch_index])
        {
            return RunOutput::Err(format!(
                "patch batches failed materialization in fixture '{}' [{}] at batch {batch_index}/{}: {}",
                fixture.name,
                label,
                patch_batches.len().saturating_sub(1),
                err
            ));
        }
    }

    RunOutput::Ok(format_patch_batches(&patch_batches))
}

struct DrainBatchesCtx<'a> {
    tokenizer: &'a mut Html5Tokenizer,
    input: &'a mut Input,
    builder: &'a mut Html5TreeBuilder,
    atoms: &'a html::html5::AtomTable,
    patch_batches: &'a mut Vec<Vec<html::DomPatch>>,
    fixture_name: &'a str,
    label: &'a str,
    saw_eof_token: &'a mut bool,
}

fn drain_batches(ctx: DrainBatchesCtx<'_>) -> Result<(), String> {
    let DrainBatchesCtx {
        tokenizer,
        input,
        builder,
        atoms,
        patch_batches,
        fixture_name,
        label,
        saw_eof_token,
    } = ctx;
    let mut patches = Vec::new();
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        patches.clear();
        let resolver = batch.resolver();
        let mut sink = VecPatchSink(&mut patches);
        for token in batch.iter() {
            if matches!(token, html::html5::Token::Eof) {
                *saw_eof_token = true;
            }
            match builder.push_token(token, atoms, &resolver, &mut sink) {
                Ok(TreeBuilderStepResult::Continue) => {}
                Ok(TreeBuilderStepResult::Suspend(reason)) => {
                    return Err(format!(
                        "tree builder suspended in fixture '{}' [{}] reason={reason:?}",
                        fixture_name, label
                    ));
                }
                Err(err) => {
                    return Err(format!(
                        "tree builder error in fixture '{}' [{}] error={err:?}",
                        fixture_name, label
                    ));
                }
            }
        }
        if !patches.is_empty() {
            patch_batches.push(std::mem::take(&mut patches));
        }
    }
    Ok(())
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
        ("finish", TokenizeResult::EmittedEof) => Ok(()),
        ("finish", other) => {
            let plan = plan_label.unwrap_or("<whole>");
            Err(format!(
                "finish must emit EOF in fixture '{}' [{plan}], got {other:?}",
                fixture.name
            ))
        }
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

fn format_patch_batches(batches: &[Vec<html::DomPatch>]) -> Vec<String> {
    let mut lines = Vec::new();
    for (batch_index, batch) in batches.iter().enumerate() {
        lines.push(format!("Batch index={batch_index} size={}", batch.len()));
        for patch in batch {
            lines.push(format_patch(patch));
        }
    }
    lines
}

fn format_patch(patch: &html::DomPatch) -> String {
    match patch {
        html::DomPatch::Clear => "Clear".to_string(),
        html::DomPatch::CreateDocument { key, doctype } => match doctype {
            Some(value) => {
                format!(
                    "CreateDocument key={} doctype=\"{}\"",
                    key.0,
                    escape_text(value)
                )
            }
            None => format!("CreateDocument key={} doctype=<none>", key.0),
        },
        html::DomPatch::CreateElement {
            key,
            name,
            attributes,
        } => {
            let attrs = format_attributes(attributes);
            format!(
                "CreateElement key={} name={} attrs=[{}]",
                key.0, name, attrs
            )
        }
        html::DomPatch::CreateText { key, text } => {
            format!("CreateText key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::CreateComment { key, text } => {
            format!("CreateComment key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::AppendChild { parent, child } => {
            format!("AppendChild parent={} child={}", parent.0, child.0)
        }
        html::DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => {
            format!(
                "InsertBefore parent={} child={} before={}",
                parent.0, child.0, before.0
            )
        }
        html::DomPatch::RemoveNode { key } => format!("RemoveNode key={}", key.0),
        html::DomPatch::SetAttributes { key, attributes } => {
            let attrs = format_attributes(attributes);
            format!("SetAttributes key={} attrs=[{}]", key.0, attrs)
        }
        html::DomPatch::SetText { key, text } => {
            format!("SetText key={} text=\"{}\"", key.0, escape_text(text))
        }
        html::DomPatch::AppendText { key, text } => {
            format!("AppendText key={} text=\"{}\"", key.0, escape_text(text))
        }
        other => panic!("unhandled DomPatch variant in golden formatter: {other:?}"),
    }
}

fn format_attributes(attributes: &[(std::sync::Arc<str>, Option<String>)]) -> String {
    if attributes.is_empty() {
        return String::new();
    }

    let mut sorted = attributes
        .iter()
        .map(|(name, value)| (name.as_ref(), value.as_deref()))
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));

    let mut out = String::new();
    for (index, (name, value)) in sorted.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(name);
        out.push('=');
        match value {
            Some(value) => {
                out.push('"');
                out.push_str(&escape_text(value));
                out.push('"');
            }
            None => out.push_str("<none>"),
        }
    }
    out
}
