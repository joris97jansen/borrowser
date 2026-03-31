use browser::dom_store::DomStore;
use core_types::{DomHandle, DomVersion};
use html::dom_snapshot::{DomSnapshotOptions, compare_dom};
use html::golden_corpus::{Expectation, fixtures};
use html::test_harness::{
    ChunkPlan, FuzzMode, deterministic_chunk_plans, random_chunk_plan, shrink_chunk_plan_with_stats,
};
use html::{DomPatch, HtmlParseOptions, HtmlParser};

const DEFAULT_BUDGET_CI: usize = 150;
const DEFAULT_BUDGET_LOCAL: usize = 600;
const DEFAULT_SEEDS_CI: usize = 25;
const DEFAULT_SEEDS_LOCAL: usize = 100;
const SEED_MIX: u64 = 0x9e3779b97f4a7c15;

#[test]
fn patch_stream_parity_golden_corpus() {
    let fixtures = fixtures();
    let fuzz_seeds = seed_count();
    let fuzz_budget = run_budget();
    let mustpass_count = fixtures
        .iter()
        .filter(|fixture| fixture.expectation == Expectation::MustPass)
        .count()
        .max(1);
    let per_fixture_budget = (fuzz_budget / mustpass_count).max(1);

    for (fixture_index, fixture) in fixtures.iter().enumerate() {
        if fixture.expectation != Expectation::MustPass {
            continue;
        }
        let input = fixture.input;
        let full_dom = parse_html_document(input);

        for plan in deterministic_chunk_plans(input) {
            run_parity_case(fixture.name, input, &full_dom, &plan, None, None);
        }

        let mut remaining = per_fixture_budget;
        let base_seed = 0x735f6c696d6974 ^ fixture_index as u64;
        for iter in 0..fuzz_seeds {
            if remaining == 0 {
                break;
            }
            let seed = base_seed ^ (iter as u64).wrapping_mul(SEED_MIX);
            let fuzz = random_chunk_plan(input, seed, FuzzMode::Mixed);
            run_parity_case(
                fixture.name,
                input,
                &full_dom,
                &fuzz.plan,
                Some(seed),
                Some(fuzz.summary.as_str()),
            );
            remaining = remaining.saturating_sub(1);
        }
    }
}

fn run_parity_case(
    fixture_name: &str,
    input: &str,
    full_dom: &html::Node,
    plan: &ChunkPlan,
    seed: Option<u64>,
    plan_summary: Option<&str>,
) {
    match run_incremental_pipeline(input, plan) {
        Ok(actual_dom) => {
            if let Err(err) = compare_dom(
                full_dom,
                &actual_dom.dom,
                DomSnapshotOptions {
                    ignore_ids: true,
                    ignore_empty_style: false,
                },
            ) {
                let err = err.to_string();
                let (min_plan, stats, minimized_failure, minimized_dom_error) =
                    shrink_on_dom_mismatch(input, full_dom, plan);
                let msg = parity_failure_message(ParityFailureContext {
                    fixture_name,
                    plan,
                    seed,
                    plan_summary,
                    headline: "dom comparison failed",
                    details: &err,
                    minimized: Some((&min_plan, stats)),
                    minimized_failure: minimized_failure.as_ref(),
                    minimized_dom_error: minimized_dom_error.as_deref(),
                    patch_summary: Some(&actual_dom.patch_summary),
                    patch_preview: actual_dom.patch_preview.as_deref(),
                });
                panic!("{msg}");
            }
        }
        Err(failure) => {
            let minimized = shrink_chunk_plan_with_stats(input, plan, |candidate| {
                run_incremental_pipeline(input, candidate).is_err()
            });
            let (min_plan, stats) = minimized;
            let minimized_failure = if min_plan == *plan {
                None
            } else {
                run_incremental_pipeline(input, &min_plan).err()
            };

            let msg = parity_failure_message(ParityFailureContext {
                fixture_name,
                plan,
                seed,
                plan_summary,
                headline: &failure.message,
                details: &failure.details,
                minimized: Some((&min_plan, stats)),
                minimized_failure: minimized_failure.as_ref(),
                minimized_dom_error: None,
                patch_summary: Some(&failure.patch_summary),
                patch_preview: failure.patch_preview.as_deref(),
            });
            panic!("{msg}");
        }
    }
}

