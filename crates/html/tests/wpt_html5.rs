#![cfg(feature = "html5")]

use html::dom_snapshot::{DomSnapshot, DomSnapshotOptions};
use html::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::{BoundaryPolicy, ChunkPlan, shrink_chunk_plan_with_stats};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "common/mod.rs"]
mod support;
#[path = "common/token_snapshot.rs"]
mod token_snapshot;
mod wpt_manifest;

use support::diff_lines;
use wpt_manifest::{CaseKind, FixtureStatus, WptCase, load_manifest};

struct ExpectedDom {
    options: DomSnapshotOptions,
    lines: Vec<String>,
}

struct ExpectedTokens {
    lines: Vec<String>,
}

struct RunConfig {
    kind: Option<CaseKind>,
    filter: Option<String>,
    ids: Vec<String>,
    chunked: bool,
    fuzz_runs: usize,
    fuzz_seed: u64,
}

struct RunSummary {
    total: usize,
    passed: usize,
    failed: usize,
    xfailed: usize,
    xpass: usize,
    skipped: usize,
}

struct Failure {
    id: String,
    message: String,
}

fn err_kind(err: &str) -> &str {
    err.lines()
        .next()
        .and_then(|line| line.split(':').next())
        .unwrap_or(err)
}

fn ensure_utf8_plan(case_id: &str, plan: &ChunkPlan, plan_label: &str) -> Result<(), String> {
    match plan {
        ChunkPlan::Fixed { policy, .. }
        | ChunkPlan::Sizes { policy, .. }
        | ChunkPlan::Boundaries { policy, .. } => {
            if matches!(policy, BoundaryPolicy::ByteStream) {
                return Err(format!(
                    "byte-stream chunking is not supported for case '{}' [{plan_label}]",
                    case_id
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn wpt_html5() {
    let manifest_path = wpt_root().join("manifest.txt");
    let cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");
    let run_config = load_run_config();
    let cases = select_cases(cases, &run_config, &manifest_path);

    let mut summary = RunSummary {
        total: cases.len(),
        passed: 0,
        failed: 0,
        xfailed: 0,
        xpass: 0,
        skipped: 0,
    };
    let mut failures = Vec::new();

    for case in cases {
        if case.status == FixtureStatus::Skip {
            summary.skipped += 1;
            continue;
        }
        let input = fs::read_to_string(&case.path)
            .unwrap_or_else(|err| panic!("failed to read WPT input {:?}: {err}", case.path));
        let chunk_plans = if run_config.chunked {
            html::chunker::build_chunk_plans_utf8(
                &input,
                run_config.fuzz_runs,
                run_config.fuzz_seed,
            )
        } else {
            Vec::new()
        };
        match case.kind {
            CaseKind::Dom => {
                let expected = parse_dom_file(&case.expected);
                let options = expected.options;
                let mut failure = None::<String>;
                let mut whole_lines = None::<Vec<String>>;
                match run_tree_builder_whole(&input, options) {
                    Ok(lines) => {
                        if lines.as_slice() != expected.lines.as_slice() {
                            failure = Some(format!(
                                "WPT DOM mismatch for '{}' ({})\n{}\nexpected file: {:?}\ninput file: {:?}",
                                case.id,
                                case.path.display(),
                                diff_lines(&expected.lines, &lines),
                                case.expected,
                                case.path
                            ));
                        } else {
                            whole_lines = Some(lines);
                        }
                    }
                    Err(err) => {
                        failure = Some(format!(
                            "WPT case '{}' failed ({}) error: {err}",
                            case.id,
                            case.path.display()
                        ));
                    }
                }
                if failure.is_none() && run_config.chunked {
                    if chunk_plans.is_empty() {
                        failure = Some(format!(
                            "WPT case '{}' requested chunked mode but no chunk plans were generated",
                            case.id
                        ));
                    } else {
                        for plan in &chunk_plans {
                            if let Err(err) = ensure_utf8_plan(&case.id, &plan.plan, &plan.label) {
                                failure = Some(format!("{err}\ninput file: {:?}", case.path));
                                break;
                            }
                            match run_tree_builder_chunked(
                                &input,
                                options,
                                &plan.plan,
                                &plan.label,
                                &case.id,
                            ) {
                                Ok(lines) => {
                                    let (diff_basis, diff_target): (&str, &[String]) =
                                        match whole_lines.as_ref() {
                                            Some(whole) => ("whole-vs-chunked", whole.as_slice()),
                                            None => {
                                                ("expected-vs-chunked", expected.lines.as_slice())
                                            }
                                        };
                                    if lines.as_slice() != diff_target {
                                        let diff = diff_lines(diff_target, &lines);
                                        let shrink_predicate =
                                            |candidate: &ChunkPlan| match run_tree_builder_chunked(
                                                &input,
                                                options,
                                                candidate,
                                                "shrinking",
                                                &case.id,
                                            ) {
                                                Ok(candidate_lines) => {
                                                    candidate_lines.as_slice() != diff_target
                                                }
                                                Err(_) => true,
                                            };
                                        let (shrunk, stats) = shrink_chunk_plan_with_stats(
                                            &input,
                                            &plan.plan,
                                            shrink_predicate,
                                        );
                                        failure = Some(format!(
                                            "WPT DOM mismatch for '{}' ({}) [chunked: {}]\ndiff basis: {diff_basis}\nshrunk: {}\nshrink stats: {:?}\n{}\nexpected file: {:?}\ninput file: {:?}",
                                            case.id,
                                            case.path.display(),
                                            plan.label,
                                            shrunk,
                                            stats,
                                            diff,
                                            case.expected,
                                            case.path
                                        ));
                                        break;
                                    }
                                }
                                Err(err) => {
                                    let err_sig = err_kind(&err).to_string();
                                    let shrink_predicate =
                                        |candidate: &ChunkPlan| match run_tree_builder_chunked(
                                            &input,
                                            options,
                                            candidate,
                                            "shrinking",
                                            &case.id,
                                        ) {
                                            Ok(_) => false,
                                            Err(candidate_err) => {
                                                err_kind(&candidate_err).contains(&err_sig)
                                            }
                                        };
                                    let (shrunk, stats) = shrink_chunk_plan_with_stats(
                                        &input,
                                        &plan.plan,
                                        shrink_predicate,
                                    );
                                    failure = Some(format!(
                                        "WPT case '{}' failed ({}) [chunked: {}] error: {err}\nshrunk: {}\nshrink stats: {:?}\ninput file: {:?}",
                                        case.id,
                                        case.path.display(),
                                        plan.label,
                                        shrunk,
                                        stats,
                                        case.path
                                    ));
                                    break;
                                }
                            }
                        }
                    }
                }

                match case.status {
                    FixtureStatus::Active => match failure {
                        Some(message) => {
                            summary.failed += 1;
                            failures.push(Failure {
                                id: case.id,
                                message,
                            });
                        }
                        None => summary.passed += 1,
                    },
                    FixtureStatus::Xfail => match failure {
                        Some(_) => summary.xfailed += 1,
                        None => {
                            summary.xpass += 1;
                            let case_id = case.id.clone();
                            let message = format!(
                                "WPT case '{}' matched expected DOM but is marked xfail; reason: {}",
                                case_id,
                                case.reason.as_deref().unwrap_or("<missing reason>")
                            );
                            failures.push(Failure {
                                id: case_id,
                                message,
                            });
                        }
                    },
                    FixtureStatus::Skip => unreachable!("skip cases are filtered before execution"),
                }
            }
            CaseKind::Tokens => {
                let expected = parse_tokens_file(&case.expected);
                let mut failure = None::<String>;
                let mut whole_lines = None::<Vec<String>>;
                match run_tokenizer_whole(&input, &case.id) {
                    Ok(lines) => {
                        if lines.as_slice() != expected.lines.as_slice() {
                            failure = Some(format!(
                                "WPT token mismatch for '{}' ({})\n{}\nexpected file: {:?}\ninput file: {:?}",
                                case.id,
                                case.path.display(),
                                diff_lines(&expected.lines, &lines),
                                case.expected,
                                case.path
                            ));
                        } else {
                            whole_lines = Some(lines);
                        }
                    }
                    Err(err) => {
                        failure = Some(format!(
                            "WPT case '{}' failed ({}) error: {err}",
                            case.id,
                            case.path.display()
                        ));
                    }
                }
                if failure.is_none() && run_config.chunked {
                    if chunk_plans.is_empty() {
                        failure = Some(format!(
                            "WPT case '{}' requested chunked mode but no chunk plans were generated",
                            case.id
                        ));
                    } else {
                        for plan in &chunk_plans {
                            if let Err(err) = ensure_utf8_plan(&case.id, &plan.plan, &plan.label) {
                                failure = Some(format!("{err}\ninput file: {:?}", case.path));
                                break;
                            }
                            match run_tokenizer_chunked(&input, &case.id, &plan.plan, &plan.label) {
                                Ok(lines) => {
                                    let (diff_basis, diff_target): (&str, &[String]) =
                                        match whole_lines.as_ref() {
                                            Some(whole) => ("whole-vs-chunked", whole.as_slice()),
                                            None => {
                                                ("expected-vs-chunked", expected.lines.as_slice())
                                            }
                                        };
                                    if lines.as_slice() != diff_target {
                                        let diff = diff_lines(diff_target, &lines);
                                        let shrink_predicate =
                                            |candidate: &ChunkPlan| match run_tokenizer_chunked(
                                                &input,
                                                &case.id,
                                                candidate,
                                                "shrinking",
                                            ) {
                                                Ok(candidate_lines) => {
                                                    candidate_lines.as_slice() != diff_target
                                                }
                                                Err(_) => true,
                                            };
                                        let (shrunk, stats) = shrink_chunk_plan_with_stats(
                                            &input,
                                            &plan.plan,
                                            shrink_predicate,
                                        );
                                        failure = Some(format!(
                                            "WPT token mismatch for '{}' ({}) [chunked: {}]\ndiff basis: {diff_basis}\nshrunk: {}\nshrink stats: {:?}\n{}\nexpected file: {:?}\ninput file: {:?}",
                                            case.id,
                                            case.path.display(),
                                            plan.label,
                                            shrunk,
                                            stats,
                                            diff,
                                            case.expected,
                                            case.path
                                        ));
                                        break;
                                    }
                                }
                                Err(err) => {
                                    let err_sig = err_kind(&err).to_string();
                                    let shrink_predicate =
                                        |candidate: &ChunkPlan| match run_tokenizer_chunked(
                                            &input,
                                            &case.id,
                                            candidate,
                                            "shrinking",
                                        ) {
                                            Ok(_) => false,
                                            Err(candidate_err) => {
                                                err_kind(&candidate_err).contains(&err_sig)
                                            }
                                        };
                                    let (shrunk, stats) = shrink_chunk_plan_with_stats(
                                        &input,
                                        &plan.plan,
                                        shrink_predicate,
                                    );
                                    failure = Some(format!(
                                        "WPT case '{}' failed ({}) [chunked: {}] error: {err}\nshrunk: {}\nshrink stats: {:?}\ninput file: {:?}",
                                        case.id,
                                        case.path.display(),
                                        plan.label,
                                        shrunk,
                                        stats,
                                        case.path
                                    ));
                                    break;
                                }
                            }
                        }
                    }
                }

                match case.status {
                    FixtureStatus::Active => match failure {
                        Some(message) => {
                            summary.failed += 1;
                            failures.push(Failure {
                                id: case.id,
                                message,
                            });
                        }
                        None => summary.passed += 1,
                    },
                    FixtureStatus::Xfail => match failure {
                        Some(_) => summary.xfailed += 1,
                        None => {
                            summary.xpass += 1;
                            let case_id = case.id.clone();
                            let message = format!(
                                "WPT case '{}' matched expected tokens but is marked xfail; reason: {}",
                                case_id,
                                case.reason.as_deref().unwrap_or("<missing reason>")
                            );
                            failures.push(Failure {
                                id: case_id,
                                message,
                            });
                        }
                    },
                    FixtureStatus::Skip => unreachable!("skip cases are filtered before execution"),
                }
            }
        }
    }

    if !failures.is_empty() {
        let mut report = String::new();
        use std::fmt::Write;
        let _ = writeln!(
            &mut report,
            "WPT run summary: total={} passed={} failed={} xfailed={} xpass={} skipped={}",
            summary.total,
            summary.passed,
            summary.failed,
            summary.xfailed,
            summary.xpass,
            summary.skipped
        );
        let mut failing_ids = failures
            .iter()
            .map(|failure| failure.id.as_str())
            .collect::<Vec<_>>();
        failing_ids.sort_unstable();
        let failing_ids = failing_ids.join(", ");
        let _ = writeln!(&mut report, "failing ids: {failing_ids}");
        let _ = writeln!(&mut report, "failures:");
        for failure in &failures {
            let _ = writeln!(&mut report, "\n- {}:\n{}", failure.id, failure.message);
        }
        panic!("{report}");
    }
}

fn wpt_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("wpt")
}

fn load_run_config() -> RunConfig {
    let kind = match env::var("WPT_KIND").ok().as_deref() {
        Some("dom") => Some(CaseKind::Dom),
        Some("tokens") => Some(CaseKind::Tokens),
        Some("all") | Some("") | None => None,
        Some(other) => panic!("unsupported WPT_KIND '{other}'; expected 'dom', 'tokens', or 'all'"),
    };
    let filter = env::var("WPT_FILTER").ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let ids = env::var("WPT_IDS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let chunked = parse_env_bool("WPT_CHUNKED");
    let fuzz_runs_raw = env::var("WPT_FUZZ_RUNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok());
    let mut fuzz_runs = if chunked {
        fuzz_runs_raw.unwrap_or(4)
    } else {
        fuzz_runs_raw.unwrap_or(0)
    };
    if chunked && env::var("CI").is_ok() && fuzz_runs == 0 {
        fuzz_runs = 1;
    }
    let fuzz_seed = env::var("WPT_FUZZ_SEED")
        .ok()
        .and_then(|value| parse_u64(&value))
        .unwrap_or(0xC0FFEE);
    RunConfig {
        kind,
        filter,
        ids,
        chunked,
        fuzz_runs,
        fuzz_seed,
    }
}

fn parse_env_bool(key: &str) -> bool {
    match env::var(key).ok().as_deref() {
        Some("1") | Some("true") | Some("yes") | Some("on") => true,
        Some("0") | Some("false") | Some("no") | Some("off") | Some("") | None => false,
        Some(other) => panic!("unsupported {key} value '{other}'; use 1/0 or true/false"),
    }
}

fn parse_u64(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(hex) = trimmed.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else {
        trimmed.parse::<u64>().ok()
    }
}

fn select_cases(cases: Vec<WptCase>, run_config: &RunConfig, manifest_path: &Path) -> Vec<WptCase> {
    let mut selected = Vec::new();
    for case in cases {
        if let Some(kind) = run_config.kind
            && case.kind != kind
        {
            continue;
        }
        if !run_config.ids.is_empty() && !run_config.ids.iter().any(|id| id == &case.id) {
            continue;
        }
        if let Some(filter) = run_config.filter.as_deref() {
            let filter = filter.to_lowercase();
            let id = case.id.to_lowercase();
            let path = case.path.to_string_lossy().to_lowercase();
            let expected = case.expected.to_string_lossy().to_lowercase();
            if !id.contains(&filter) && !path.contains(&filter) && !expected.contains(&filter) {
                continue;
            }
        }
        selected.push(case);
    }

    let has_filters =
        run_config.kind.is_some() || run_config.filter.is_some() || !run_config.ids.is_empty();
    if has_filters && selected.is_empty() {
        panic!(
            "no WPT cases matched filters in {manifest_path:?} (WPT_KIND={:?}, WPT_FILTER={:?}, WPT_IDS={:?})",
            run_config.kind, run_config.filter, run_config.ids
        );
    }
    selected
}

fn parse_dom_file(path: &Path) -> ExpectedDom {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read expected DOM file {path:?}: {err}"));
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
    assert_eq!(format, "html5-dom-v1", "unsupported format in {path:?}");

    let options = DomSnapshotOptions {
        ignore_ids: header_bool(&headers, "ignore_ids", true, path),
        ignore_empty_style: header_bool(&headers, "ignore_empty_style", true, path),
    };

    if lines.is_empty() {
        panic!("expected DOM file {path:?} has no snapshot lines");
    }
    if !lines[0].starts_with("#document") {
        panic!("expected DOM file {path:?} must start with #document");
    }

    ExpectedDom { options, lines }
}

fn parse_tokens_file(path: &Path) -> ExpectedTokens {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read expected tokens file {path:?}: {err}"));
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
    assert_eq!(format, "html5-token-v1", "unsupported format in {path:?}");
    if headers.contains_key("status") || headers.contains_key("reason") {
        panic!(
            "status/reason headers are not supported in {path:?}; use manifest.txt as the source of truth"
        );
    }

    if lines.is_empty() {
        panic!("expected tokens file {path:?} has no token lines");
    }
    if lines.last().map(String::as_str) != Some("EOF") {
        panic!("expected tokens file {path:?} must end with EOF");
    }

    ExpectedTokens { lines }
}

fn header_bool(headers: &BTreeMap<String, String>, key: &str, default: bool, path: &Path) -> bool {
    match headers.get(key).map(|s| s.as_str()) {
        None => default,
        Some("true") => true,
        Some("false") => false,
        Some(other) => panic!("invalid boolean '{other}' for {key} in {path:?}"),
    }
}

fn run_tree_builder_whole(
    input_html: &str,
    options: DomSnapshotOptions,
) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;

    input.push_str(input_html);
    handle_tokenize_result(tokenizer.push_input(&mut input, &mut ctx), "push_input")?;
    drain_batches(
        &mut tokenizer,
        &mut input,
        &mut builder,
        &ctx,
        &mut patch_batches,
        &mut saw_eof_token,
    )?;

    handle_tokenize_result(tokenizer.finish(&input), "finish")?;
    drain_batches(
        &mut tokenizer,
        &mut input,
        &mut builder,
        &ctx,
        &mut patch_batches,
        &mut saw_eof_token,
    )?;
    if !saw_eof_token {
        return Err("expected EOF token but none was observed".to_string());
    }

    let dom = html::test_harness::materialize_patch_batches(&patch_batches)?;
    let snapshot = DomSnapshot::new(&dom, options);
    Ok(snapshot.as_lines().to_vec())
}

fn run_tree_builder_chunked(
    input_html: &str,
    options: DomSnapshotOptions,
    plan: &ChunkPlan,
    plan_label: &str,
    case_id: &str,
) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;
    let mut error = None::<String>;

    plan.for_each_chunk(input_html, |chunk| {
        if error.is_some() {
            return;
        }
        let chunk_str = std::str::from_utf8(chunk).unwrap_or_else(|_| {
            error = Some(format!(
                "chunk plan produced invalid UTF-8 boundary in case '{}' [{plan_label}]",
                case_id
            ));
            ""
        });
        if error.is_some() {
            return;
        }
        input.push_str(chunk_str);
        if let Err(err) =
            handle_tokenize_result(tokenizer.push_input(&mut input, &mut ctx), "push_input")
        {
            error = Some(format!("case '{}' [{plan_label}] error: {err}", case_id));
            return;
        }
        if let Err(err) = drain_batches(
            &mut tokenizer,
            &mut input,
            &mut builder,
            &ctx,
            &mut patch_batches,
            &mut saw_eof_token,
        ) {
            error = Some(format!("case '{}' [{plan_label}] error: {err}", case_id));
        }
    });

    if let Some(err) = error {
        return Err(err);
    }

    handle_tokenize_result(tokenizer.finish(&input), "finish")?;
    drain_batches(
        &mut tokenizer,
        &mut input,
        &mut builder,
        &ctx,
        &mut patch_batches,
        &mut saw_eof_token,
    )?;
    if !saw_eof_token {
        return Err(format!(
            "expected EOF token but none was observed (case '{}' [{plan_label}])",
            case_id
        ));
    }

    let dom = html::test_harness::materialize_patch_batches(&patch_batches)?;
    let snapshot = DomSnapshot::new(&dom, options);
    Ok(snapshot.as_lines().to_vec())
}

fn run_tokenizer_whole(input_html: &str, case_id: &str) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut input = Input::new();
    let mut saw_eof_token = false;
    input.push_str(input_html);
    handle_tokenize_result(tokenizer.push_input(&mut input, &mut ctx), "push_input")?;
    let mut out = Vec::new();
    let mut index = 0usize;
    let context = token_snapshot::TokenFormatContext {
        case_id,
        mode: "whole",
    };
    drain_tokens(
        &mut out,
        &mut tokenizer,
        &mut input,
        &ctx,
        &context,
        &mut index,
        &mut saw_eof_token,
    )?;
    handle_tokenize_result(tokenizer.finish(&input), "finish")?;
    drain_tokens(
        &mut out,
        &mut tokenizer,
        &mut input,
        &ctx,
        &context,
        &mut index,
        &mut saw_eof_token,
    )?;
    if !saw_eof_token {
        return Err(format!(
            "expected EOF token but none was observed (case '{}' [whole])",
            case_id
        ));
    }
    Ok(out)
}

fn run_tokenizer_chunked(
    input_html: &str,
    case_id: &str,
    plan: &ChunkPlan,
    plan_label: &str,
) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();
    let mut index = 0usize;
    let mut saw_eof_token = false;
    let context = token_snapshot::TokenFormatContext {
        case_id,
        mode: plan_label,
    };
    let mut error = None::<String>;

    plan.for_each_chunk(input_html, |chunk| {
        if error.is_some() {
            return;
        }
        let chunk_str = std::str::from_utf8(chunk).unwrap_or_else(|_| {
            error = Some(format!(
                "chunk plan produced invalid UTF-8 boundary in case '{}' [{plan_label}]",
                case_id
            ));
            ""
        });
        if error.is_some() {
            return;
        }
        input.push_str(chunk_str);
        if let Err(err) =
            handle_tokenize_result(tokenizer.push_input(&mut input, &mut ctx), "push_input")
        {
            error = Some(format!("case '{}' [{plan_label}] error: {err}", case_id));
            return;
        }
        if let Err(err) = drain_tokens(
            &mut out,
            &mut tokenizer,
            &mut input,
            &ctx,
            &context,
            &mut index,
            &mut saw_eof_token,
        ) {
            error = Some(format!("case '{}' [{plan_label}] error: {err}", case_id));
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    handle_tokenize_result(tokenizer.finish(&input), "finish")?;
    drain_tokens(
        &mut out,
        &mut tokenizer,
        &mut input,
        &ctx,
        &context,
        &mut index,
        &mut saw_eof_token,
    )?;
    if !saw_eof_token {
        return Err(format!(
            "expected EOF token but none was observed (case '{}' [{plan_label}])",
            case_id
        ));
    }
    Ok(out)
}

fn drain_tokens(
    out: &mut Vec<String>,
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
    context: &token_snapshot::TokenFormatContext<'_>,
    index: &mut usize,
    saw_eof_token: &mut bool,
) -> Result<(), String> {
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        out.extend(token_snapshot::format_tokens(
            batch.tokens(),
            &resolver,
            ctx,
            context,
            index,
            Some(saw_eof_token),
        )?);
    }
    Ok(())
}

fn drain_batches(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    builder: &mut Html5TreeBuilder,
    ctx: &DocumentParseContext,
    patch_batches: &mut Vec<Vec<html::DomPatch>>,
    saw_eof_token: &mut bool,
) -> Result<(), String> {
    let mut patches = Vec::new();
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        patches.clear();
        let resolver = batch.resolver();
        let atoms = &ctx.atoms;
        let mut sink = html::html5::tree_builder::VecPatchSink(&mut patches);
        for token in batch.iter() {
            if matches!(token, html::html5::Token::Eof) {
                *saw_eof_token = true;
            }
            match builder.push_token(token, atoms, &resolver, &mut sink) {
                Ok(TreeBuilderStepResult::Continue) => {}
                Ok(TreeBuilderStepResult::Suspend(reason)) => {
                    return Err(format!("tree builder suspended: {reason:?}"));
                }
                Err(err) => {
                    return Err(format!("tree builder error: {err:?}"));
                }
            }
        }
        if !patches.is_empty() {
            patch_batches.push(std::mem::take(&mut patches));
        }
    }
    Ok(())
}

fn handle_tokenize_result(result: TokenizeResult, stage: &str) -> Result<(), String> {
    match (stage, result) {
        ("push_input", TokenizeResult::EmittedEof) => {
            Err("unexpected EOF while pushing input".to_string())
        }
        ("finish", TokenizeResult::EmittedEof) => Ok(()),
        ("finish", other) => Err(format!("finish must emit EOF, got {other:?}")),
        ("push_input", TokenizeResult::NeedMoreInput | TokenizeResult::Progress) => Ok(()),
        _ => Err(format!(
            "unexpected tokenizer state stage={stage} result={result:?}"
        )),
    }
}
