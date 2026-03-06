use crate::wpt_formats::TREE_BUILDER_SKIPS_FORMAT_V1;
use html::dom_snapshot::DomSnapshotOptions;
use html::html5::tree_builder::{
    Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult, serialize_dom_for_test_with_options,
};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::{BoundaryPolicy, ChunkPlan};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeBuilderSkipStatus {
    Skip,
    Xfail,
}

#[derive(Clone, Debug)]
pub struct TreeBuilderSkipOverride {
    pub status: TreeBuilderSkipStatus,
    pub reason: String,
    pub tracking_issue: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct TreeBuilderSkipManifest {
    format: String,
    cases: Vec<TreeBuilderSkipCase>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct TreeBuilderSkipCase {
    id: String,
    status: TreeBuilderSkipCaseStatus,
    reason: String,
    tracking_issue: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TreeBuilderSkipCaseStatus {
    Skip,
    Xfail,
}

pub fn ensure_utf8_plan(case_id: &str, plan: &ChunkPlan, plan_label: &str) -> Result<(), String> {
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

pub fn load_tree_builder_skip_overrides(
    wpt_root: &Path,
) -> BTreeMap<String, TreeBuilderSkipOverride> {
    let root = wpt_root.join("tree_builder");
    let toml_path = root.join("skips.toml");
    let json_path = root.join("skips.json");

    let toml_manifest: TreeBuilderSkipManifest = {
        let content = fs::read_to_string(&toml_path).unwrap_or_else(|err| {
            panic!("failed to read tree-builder skip TOML {toml_path:?}: {err}")
        });
        toml::from_str(&content).unwrap_or_else(|err| {
            panic!("failed to parse tree-builder skip TOML {toml_path:?}: {err}")
        })
    };
    let json_manifest: TreeBuilderSkipManifest = {
        let content = fs::read_to_string(&json_path).unwrap_or_else(|err| {
            panic!("failed to read tree-builder skip JSON {json_path:?}: {err}")
        });
        serde_json::from_str(&content).unwrap_or_else(|err| {
            panic!("failed to parse tree-builder skip JSON {json_path:?}: {err}")
        })
    };

    validate_skip_manifest(&toml_manifest, &toml_path);
    validate_skip_manifest(&json_manifest, &json_path);

    let mut toml_sorted = toml_manifest.cases.clone();
    let mut json_sorted = json_manifest.cases.clone();
    toml_sorted.sort_by(|a, b| a.id.cmp(&b.id));
    json_sorted.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(
        toml_manifest.format, TREE_BUILDER_SKIPS_FORMAT_V1,
        "unsupported tree-builder skip manifest format in {toml_path:?}"
    );
    assert_eq!(
        json_manifest.format, TREE_BUILDER_SKIPS_FORMAT_V1,
        "unsupported tree-builder skip manifest format in {json_path:?}"
    );
    assert_eq!(
        toml_sorted, json_sorted,
        "tree-builder skip manifests diverged: {toml_path:?} vs {json_path:?}"
    );

    let mut out = BTreeMap::new();
    for entry in toml_sorted {
        let override_status = match entry.status {
            TreeBuilderSkipCaseStatus::Skip => TreeBuilderSkipStatus::Skip,
            TreeBuilderSkipCaseStatus::Xfail => TreeBuilderSkipStatus::Xfail,
        };
        let inserted = out.insert(
            entry.id.clone(),
            TreeBuilderSkipOverride {
                status: override_status,
                reason: entry.reason,
                tracking_issue: entry.tracking_issue,
            },
        );
        assert!(
            inserted.is_none(),
            "duplicate tree-builder skip id in {toml_path:?}: {}",
            entry.id
        );
    }
    out
}

pub fn validate_skip_override_ids(
    skip_overrides: &BTreeMap<String, TreeBuilderSkipOverride>,
    dom_case_ids: &BTreeSet<String>,
    manifest_path: &Path,
) {
    for id in skip_overrides.keys() {
        assert!(
            dom_case_ids.contains(id),
            "tree-builder skip manifest contains unknown or non-dom case id '{id}' (not present in {manifest_path:?})"
        );
    }
}

pub fn applied_skip_override(
    case_id: &str,
    overrides: &BTreeMap<String, TreeBuilderSkipOverride>,
) -> Option<(TreeBuilderSkipStatus, String)> {
    let entry = overrides.get(case_id)?;
    Some((
        entry.status,
        format!("{} (tracking: {})", entry.reason, entry.tracking_issue),
    ))
}

pub fn run_tree_builder_whole(
    input_html: &str,
    case_id: &str,
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
        return Err(format!(
            "expected EOF token but none was observed (case '{}' [whole])",
            case_id
        ));
    }

    let dom = html::test_harness::materialize_patch_batches(&patch_batches)?;
    Ok(serialize_dom_for_test_with_options(&dom, options))
}

pub fn run_tree_builder_chunked(
    input_html: &str,
    case_id: &str,
    plan: &ChunkPlan,
    plan_label: &str,
    options: DomSnapshotOptions,
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
    Ok(serialize_dom_for_test_with_options(&dom, options))
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

fn validate_skip_manifest(manifest: &TreeBuilderSkipManifest, path: &Path) {
    assert_eq!(
        manifest.format, TREE_BUILDER_SKIPS_FORMAT_V1,
        "unsupported tree-builder skip manifest format in {path:?}"
    );
    for entry in &manifest.cases {
        assert!(
            !entry.id.trim().is_empty(),
            "empty id in tree-builder skip manifest {path:?}"
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "empty reason for '{}' in tree-builder skip manifest {path:?}",
            entry.id
        );
        assert!(
            !entry.tracking_issue.trim().is_empty(),
            "empty tracking_issue for '{}' in tree-builder skip manifest {path:?}",
            entry.id
        );
    }
}