struct FailureInfo {
    message: String,
    details: String,
    patch_summary: String,
    patch_preview: Option<String>,
}

struct RunResult {
    dom: Box<html::Node>,
    patch_summary: String,
    patch_preview: Option<String>,
}

fn run_incremental_pipeline(input: &str, plan: &ChunkPlan) -> Result<RunResult, FailureInfo> {
    let mut parser = HtmlParser::new(HtmlParseOptions::default()).map_err(|err| FailureInfo {
        message: "parser init error".to_string(),
        details: err.to_string(),
        patch_summary: String::new(),
        patch_preview: None,
    })?;
    let mut patch_batches: Vec<Vec<DomPatch>> = Vec::new();
    let mut pending_patches: Vec<DomPatch> = Vec::new();
    let mut pending_tokens: usize = 0;
    let mut pending_bytes: usize = 0;
    let mut last_tokens_processed: u64 = 0;
    let mut chunks_since_flush: usize = 0;
    let batch_policy = BatchPolicy::from_env();
    let mut error: Option<FailureInfo> = None;

    plan.for_each_chunk(input, |chunk: &[u8]| {
        if error.is_some() {
            return;
        }
        chunks_since_flush = chunks_since_flush.saturating_add(1);
        pending_bytes = pending_bytes.saturating_add(chunk.len());
        if let Err(err) = parser.push_bytes(chunk) {
            error = Some(FailureInfo {
                message: "parser push error".to_string(),
                details: err.to_string(),
                patch_summary: patch_summary(&patch_batches),
                patch_preview: patch_preview(&patch_batches, &pending_patches),
            });
            return;
        }
        if let Err(err) = parser.pump() {
            error = Some(FailureInfo {
                message: "parser pump error".to_string(),
                details: err.to_string(),
                patch_summary: patch_summary(&patch_batches),
                patch_preview: patch_preview(&patch_batches, &pending_patches),
            });
            return;
        }
        let total_tokens = parser.tokens_processed();
        pending_tokens = pending_tokens
            .saturating_add(total_tokens.saturating_sub(last_tokens_processed) as usize);
        last_tokens_processed = total_tokens;
        match parser.take_patches() {
            Ok(patches) => {
                if !patches.is_empty() {
                    pending_patches.extend(patches);
                }
            }
            Err(err) => {
                error = Some(FailureInfo {
                    message: "patch drain error".to_string(),
                    details: err.to_string(),
                    patch_summary: patch_summary(&patch_batches),
                    patch_preview: patch_preview(&patch_batches, &pending_patches),
                });
                return;
            }
        }
        if batch_policy.should_flush(
            pending_tokens,
            pending_bytes,
            !pending_patches.is_empty(),
            chunks_since_flush,
        ) {
            flush_pending(&mut pending_patches, &mut patch_batches);
            pending_tokens = 0;
            pending_bytes = 0;
            chunks_since_flush = 0;
        }
    });

    if let Some(err) = error {
        return Err(err);
    }

    parser.finish().map_err(|err| FailureInfo {
        message: "parser finish error".to_string(),
        details: err.to_string(),
        patch_summary: patch_summary(&patch_batches),
        patch_preview: patch_preview(&patch_batches, &pending_patches),
    })?;
    let final_patches = parser.take_patches().map_err(|err| FailureInfo {
        message: "final patch drain error".to_string(),
        details: err.to_string(),
        patch_summary: patch_summary(&patch_batches),
        patch_preview: patch_preview(&patch_batches, &pending_patches),
    })?;
    if !final_patches.is_empty() {
        pending_patches.extend(final_patches);
    }
    flush_pending(&mut pending_patches, &mut patch_batches);

    let summary = patch_summary(&patch_batches);
    let preview = patch_preview(&patch_batches, &[]);
    let output = parser.into_output().map_err(|err| FailureInfo {
        message: "parser output error".to_string(),
        details: err.to_string(),
        patch_summary: summary.clone(),
        patch_preview: preview.clone(),
    })?;
    match apply_patches_to_store(&patch_batches) {
        Ok(dom) => {
            if let Err(err) = compare_dom(&output.document, &dom, DomSnapshotOptions::default()) {
                return Err(FailureInfo {
                    message: "patch application DOM mismatch".to_string(),
                    details: err.to_string(),
                    patch_summary: summary,
                    patch_preview: preview,
                });
            }
            Ok(RunResult {
                dom,
                patch_summary: summary,
                patch_preview: preview,
            })
        }
        Err(err) => Err(FailureInfo {
            message: "patch application failed".to_string(),
            details: err,
            patch_summary: summary,
            patch_preview: preview,
        }),
    }
}

