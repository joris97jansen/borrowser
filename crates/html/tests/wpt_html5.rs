#![cfg(feature = "html5")]

use html::dom_snapshot::DomSnapshotOptions;
use html::test_harness::{ChunkPlan, shrink_chunk_plan_with_stats};
use html_test_support::diff_lines;
use html_test_support::wpt_expected::{parse_expected_dom, parse_expected_tokens};
use html_test_support::wpt_tokenizer::{
    TokenizerSkipStatus, applied_skip_override, ensure_utf8_plan as ensure_utf8_tokenizer_plan,
    load_tokenizer_skip_overrides, parse_env_bool, parse_u64, run_tokenizer_chunked,
    run_tokenizer_whole, validate_skip_override_ids,
};
use html_test_support::wpt_tree_builder::{
    TreeBuilderSkipStatus, applied_skip_override as applied_tree_builder_skip_override,
    ensure_utf8_plan as ensure_utf8_tree_builder_plan, load_tree_builder_skip_overrides,
    run_tree_builder_chunked, run_tree_builder_whole,
    validate_skip_override_ids as validate_tree_builder_skip_ids,
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

fn apply_case_outcome(
    summary: &mut RunSummary,
    failures: &mut Vec<Failure>,
    case: &WptCase,
    failure: Option<String>,
    expected_label: &str,
) {
    match case.status {
        FixtureStatus::Active => match failure {
            Some(message) => {
                summary.failed += 1;
                failures.push(Failure {
                    id: case.id.clone(),
                    message,
                });
            }
            None => summary.passed += 1,
        },
        FixtureStatus::Xfail => match failure {
            Some(_) => summary.xfailed += 1,
            None => {
                summary.xpass += 1;
                failures.push(Failure {
                    id: case.id.clone(),
                    message: format!(
                        "WPT case '{}' matched expected {expected_label} but is marked xfail; reason: {}",
                        case.id,
                        case.reason.as_deref().unwrap_or("<missing reason>")
                    ),
                });
            }
        },
        FixtureStatus::Skip => unreachable!("skip cases are filtered before execution"),
    }
}

struct LineCaseExecConfig<'a> {
    run_config: &'a RunConfig,
    mismatch_label: &'a str,
    case_label: &'a str,
    ensure_plan: fn(&str, &ChunkPlan, &str) -> Result<(), String>,
}

