use crate::token_snapshot;
use crate::tokenizer_text_mode::TokenizerTextModeSupport;
use crate::wpt_formats::TOKENIZER_SKIPS_FORMAT_V1;
use html::html5::{
    AtomId, DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig,
};
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
    let text_mode_support = TokenizerTextModeSupport::new(&mut ctx);
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut input = Input::new();
    input.push_str(input_html);
    let mut out = Vec::new();
    let mut index = 0usize;
    let mut saw_eof_token = false;
    let mut active_text_mode = None;
    let context = token_snapshot::TokenFormatContext {
        case_id,
        mode: "whole",
    };
    let driver = TokenizerHarnessDriver {
        context: &context,
        text_mode_support: &text_mode_support,
    };
    let mut drain_state = TokenDrainState {
        out: &mut out,
        index: &mut index,
        saw_eof_token: &mut saw_eof_token,
        active_text_mode: &mut active_text_mode,
    };
    pump_until_blocked(
        &mut drain_state,
        &mut tokenizer,
        &mut input,
        &mut ctx,
        &driver,
    )?;
    handle_tokenize_result(tokenizer.finish(&input), "finish")?;
    let _ = drain_tokens(
        &mut drain_state,
        &mut tokenizer,
        &mut input,
        &ctx,
        &driver,
        false,
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
    let text_mode_support = TokenizerTextModeSupport::new(&mut ctx);
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();
    let mut index = 0usize;
    let mut saw_eof_token = false;
    let mut active_text_mode = None;
    let context = token_snapshot::TokenFormatContext {
        case_id,
        mode: plan_label,
    };
    let driver = TokenizerHarnessDriver {
        context: &context,
        text_mode_support: &text_mode_support,
    };
    let mut drain_state = TokenDrainState {
        out: &mut out,
        index: &mut index,
        saw_eof_token: &mut saw_eof_token,
        active_text_mode: &mut active_text_mode,
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
        if let Err(err) = pump_until_blocked(
            &mut drain_state,
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &driver,
        ) {
            error = Some(format!("case '{}' [{plan_label}] error: {err}", case_id));
        }
    });

    if let Some(err) = error {
        return Err(err);
    }

    handle_tokenize_result(tokenizer.finish(&input), "finish")?;
    let _ = drain_tokens(
        &mut drain_state,
        &mut tokenizer,
        &mut input,
        &ctx,
        &driver,
        false,
    )?;
    if !saw_eof_token {
        return Err(format!(
            "expected EOF token but none was observed (case '{}' [{plan_label}])",
            case_id
        ));
    }
    Ok(out)
}

fn pump_until_blocked(
    drain_state: &mut TokenDrainState<'_>,
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    driver: &TokenizerHarnessDriver<'_>,
) -> Result<(), String> {
    loop {
        let result = tokenizer.push_input_until_token(input, ctx);
        handle_tokenize_result(result, "push_input")?;
        let drained = drain_tokens(drain_state, tokenizer, input, ctx, driver, true)?;
        if matches!(result, TokenizeResult::NeedMoreInput) && !drained {
            break;
        }
    }
    Ok(())
}

fn drain_tokens(
    drain_state: &mut TokenDrainState<'_>,
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
    driver: &TokenizerHarnessDriver<'_>,
    expect_token_granular_batches: bool,
) -> Result<bool, String> {
    let mut saw_any = false;
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        saw_any = true;
        if expect_token_granular_batches {
            assert_eq!(
                batch.tokens().len(),
                1,
                "tokenizer control-aware tokenizer harness must observe exactly one token per pump"
            );
        }
        let (formatted, control) = {
            let resolver = batch.resolver();
            let formatted = token_snapshot::format_tokens(
                batch.tokens(),
                &resolver,
                ctx,
                driver.context,
                drain_state.index,
                Some(drain_state.saw_eof_token),
            )?;
            let control = batch.tokens().first().and_then(|token| {
                driver
                    .text_mode_support
                    .control_for_token(token, drain_state.active_text_mode)
            });
            (formatted, control)
        };
        drain_state.out.extend(formatted);
        if let Some(control) = control {
            tokenizer.apply_control(control);
        }
    }
    Ok(saw_any)
}

struct TokenizerHarnessDriver<'a> {
    context: &'a token_snapshot::TokenFormatContext<'a>,
    text_mode_support: &'a TokenizerTextModeSupport,
}

struct TokenDrainState<'a> {
    out: &'a mut Vec<String>,
    index: &'a mut usize,
    saw_eof_token: &'a mut bool,
    active_text_mode: &'a mut Option<AtomId>,
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