fn parse_html_document(input: &str) -> html::Node {
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("full HTML5 parity parser init");
    parser
        .push_bytes(input.as_bytes())
        .expect("full HTML5 parity push should succeed");
    parser
        .pump()
        .expect("full HTML5 parity pump should succeed");
    parser
        .finish()
        .expect("full HTML5 parity finish should succeed");
    parser
        .into_output()
        .expect("full HTML5 parity output should materialize")
        .document
}

fn apply_patches_to_store(batches: &[Vec<DomPatch>]) -> Result<Box<html::Node>, String> {
    let mut store = DomStore::new();
    let handle = DomHandle(1);
    store.create(handle).map_err(|err| format!("{err:?}"))?;
    let mut version = DomVersion::INITIAL;

    for (batch_index, batch) in batches.iter().enumerate() {
        let from = version;
        let to = from.next();
        store
            .apply(handle, from, to, batch)
            .map_err(|err| format!("batch {batch_index}: {err:?}"))?;
        version = to;
    }

    store.materialize(handle).map_err(|err| format!("{err:?}"))
}

struct ParityFailureContext<'a> {
    fixture_name: &'a str,
    plan: &'a ChunkPlan,
    seed: Option<u64>,
    plan_summary: Option<&'a str>,
    headline: &'a str,
    details: &'a str,
    minimized: Option<(&'a ChunkPlan, html::test_harness::ShrinkStats)>,
    minimized_failure: Option<&'a FailureInfo>,
    minimized_dom_error: Option<&'a str>,
    patch_summary: Option<&'a str>,
    patch_preview: Option<&'a str>,
}

fn parity_failure_message(ctx: ParityFailureContext<'_>) -> String {
    let mut msg = String::new();
    let seed_line = ctx
        .seed
        .map(|seed| format!("seed=0x{seed:016x}"))
        .unwrap_or_else(|| "seed=<none>".to_string());
    msg.push_str(&format!(
        "patch stream parity failed: fixture={} {seed_line}\n",
        ctx.fixture_name
    ));
    msg.push_str(&format!("plan={}\n", ctx.plan));
    if let Some(summary) = ctx.plan_summary {
        msg.push_str(&format!("plan_summary={summary}\n"));
    }
    msg.push_str(&format!("{}: {}\n", ctx.headline, ctx.details));
    if let Some(summary) = ctx.patch_summary
        && !summary.is_empty()
    {
        msg.push_str(&format!("patch_summary={summary}\n"));
    }
    if let Some(preview) = ctx.patch_preview
        && !preview.is_empty()
    {
        msg.push_str(&format!("patch_preview={preview}\n"));
    }

    if let Some((min_plan, stats)) = ctx.minimized {
        msg.push_str(&format!(
            "minimized_plan={min_plan} original_boundaries={} minimized_boundaries={} checks={} budget_exhausted={}\n",
            stats.original_boundaries,
            stats.minimized_boundaries,
            stats.checks,
            stats.budget_exhausted
        ));
        if let Some(error) = ctx.minimized_dom_error {
            msg.push_str(&format!("minimized_dom_error={error}\n"));
        }
        if let Some(min_fail) = ctx.minimized_failure {
            msg.push_str(&format!(
                "minimized_failure={}: {}\n",
                min_fail.message, min_fail.details
            ));
            if let Some(preview) = min_fail.patch_preview.as_deref()
                && !preview.is_empty()
            {
                msg.push_str(&format!("minimized_patch_preview={preview}\n"));
            }
        }
    }

    msg
}