fn run_case_whole_and_chunked<FWhole, FChunked>(
    case: &WptCase,
    input: &str,
    expected_lines: &[String],
    config: LineCaseExecConfig<'_>,
    run_whole: FWhole,
    run_chunked: FChunked,
) -> Option<String>
where
    FWhole: Fn() -> Result<Vec<String>, String>,
    FChunked: Fn(&ChunkPlan, &str) -> Result<Vec<String>, String>,
{
    let chunk_plans = if config.run_config.chunked {
        html::chunker::build_chunk_plans_utf8(
            input,
            config.run_config.fuzz_runs,
            config.run_config.fuzz_seed,
        )
    } else {
        Vec::new()
    };

    let mut failure = None::<String>;
    let mut whole_lines = None::<Vec<String>>;

    match run_whole() {
        Ok(lines) => {
            if lines.as_slice() != expected_lines {
                failure = Some(format!(
                    "{} for '{}' ({})\n{}\nexpected file: {:?}\ninput file: {:?}",
                    config.mismatch_label,
                    case.id,
                    case.path.display(),
                    diff_lines(expected_lines, &lines),
                    case.expected,
                    case.path
                ));
            } else {
                whole_lines = Some(lines);
            }
        }
        Err(err) => {
            failure = Some(format!(
                "{} '{}' failed ({}) error: {err}",
                config.case_label,
                case.id,
                case.path.display()
            ));
        }
    }

    if config.run_config.chunked && (failure.is_none() || config.run_config.chunked_force) {
        if chunk_plans.is_empty() {
            failure = Some(format!(
                "{} '{}' requested chunked mode but no chunk plans were generated",
                config.case_label, case.id
            ));
        } else {
            for plan in &chunk_plans {
                if let Err(err) = (config.ensure_plan)(&case.id, &plan.plan, &plan.label) {
                    failure = Some(format!("{err}\ninput file: {:?}", case.path));
                    break;
                }
                match run_chunked(&plan.plan, &plan.label) {
                    Ok(lines) => {
                        let (diff_basis, diff_target): (&str, &[String]) =
                            match whole_lines.as_ref() {
                                Some(whole) => ("whole-vs-chunked", whole.as_slice()),
                                None => ("expected-vs-chunked", expected_lines),
                            };
                        if lines.as_slice() != diff_target {
                            let diff = diff_lines(diff_target, &lines);
                            let shrink_predicate =
                                |candidate: &ChunkPlan| match run_chunked(candidate, "shrinking") {
                                    Ok(candidate_lines) => {
                                        candidate_lines.as_slice() != diff_target
                                    }
                                    Err(_) => true,
                                };
                            let (shrunk, stats) =
                                shrink_chunk_plan_with_stats(input, &plan.plan, shrink_predicate);
                            failure = Some(format!(
                                "{} for '{}' ({}) [chunked: {}]\ndiff basis: {diff_basis}\nshrunk: {}\nshrink stats: {:?}\n{}\nexpected file: {:?}\ninput file: {:?}",
                                config.mismatch_label,
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
                            |candidate: &ChunkPlan| match run_chunked(candidate, "shrinking") {
                                Ok(_) => false,
                                Err(candidate_err) => err_kind(&candidate_err).contains(&err_sig),
                            };
                        let (shrunk, stats) =
                            shrink_chunk_plan_with_stats(input, &plan.plan, shrink_predicate);
                        failure = Some(format!(
                            "{} '{}' failed ({}) [chunked: {}] error: {err}\nshrunk: {}\nshrink stats: {:?}\ninput file: {:?}",
                            config.case_label,
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

    failure
}

#[test]
fn wpt_html5() {
    let manifest_path = wpt_root().join("manifest.txt");
    let mut cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");
    apply_tokenizer_skip_overrides(&mut cases, &manifest_path);
    apply_tree_builder_skip_overrides(&mut cases, &manifest_path);
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
        match case.kind {
            CaseKind::Dom => {
                let expected = parse_expected_dom(&case.expected);
                let options = DomSnapshotOptions {
                    ignore_ids: expected.ignore_ids,
                    ignore_empty_style: expected.ignore_empty_style,
                };
                let failure = run_case_whole_and_chunked(
                    &case,
                    &input,
                    &expected.lines,
                    LineCaseExecConfig {
                        run_config: &run_config,
                        mismatch_label: "WPT DOM mismatch",
                        case_label: "WPT DOM case",
                        ensure_plan: ensure_utf8_tree_builder_plan,
                    },
                    || run_tree_builder_whole(&input, &case.id, options),
                    |plan, label| run_tree_builder_chunked(&input, &case.id, plan, label, options),
                );
                apply_case_outcome(&mut summary, &mut failures, &case, failure, "DOM snapshot");
            }
            CaseKind::Tokens => {
                let expected_lines = parse_expected_tokens(&case.expected);
                let failure = run_case_whole_and_chunked(
                    &case,
                    &input,
                    &expected_lines,
                    LineCaseExecConfig {
                        run_config: &run_config,
                        mismatch_label: "WPT token mismatch",
                        case_label: "WPT token case",
                        ensure_plan: ensure_utf8_tokenizer_plan,
                    },
                    || run_tokenizer_whole(&input, &case.id),
                    |plan, label| run_tokenizer_chunked(&input, &case.id, plan, label),
                );
                apply_case_outcome(&mut summary, &mut failures, &case, failure, "token stream");
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

fn apply_tree_builder_skip_overrides(cases: &mut [WptCase], manifest_path: &Path) {
    let overrides = load_tree_builder_skip_overrides(&wpt_root());
    let dom_case_ids = cases
        .iter()
        .filter(|case| case.kind == CaseKind::Dom)
        .map(|case| case.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    validate_tree_builder_skip_ids(&overrides, &dom_case_ids, manifest_path);

    for case in cases.iter_mut().filter(|case| case.kind == CaseKind::Dom) {
        if let Some((override_status, override_reason)) =
            applied_tree_builder_skip_override(&case.id, &overrides)
        {
            case.status = match override_status {
                TreeBuilderSkipStatus::Skip => FixtureStatus::Skip,
                TreeBuilderSkipStatus::Xfail => FixtureStatus::Xfail,
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
            Some(trimmed.to_ascii_lowercase())
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
            let id = case.id.to_lowercase();
            let path = case.path.to_string_lossy().to_lowercase();
            let expected = case.expected.to_string_lossy().to_lowercase();
            if !id.contains(filter) && !path.contains(filter) && !expected.contains(filter) {
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
