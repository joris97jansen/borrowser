use crate::token_snapshot;
use crate::wpt_formats::TOKENIZER_SKIPS_FORMAT_V1;
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::{BoundaryPolicy, ChunkPlan};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenizerSkipStatus {
    Skip,
    Xfail,
}

#[derive(Clone, Debug)]
pub struct TokenizerSkipOverride {
    pub status: TokenizerSkipStatus,
    pub reason: String,
    pub tracking_issue: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct TokenizerSkipManifest {
    format: String,
    cases: Vec<TokenizerSkipCase>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct TokenizerSkipCase {
    id: String,
    status: TokenizerSkipCaseStatus,
    reason: String,
    tracking_issue: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TokenizerSkipCaseStatus {
    Skip,
    Xfail,
}

pub fn parse_env_bool(key: &str) -> bool {
    match std::env::var(key).ok().as_deref() {
        Some("1") | Some("true") | Some("yes") | Some("on") => true,
        Some("0") | Some("false") | Some("no") | Some("off") | Some("") | None => false,
        Some(other) => panic!("unsupported {key} value '{other}'; use 1/0 or true/false"),
    }
}

pub fn parse_u64(raw: &str) -> Option<u64> {
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

pub fn load_tokenizer_skip_overrides(wpt_root: &Path) -> BTreeMap<String, TokenizerSkipOverride> {
    let root = wpt_root.join("tokenizer");
    let toml_path = root.join("skips.toml");
    let json_path = root.join("skips.json");

    let toml_manifest: TokenizerSkipManifest = {
        let content = fs::read_to_string(&toml_path).unwrap_or_else(|err| {
            panic!("failed to read tokenizer skip TOML {toml_path:?}: {err}")
        });
        toml::from_str(&content).unwrap_or_else(|err| {
            panic!("failed to parse tokenizer skip TOML {toml_path:?}: {err}")
        })
    };
    let json_manifest: TokenizerSkipManifest = {
        let content = fs::read_to_string(&json_path).unwrap_or_else(|err| {
            panic!("failed to read tokenizer skip JSON {json_path:?}: {err}")
        });
        serde_json::from_str(&content).unwrap_or_else(|err| {
            panic!("failed to parse tokenizer skip JSON {json_path:?}: {err}")
        })
    };

    validate_skip_manifest(&toml_manifest, &toml_path);
    validate_skip_manifest(&json_manifest, &json_path);

    let mut toml_sorted = toml_manifest.cases.clone();
    let mut json_sorted = json_manifest.cases.clone();
    toml_sorted.sort_by(|a, b| a.id.cmp(&b.id));
    json_sorted.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(
        toml_manifest.format, TOKENIZER_SKIPS_FORMAT_V1,
        "unsupported tokenizer skip manifest format in {toml_path:?}"
    );
    assert_eq!(
        json_manifest.format, TOKENIZER_SKIPS_FORMAT_V1,
        "unsupported tokenizer skip manifest format in {json_path:?}"
    );
    assert_eq!(
        toml_sorted, json_sorted,
        "tokenizer skip manifests diverged: {toml_path:?} vs {json_path:?}"
    );

    let mut out = BTreeMap::new();
    for entry in toml_sorted {
        let override_status = match entry.status {
            TokenizerSkipCaseStatus::Skip => TokenizerSkipStatus::Skip,
            TokenizerSkipCaseStatus::Xfail => TokenizerSkipStatus::Xfail,
        };
        let inserted = out.insert(
            entry.id.clone(),
            TokenizerSkipOverride {
                status: override_status,
                reason: entry.reason,
                tracking_issue: entry.tracking_issue,
            },
        );
        assert!(
            inserted.is_none(),
            "duplicate tokenizer skip id in {toml_path:?}: {}",
            entry.id
        );
    }
    out
}

pub fn validate_skip_override_ids(
    skip_overrides: &BTreeMap<String, TokenizerSkipOverride>,
    token_case_ids: &BTreeSet<String>,
    manifest_path: &Path,
) {
    for id in skip_overrides.keys() {
        assert!(
            token_case_ids.contains(id),
            "tokenizer skip manifest contains unknown or non-token case id '{id}' (not present in {manifest_path:?})"
        );
    }
}

pub fn applied_skip_override(
    case_id: &str,
    overrides: &BTreeMap<String, TokenizerSkipOverride>,
) -> Option<(TokenizerSkipStatus, String)> {
    let entry = overrides.get(case_id)?;
    Some((
        entry.status,
        format!("{} (tracking: {})", entry.reason, entry.tracking_issue),
    ))
}

pub fn run_tokenizer_whole(input_html: &str, case_id: &str) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut input = Input::new();
    input.push_str(input_html);
    handle_tokenize_result(tokenizer.push_input(&mut input, &mut ctx), "push_input")?;
    let mut out = Vec::new();
    let mut index = 0usize;
    let mut saw_eof_token = false;
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

pub fn run_tokenizer_chunked(
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
        out.extend(token_snapshot::format_tokens(
            batch.tokens(),
            &batch.resolver(),
            ctx,
            context,
            index,
            Some(saw_eof_token),
        )?);
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

fn validate_skip_manifest(manifest: &TokenizerSkipManifest, path: &PathBuf) {
    assert_eq!(
        manifest.format, TOKENIZER_SKIPS_FORMAT_V1,
        "unsupported tokenizer skip manifest format in {path:?}"
    );
    for entry in &manifest.cases {
        assert!(
            !entry.id.trim().is_empty(),
            "empty id in tokenizer skip manifest {path:?}"
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "empty reason for '{}' in tokenizer skip manifest {path:?}",
            entry.id
        );
        assert!(
            !entry.tracking_issue.trim().is_empty(),
            "empty tracking_issue for '{}' in tokenizer skip manifest {path:?}",
            entry.id
        );
    }
}