fn patch_summary(batches: &[Vec<DomPatch>]) -> String {
    let mut total = 0usize;
    let mut creates = 0usize;
    let mut appends = 0usize;
    let mut set_text = 0usize;
    let mut set_attrs = 0usize;
    let mut removes = 0usize;
    let mut clears = 0usize;
    let mut comments = 0usize;
    let mut other = 0usize;
    for batch in batches {
        total += batch.len();
        for patch in batch {
            match patch {
                DomPatch::CreateDocument { .. }
                | DomPatch::CreateElement { .. }
                | DomPatch::CreateText { .. } => creates += 1,
                DomPatch::CreateComment { .. } => comments += 1,
                DomPatch::AppendChild { .. } => appends += 1,
                DomPatch::SetText { .. } => set_text += 1,
                DomPatch::SetAttributes { .. } => set_attrs += 1,
                DomPatch::RemoveNode { .. } => removes += 1,
                DomPatch::Clear => clears += 1,
                _ => other += 1,
            }
        }
    }
    let batch_sizes: Vec<usize> = batches.iter().map(|b| b.len()).collect();
    format!(
        "batches={} total={} create={} comment={} append={} set_text={} set_attrs={} remove={} clear={} other={} batch_sizes={batch_sizes:?}",
        batches.len(),
        total,
        creates,
        comments,
        appends,
        set_text,
        set_attrs,
        removes,
        clears,
        other
    )
}

