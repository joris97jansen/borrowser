#![cfg(feature = "html5")]

use html::dom_snapshot::{DomSnapshot, DomSnapshotOptions};
use html::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::{ChunkPlan, shrink_chunk_plan_with_stats};
use html_test_support::diff_lines;
use html_test_support::wpt_expected::{parse_expected_dom, parse_expected_tokens};
use html_test_support::wpt_tokenizer::{
    TokenizerSkipStatus, applied_skip_override, ensure_utf8_plan, load_tokenizer_skip_overrides,
    parse_env_bool, parse_u64, run_tokenizer_chunked, run_tokenizer_whole,
    validate_skip_override_ids,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod wpt_manifest;

use wpt_manifest::{CaseKind, FixtureStatus, WptCase, load_manifest};

struct RunConfig {
    kind: Option<CaseKind>,
    filter: Option<String>,
    ids: Vec<String>,
    chunked: bool,
    chunked_force: bool,
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

#[test]
fn wpt_html5() {
    let manifest_path = wpt_root().join("manifest.txt");
    let mut cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");
    apply_tokenizer_skip_overrides(&mut cases, &manifest_path);
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
                let expected = parse_expected_dom(&case.expected);
                let options = DomSnapshotOptions {
                    ignore_ids: expected.ignore_ids,
                    ignore_empty_style: expected.ignore_empty_style,
                };
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
                if run_config.chunked && (failure.is_none() || run_config.chunked_force) {
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
                let expected_lines = parse_expected_tokens(&case.expected);
                let mut failure = None::<String>;
                let mut whole_lines = None::<Vec<String>>;
                match run_tokenizer_whole(&input, &case.id) {
                    Ok(lines) => {
                        if lines.as_slice() != expected_lines.as_slice() {
                            failure = Some(format!(
                                "WPT token mismatch for '{}' ({})\n{}\nexpected file: {:?}\ninput file: {:?}",
                                case.id,
                                case.path.display(),
                                diff_lines(&expected_lines, &lines),
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
                if run_config.chunked && (failure.is_none() || run_config.chunked_force) {
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
                                                ("expected-vs-chunked", expected_lines.as_slice())
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

fn apply_tokenizer_skip_overrides(cases: &mut [WptCase], manifest_path: &Path) {
    let overrides = load_tokenizer_skip_overrides(&wpt_root());
    let token_case_ids = cases
        .iter()
        .filter(|case| case.kind == CaseKind::Tokens)
        .map(|case| case.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    validate_skip_override_ids(&overrides, &token_case_ids, manifest_path);

    for case in cases
        .iter_mut()
        .filter(|case| case.kind == CaseKind::Tokens)
    {
        if let Some((override_status, override_reason)) =
            applied_skip_override(&case.id, &overrides)
        {
            case.status = match override_status {
                TokenizerSkipStatus::Skip => FixtureStatus::Skip,
                TokenizerSkipStatus::Xfail => FixtureStatus::Xfail,
            };
            case.reason = Some(override_reason);
        }
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
    let chunked_force = parse_env_bool("WPT_CHUNKED_FORCE");
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
        chunked_force,
        fuzz_runs,
        fuzz_seed,
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

fn run_tree_builder_whole(
    input_html: &str,
    options: DomSnapshotOptions,
) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx)
        .map_err(|err| format!("failed to init tree builder: {err:?}"))?;
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
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx)
        .map_err(|err| format!("failed to init tree builder: {err:?}"))?;
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
