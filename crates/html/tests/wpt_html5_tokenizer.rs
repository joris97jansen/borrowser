#![cfg(feature = "html5")]

use html::test_harness::{ChunkPlan, shrink_chunk_plan_with_stats};
use html_test_support::diff_lines;
use html_test_support::wpt_expected::parse_expected_tokens;
use html_test_support::wpt_tokenizer::{
    TokenizerSkipOverride, TokenizerSkipStatus, applied_skip_override, ensure_utf8_plan,
    load_tokenizer_skip_overrides, parse_env_bool, parse_u64, run_tokenizer_chunked,
    run_tokenizer_whole, validate_skip_override_ids,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod wpt_manifest;

use wpt_manifest::{CaseKind, FixtureStatus, WptCase, load_manifest};

#[derive(Clone, Debug)]
struct RunConfig {
    filter: Option<String>,
    ids: BTreeSet<String>,
    chunked: bool,
    chunked_force: bool,
    fuzz_runs: usize,
    fuzz_seed: u64,
}

#[derive(Clone, Debug)]
struct Failure {
    id: String,
    message: String,
}

#[test]
fn wpt_html5_tokenizer_slice() {
    let manifest_path = wpt_root().join("manifest.txt");
    let cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");

    let run_config = load_run_config();
    let skip_overrides = load_tokenizer_skip_overrides(&wpt_root());
    let token_case_ids = cases
        .iter()
        .filter(|case| case.kind == CaseKind::Tokens)
        .map(|case| case.id.clone())
        .collect::<BTreeSet<_>>();
    validate_skip_override_ids(&skip_overrides, &token_case_ids, &manifest_path);
    let cases = select_tokenizer_cases(cases, &run_config, &skip_overrides, &manifest_path);
    assert!(
        !cases.is_empty(),
        "no tokenizer WPT cases selected after filters and skips"
    );

    let mut failures = Vec::<Failure>::new();
    let mut total = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut xfailed = 0usize;
    let mut xpass = 0usize;

    for case in cases {
        total += 1;
        if case.status == FixtureStatus::Skip {
            skipped += 1;
            continue;
        }

        let input = fs::read_to_string(&case.path)
            .unwrap_or_else(|err| panic!("failed to read WPT input {:?}: {err}", case.path));
        let expected_lines = parse_expected_tokens(&case.expected);
        let chunk_plans = if run_config.chunked {
            html::chunker::build_chunk_plans_utf8(
                &input,
                run_config.fuzz_runs,
                run_config.fuzz_seed,
            )
        } else {
            Vec::new()
        };

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
                    "WPT tokenizer case '{}' failed ({}) error: {err}",
                    case.id,
                    case.path.display()
                ));
            }
        }

        if run_config.chunked && (failure.is_none() || run_config.chunked_force) {
            if chunk_plans.is_empty() {
                failure = Some(format!(
                    "WPT tokenizer case '{}' requested chunked mode but no chunk plans were generated",
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
                                    None => ("expected-vs-chunked", expected_lines.as_slice()),
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
                            failure = Some(format!(
                                "WPT tokenizer case '{}' failed ({}) [chunked: {}] error: {err}\ninput file: {:?}",
                                case.id,
                                case.path.display(),
                                plan.label,
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
                    failed += 1;
                    failures.push(Failure {
                        id: case.id,
                        message,
                    });
                }
                None => passed += 1,
            },
            FixtureStatus::Xfail => match failure {
                Some(_) => xfailed += 1,
                None => {
                    xpass += 1;
                    failures.push(Failure {
                        id: case.id.clone(),
                        message: format!(
                            "WPT tokenizer case '{}' matched expected tokens but is marked xfail; reason: {}",
                            case.id,
                            case.reason.as_deref().unwrap_or("<missing reason>")
                        ),
                    });
                }
            },
            FixtureStatus::Skip => unreachable!("skip cases are filtered before execution"),
        }
    }

    if !failures.is_empty() {
        let mut report = String::new();
        use std::fmt::Write;
        let _ = writeln!(
            &mut report,
            "WPT tokenizer run summary: total={} passed={} failed={} xfailed={} xpass={} skipped={}",
            total, passed, failed, xfailed, xpass, skipped
        );
        let mut ids = failures
            .iter()
            .map(|failure| failure.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        let _ = writeln!(&mut report, "failing ids: {}", ids.join(", "));
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
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let chunked = parse_env_bool("WPT_CHUNKED");
    let chunked_force = parse_env_bool("WPT_CHUNKED_FORCE");
    let mut fuzz_runs = env::var("WPT_FUZZ_RUNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(if chunked { 4 } else { 0 });
    if chunked && env::var("CI").is_ok() && fuzz_runs == 0 {
        fuzz_runs = 1;
    }
    let fuzz_seed = env::var("WPT_FUZZ_SEED")
        .ok()
        .and_then(|value| parse_u64(&value))
        .unwrap_or(0xC0FFEE);

    RunConfig {
        filter,
        ids,
        chunked,
        chunked_force,
        fuzz_runs,
        fuzz_seed,
    }
}

fn select_tokenizer_cases(
    cases: Vec<WptCase>,
    run_config: &RunConfig,
    skip_overrides: &BTreeMap<String, TokenizerSkipOverride>,
    manifest_path: &Path,
) -> Vec<WptCase> {
    let mut selected = Vec::new();
    for mut case in cases {
        if case.kind != CaseKind::Tokens {
            continue;
        }

        if let Some((override_status, override_reason)) =
            applied_skip_override(&case.id, skip_overrides)
        {
            case.status = match override_status {
                TokenizerSkipStatus::Skip => FixtureStatus::Skip,
                TokenizerSkipStatus::Xfail => FixtureStatus::Xfail,
            };
            case.reason = Some(override_reason);
        }

        if let Some(filter) = run_config.filter.as_deref() {
            let id = case.id.to_ascii_lowercase();
            let path = case.path.to_string_lossy().to_ascii_lowercase();
            let expected = case.expected.to_string_lossy().to_ascii_lowercase();
            if !id.contains(filter) && !path.contains(filter) && !expected.contains(filter) {
                continue;
            }
        }

        if !run_config.ids.is_empty() && !run_config.ids.contains(&case.id) {
            continue;
        }

        selected.push(case);
    }

    let has_filters = run_config.filter.is_some() || !run_config.ids.is_empty();
    if has_filters && selected.is_empty() {
        panic!(
            "no tokenizer WPT cases matched filters in {manifest_path:?} (WPT_FILTER={:?}, WPT_IDS={:?})",
            run_config.filter, run_config.ids
        );
    }

    selected
}