fn patch_preview(batches: &[Vec<DomPatch>], pending: &[DomPatch]) -> Option<String> {
    let mut entries: Vec<String> = Vec::new();
    for (idx, batch) in batches.iter().enumerate() {
        entries.push(format!("Batch({idx})"));
        for patch in batch {
            entries.push(patch_tag(patch));
        }
    }
    if !pending.is_empty() {
        entries.push("Pending".to_string());
        for patch in pending {
            entries.push(patch_tag(patch));
        }
    }
    if entries.is_empty() {
        return None;
    }
    let max_preview = std::env::var("BORROWSER_PATCH_PARITY_VERBOSE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    let total = entries.len();
    if total <= max_preview.saturating_mul(2) {
        return Some(entries.join(","));
    }
    let mut parts = Vec::new();
    let head = max_preview.min(total);
    parts.extend(entries.iter().take(head).cloned());
    parts.push("...".to_string());
    let tail = max_preview;
    parts.extend(entries.iter().skip(total - tail).cloned());
    Some(parts.join(","))
}

fn patch_tag(patch: &DomPatch) -> String {
    match patch {
        DomPatch::Clear => "Clear".to_string(),
        DomPatch::CreateDocument { key, .. } => format!("CreateDocument({key:?})"),
        DomPatch::CreateElement { key, .. } => format!("CreateElement({key:?})"),
        DomPatch::CreateText { key, .. } => format!("CreateText({key:?})"),
        DomPatch::CreateComment { key, .. } => format!("CreateComment({key:?})"),
        DomPatch::AppendChild { parent, child } => {
            format!("AppendChild({parent:?}->{child:?})")
        }
        DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => {
            format!("InsertBefore({parent:?},{child:?}<{before:?})")
        }
        DomPatch::RemoveNode { key } => format!("RemoveNode({key:?})"),
        DomPatch::SetAttributes { key, .. } => format!("SetAttributes({key:?})"),
        DomPatch::SetText { key, .. } => format!("SetText({key:?})"),
        _ => "Other".to_string(),
    }
}

fn flush_pending(pending: &mut Vec<DomPatch>, batches: &mut Vec<Vec<DomPatch>>) {
    if pending.is_empty() {
        return;
    }
    batches.push(std::mem::take(pending));
}

struct BatchPolicy {
    token_threshold: Option<usize>,
    byte_threshold: Option<usize>,
    flush_every_chunk: bool,
    flush_every_n_chunks: Option<usize>,
}

impl BatchPolicy {
    fn from_env() -> Self {
        let token_threshold = std::env::var("BORROWSER_PATCH_PARITY_TOKEN_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0);
        let byte_threshold = std::env::var("BORROWSER_PATCH_PARITY_BYTE_THRESHOLD")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0);
        let flush_every_n_chunks = std::env::var("BORROWSER_PATCH_PARITY_FLUSH_EVERY_N_CHUNKS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0);
        let flush_every_chunk =
            token_threshold.is_none() && byte_threshold.is_none() && flush_every_n_chunks.is_none();
        Self {
            token_threshold,
            byte_threshold,
            flush_every_chunk,
            flush_every_n_chunks,
        }
    }

    fn should_flush(
        &self,
        pending_tokens: usize,
        pending_bytes: usize,
        has_patches: bool,
        chunks_since_flush: usize,
    ) -> bool {
        if !has_patches {
            return false;
        }
        if self.flush_every_chunk {
            return true;
        }
        if let Some(threshold) = self.flush_every_n_chunks
            && chunks_since_flush >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.token_threshold
            && pending_tokens >= threshold
        {
            return true;
        }
        if let Some(threshold) = self.byte_threshold
            && pending_bytes >= threshold
        {
            return true;
        }
        false
    }
}

fn shrink_on_dom_mismatch(
    input: &str,
    full_dom: &html::Node,
    plan: &ChunkPlan,
) -> (
    ChunkPlan,
    html::test_harness::ShrinkStats,
    Option<FailureInfo>,
    Option<String>,
) {
    let (min_plan, stats) = shrink_chunk_plan_with_stats(input, plan, |candidate| {
        match run_incremental_pipeline(input, candidate) {
            Ok(run) => compare_dom(
                full_dom,
                &run.dom,
                DomSnapshotOptions {
                    ignore_ids: true,
                    ignore_empty_style: false,
                },
            )
            .is_err(),
            Err(_) => true,
        }
    });
    let (minimized_failure, minimized_dom_error) = if min_plan == *plan {
        (None, None)
    } else {
        match run_incremental_pipeline(input, &min_plan) {
            Ok(run) => {
                let dom_error = compare_dom(
                    full_dom,
                    &run.dom,
                    DomSnapshotOptions {
                        ignore_ids: true,
                        ignore_empty_style: false,
                    },
                )
                .err()
                .map(|err| err.to_string());
                (None, dom_error)
            }
            Err(err) => (Some(err), None),
        }
    };
    (min_plan, stats, minimized_failure, minimized_dom_error)
}

fn seed_count() -> usize {
    if let Ok(value) = std::env::var("BORROWSER_PATCH_PARITY_SEEDS")
        && let Ok(parsed) = value.parse::<usize>()
    {
        return parsed.max(1);
    }
    if std::env::var("CI").is_ok() {
        DEFAULT_SEEDS_CI
    } else {
        DEFAULT_SEEDS_LOCAL
    }
}

fn run_budget() -> usize {
    if let Ok(value) = std::env::var("BORROWSER_PATCH_PARITY_BUDGET")
        && let Ok(parsed) = value.parse::<usize>()
    {
        return parsed.max(1);
    }
    if std::env::var("CI").is_ok() {
        DEFAULT_BUDGET_CI
    } else {
        DEFAULT_BUDGET_LOCAL
    }
}
