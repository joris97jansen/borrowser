use crate::wpt_manifest::{DiffKind, FixtureStatus, WptCase, load_manifest};
use html::dom_snapshot::{DomSnapshotOptions, compare_dom};
use html::{build_owned_dom, tokenize};
use html_test_support::diff_lines;
use html_test_support::wpt_tokenizer::{
    TokenizerSkipStatus, applied_skip_override, load_tokenizer_skip_overrides,
    validate_skip_override_ids,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod dom_diff_support;
mod token_diff_support;

use dom_diff_support::Html5DomDriver;
use token_diff_support::{
    Html5TokenDiffDriver, format_norm_tokens, html5_only_eof, normalize_simplified_tokens,
};

#[derive(Clone, Copy)]
pub(super) struct CaseContext<'a> {
    pub(super) id: &'a str,
    pub(super) path: &'a Path,
}

impl<'a> CaseContext<'a> {
    fn new(id: &'a str, path: &'a Path) -> Self {
        Self { id, path }
    }
}

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

#[test]
fn diff_html5() {
    let manifest_path = wpt_root().join("manifest.txt");
    let mut cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");
    apply_tokenizer_skip_overrides(&mut cases, &manifest_path);

    let mode = diff_mode();
    let strict = diff_strict();
    let cases = select_cases(cases);
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
        match run_diff_case(&case, &input, case_mode, strict) {
            Ok(()) => summary.passed += 1,
            Err(message) => {
                if message.starts_with("SKIP:") {
                    summary.skipped += 1;
                    continue;
                }
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
            "HTML diff summary: total={} passed={} failed={} skipped={}",
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

fn run_diff_case(case: &WptCase, input: &str, mode: DiffKind, strict: bool) -> Result<(), String> {
    match mode {
        DiffKind::Tokens => diff_tokens(case, input, strict),
        DiffKind::Dom => diff_dom(case, input, strict),
        DiffKind::Both => {
            diff_tokens(case, input, strict)?;
            diff_dom(case, input, strict)?;
            Ok(())
        }
        DiffKind::Skip => Ok(()),
    }
}

fn diff_tokens(case: &WptCase, input: &str, strict: bool) -> Result<(), String> {
    let case_ctx = CaseContext::new(&case.id, &case.path);
    let token_driver = Html5TokenDiffDriver::new(case_ctx, strict);
    let simplified = normalize_simplified_tokens(&tokenize(input));
    let html5 = token_driver.collect_normalized_html5_tokens(input)?;
    if html5_only_eof(&html5) && !html5_only_eof(&simplified) {
        return Err(format!(
            "SKIP: html5 tokenizer produced only EOF (unimplemented) for '{}' ({})",
            case.id,
            case.path.display()
        ));
    }
    if simplified != html5 {
        let simplified_lines = format_norm_tokens(&simplified);
        let html5_lines = format_norm_tokens(&html5);
        return Err(format!(
            "token diff for '{}' ({})\nmode: tokens\n{}\nsource: simplified vs html5",
            case.id,
            case.path.display(),
            diff_lines(&simplified_lines, &html5_lines)
        ));
    }
    Ok(())
}

fn diff_dom(case: &WptCase, input: &str, strict: bool) -> Result<(), String> {
    let case_ctx = CaseContext::new(&case.id, &case.path);
    let token_driver = Html5TokenDiffDriver::new(case_ctx, strict);
    let html5_tokens = token_driver.collect_normalized_html5_tokens(input)?;
    if html5_only_eof(&html5_tokens) && !input.is_empty() {
        return Err(format!(
            "SKIP: html5 tokenizer produced only EOF (unimplemented) for '{}' ({})",
            case.id,
            case.path.display()
        ));
    }

    let simplified_stream = tokenize(input);
    let simplified_dom = build_owned_dom(&simplified_stream);
    let html5_dom = Html5DomDriver::new(case_ctx).materialize_html5_dom_via_patches(input)?;
    compare_dom(&simplified_dom, &html5_dom, DomSnapshotOptions::default()).map_err(|err| {
        format!(
            "dom diff for '{}' ({})\nmode: dom\n{}\nsource: simplified vs html5",
            case.id,
            case.path.display(),
            err
        )
    })?;
    Ok(())
}

pub(super) fn validate_tokenize_result(
    result: html::html5::TokenizeResult,
    stage: &str,
) -> Result<(), String> {
    match (stage, result) {
        ("push_input", html::html5::TokenizeResult::EmittedEof) => {
            Err("unexpected EOF while pushing input".to_string())
        }
        ("finish", html::html5::TokenizeResult::EmittedEof) => Ok(()),
        ("finish", other) => Err(format!("finish must emit EOF, got {other:?}")),
        (
            "push_input",
            html::html5::TokenizeResult::NeedMoreInput | html::html5::TokenizeResult::Progress,
        ) => Ok(()),
        _ => Err(format!(
            "unexpected tokenizer state stage={stage} result={result:?}"
        )),
    }
}

pub(super) fn ensure_need_more_input_only_at_buffer_end(
    case: CaseContext<'_>,
    result: html::html5::TokenizeResult,
    consumed_before: u64,
    consumed_after: u64,
    buffered_len: usize,
) -> Result<(), String> {
    if matches!(result, html::html5::TokenizeResult::NeedMoreInput)
        && consumed_after < buffered_len as u64
    {
        return Err(format!(
            "harness assumption violated: tokenizer returned NeedMoreInput despite buffered data in '{}' at {:?} (result={result:?}, consumed={} buffered={} before={}); either tokenizer stalled or input abstraction changed (repro: set DIFF_IDS={})",
            case.id, case.path, consumed_after, buffered_len, consumed_before, case.id
        ));
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
    validate_skip_override_ids(&overrides, &token_case_ids, manifest_path);

    for case in cases
        .iter_mut()
        .filter(|case| case.kind == crate::wpt_manifest::CaseKind::Tokens)
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

fn diff_mode() -> DiffKind {
    match env::var("DIFF_MODE").ok().as_deref() {
        Some("dom") => DiffKind::Dom,
        Some("both") => DiffKind::Both,
        Some("tokens") | Some("") | None => DiffKind::Tokens,
        Some(other) => panic!("unsupported DIFF_MODE '{other}'; expected tokens|dom|both"),
    }
}

fn diff_strict() -> bool {
    match env::var("DIFF_STRICT").ok().as_deref() {
        Some("1") | Some("true") | Some("yes") | Some("on") => true,
        Some("0") | Some("false") | Some("no") | Some("off") | Some("") | None => false,
        Some(other) => panic!("unsupported DIFF_STRICT value '{other}'; use 1/0 or true/false"),
    }
}

fn select_cases(cases: Vec<WptCase>) -> Vec<WptCase> {
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

    let filter_lower = filter.as_ref().map(|value| value.to_lowercase());
    let mut selected = Vec::new();
    for case in cases {
        if !ids.is_empty() && !ids.iter().any(|id| id == &case.id) {
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
        if let Some(limit) = limit
            && selected.len() >= limit
        {
            break;
        }
    }

    let has_filters = filter_lower.is_some() || !ids.is_empty();
    if has_filters && selected.is_empty() {
        panic!(
            "no diff cases matched filters (DIFF_FILTER={:?}, DIFF_IDS={:?})",
            filter, ids
        );
    }
    selected
}
