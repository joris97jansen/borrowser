use crate::wpt_manifest::{DiffKind, FixtureStatus, WptCase, load_manifest};
use html::dom_snapshot::DomSnapshotOptions;
use html::test_harness::shrink_chunk_plan_with_stats;
use html_test_support::diff_lines;
use html_test_support::wpt_tokenizer::{
    TokenizerSkipStatus, applied_skip_override as applied_tokenizer_skip_override,
    ensure_utf8_plan as ensure_utf8_tokenizer_plan, load_tokenizer_skip_overrides, parse_u64,
    run_tokenizer_chunked, run_tokenizer_whole,
    validate_skip_override_ids as validate_tokenizer_skip_ids,
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

#[derive(Clone, Debug)]
struct DiffFailure {
    id: String,
    message: String,
}

#[derive(Clone, Debug)]
struct DiffSummary {
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
}

#[derive(Clone, Debug)]
struct RunConfig {
    filter: Option<String>,
    ids: Vec<String>,
    limit: Option<usize>,
    fuzz_runs: usize,
    fuzz_seed: u64,
}

#[test]
fn diff_html5() {
    let manifest_path = wpt_root().join("manifest.txt");
    let mut cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");
    apply_tokenizer_skip_overrides(&mut cases, &manifest_path);
    apply_tree_builder_skip_overrides(&mut cases, &manifest_path);
    normalize_statuses_for_parity(&mut cases);

    let mode = diff_mode();
    let run_config = load_run_config();
    let cases = select_cases(cases, &run_config);
    let mut summary = DiffSummary {
        total: cases.len(),
        passed: 0,
        failed: 0,
        skipped: 0,
    };
    let mut failures = Vec::new();

    for case in cases {
        if case.status == FixtureStatus::Skip {
            summary.skipped += 1;
            continue;
        }
        let case_mode = case.diff.unwrap_or(mode);
        if case_mode == DiffKind::Skip {
            summary.skipped += 1;
            continue;
        }
        let input = fs::read_to_string(&case.path)
            .unwrap_or_else(|err| panic!("failed to read WPT input {:?}: {err}", case.path));
        match run_diff_case(&case, &input, case_mode, &run_config) {
            Ok(()) => summary.passed += 1,
            Err(message) => {
                summary.failed += 1;
                failures.push(DiffFailure {
                    id: case.id,
                    message,
                });
            }
        }
    }

    if !failures.is_empty() {
        let mut report = String::new();
        use std::fmt::Write;
        let _ = writeln!(
            &mut report,
            "HTML5 parity summary: total={} passed={} failed={} skipped={}",
            summary.total, summary.passed, summary.failed, summary.skipped
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

fn run_diff_case(
    case: &WptCase,
    input: &str,
    mode: DiffKind,
    run_config: &RunConfig,
) -> Result<(), String> {
    match mode {
        DiffKind::Tokens => diff_tokens(case, input, run_config),
        DiffKind::Dom => diff_dom(case, input, run_config),
        DiffKind::Both => {
            diff_tokens(case, input, run_config)?;
            diff_dom(case, input, run_config)?;
            Ok(())
        }
        DiffKind::Skip => Ok(()),
    }
}

fn diff_tokens(case: &WptCase, input: &str, run_config: &RunConfig) -> Result<(), String> {
    let whole = run_tokenizer_whole(input, &case.id).map_err(|err| {
        format!(
            "token parity baseline failed for '{}' ({}) error: {err}",
            case.id,
            case.path.display()
        )
    })?;
    let plans =
        html::chunker::build_chunk_plans_utf8(input, run_config.fuzz_runs, run_config.fuzz_seed);
    if plans.is_empty() {
        return Err(format!(
            "token parity generated no chunk plans for '{}' ({})",
            case.id,
            case.path.display()
        ));
    }

    for plan in &plans {
        ensure_utf8_tokenizer_plan(&case.id, &plan.plan, &plan.label)?;
        let chunked =
            run_tokenizer_chunked(input, &case.id, &plan.plan, &plan.label).map_err(|err| {
                format!(
                    "token parity chunked run failed for '{}' ({}) [chunked: {}] error: {err}",
                    case.id,
                    case.path.display(),
                    plan.label
                )
            })?;
        if chunked != whole {
            let diff = diff_lines(&whole, &chunked);
            let shrink_predicate =
                |candidate: &html::test_harness::ChunkPlan| match run_tokenizer_chunked(
                    input,
                    &case.id,
                    candidate,
                    "shrinking",
                ) {
                    Ok(candidate_lines) => candidate_lines != whole,
                    Err(_) => true,
                };
            let (shrunk, stats) = shrink_chunk_plan_with_stats(input, &plan.plan, shrink_predicate);
            return Err(format!(
                "token parity diff for '{}' ({}) [chunked: {}]\nshrunk: {}\nshrink stats: {:?}\n{}",
                case.id,
                case.path.display(),
                plan.label,
                shrunk,
                stats,
                diff
            ));
        }
    }

    Ok(())
}

fn diff_dom(case: &WptCase, input: &str, run_config: &RunConfig) -> Result<(), String> {
    let options = DomSnapshotOptions::default();
    let whole = run_tree_builder_whole(input, &case.id, options).map_err(|err| {
        format!(
            "DOM parity baseline failed for '{}' ({}) error: {err}",
            case.id,
            case.path.display()
        )
    })?;
    let plans =
        html::chunker::build_chunk_plans_utf8(input, run_config.fuzz_runs, run_config.fuzz_seed);
    if plans.is_empty() {
        return Err(format!(
            "DOM parity generated no chunk plans for '{}' ({})",
            case.id,
            case.path.display()
        ));
    }

    for plan in &plans {
        ensure_utf8_tree_builder_plan(&case.id, &plan.plan, &plan.label)?;
        let chunked = run_tree_builder_chunked(input, &case.id, &plan.plan, &plan.label, options)
            .map_err(|err| {
            format!(
                "DOM parity chunked run failed for '{}' ({}) [chunked: {}] error: {err}",
                case.id,
                case.path.display(),
                plan.label
            )
        })?;
        if chunked != whole {
            let diff = diff_lines(&whole, &chunked);
            let shrink_predicate =
                |candidate: &html::test_harness::ChunkPlan| match run_tree_builder_chunked(
                    input,
                    &case.id,
                    candidate,
                    "shrinking",
                    options,
                ) {
                    Ok(candidate_lines) => candidate_lines != whole,
                    Err(_) => true,
                };
            let (shrunk, stats) = shrink_chunk_plan_with_stats(input, &plan.plan, shrink_predicate);
            return Err(format!(
                "DOM parity diff for '{}' ({}) [chunked: {}]\nshrunk: {}\nshrink stats: {:?}\n{}",
                case.id,
                case.path.display(),
                plan.label,
                shrunk,
                stats,
                diff
            ));
        }
    }

    Ok(())
}

fn wpt_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("wpt")
}

fn apply_tokenizer_skip_overrides(cases: &mut [WptCase], manifest_path: &Path) {
    let overrides = load_tokenizer_skip_overrides(&wpt_root());
    let token_case_ids = cases
        .iter()
        .filter(|case| case.kind == crate::wpt_manifest::CaseKind::Tokens)
        .map(|case| case.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    validate_tokenizer_skip_ids(&overrides, &token_case_ids, manifest_path);

    for case in cases
        .iter_mut()
        .filter(|case| case.kind == crate::wpt_manifest::CaseKind::Tokens)
    {
        if let Some((override_status, override_reason)) =
            applied_tokenizer_skip_override(&case.id, &overrides)
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
        .filter(|case| case.kind == crate::wpt_manifest::CaseKind::Dom)
        .map(|case| case.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    validate_tree_builder_skip_ids(&overrides, &dom_case_ids, manifest_path);

    for case in cases
        .iter_mut()
        .filter(|case| case.kind == crate::wpt_manifest::CaseKind::Dom)
    {
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

fn normalize_statuses_for_parity(cases: &mut [WptCase]) {
    for case in cases {
        if case.status == FixtureStatus::Xfail {
            case.status = FixtureStatus::Active;
        }
    }
}

fn diff_mode() -> DiffKind {
    match env::var("DIFF_MODE").ok().as_deref() {
        Some("dom") => DiffKind::Dom,
        Some("both") => DiffKind::Both,
        Some("tokens") | Some("") | None => DiffKind::Tokens,
        Some(other) => panic!("unsupported DIFF_MODE '{other}'; expected tokens|dom|both"),
    }
}

fn load_run_config() -> RunConfig {
    let filter = env::var("DIFF_FILTER").ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let ids = env::var("DIFF_IDS")
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
    let limit = env::var("DIFF_LIMIT")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok());
    let fuzz_runs = env::var("DIFF_FUZZ_RUNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let fuzz_seed = env::var("DIFF_FUZZ_SEED")
        .ok()
        .and_then(|value| parse_u64(&value))
        .unwrap_or(0xD1FF_5EED);

    RunConfig {
        filter,
        ids,
        limit,
        fuzz_runs,
        fuzz_seed,
    }
}

fn select_cases(cases: Vec<WptCase>, run_config: &RunConfig) -> Vec<WptCase> {
    let filter_lower = run_config.filter.as_ref().map(|value| value.to_lowercase());
    let mut selected = Vec::new();
    for case in cases {
        if !run_config.ids.is_empty() && !run_config.ids.iter().any(|id| id == &case.id) {
            continue;
        }
        if let Some(filter) = filter_lower.as_deref() {
            let id = case.id.to_lowercase();
            let path = case.path.to_string_lossy().to_lowercase();
            if !id.contains(filter) && !path.contains(filter) {
                continue;
            }
        }
        selected.push(case);
        if let Some(limit) = run_config.limit
            && selected.len() >= limit
        {
            break;
        }
    }

    let has_filters = filter_lower.is_some() || !run_config.ids.is_empty();
    if has_filters && selected.is_empty() {
        panic!(
            "no diff cases matched filters (DIFF_FILTER={:?}, DIFF_IDS={:?})",
            run_config.filter, run_config.ids
        );
    }
    selected
}
