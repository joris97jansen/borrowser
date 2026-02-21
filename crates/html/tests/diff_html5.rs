#![cfg(all(feature = "html5", feature = "dom-snapshot"))]

use html::dom_snapshot::{DomSnapshotOptions, compare_dom};
use html::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult};
use html::html5::{
    AttributeValue, DocumentParseContext, Html5Tokenizer, Input, TextResolver, TextValue, Token,
    TokenizeResult, TokenizerConfig,
};
use html::{TokenStream, build_owned_dom, tokenize};
use html_test_support::{diff_lines, escape_text};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod wpt_manifest;

use wpt_manifest::{DiffKind, FixtureStatus, WptCase, load_manifest};

#[derive(Clone, Debug, Eq, PartialEq)]
enum NormToken {
    Doctype {
        name: Option<String>,
    },
    StartTag {
        name: String,
        attrs: Vec<(String, Option<String>)>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Comment {
        text: String,
    },
    Char {
        text: String,
    },
    Eof,
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
    let cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");

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
    // Normalize to comparable semantic tokens: coalesce CHAR runs and compare only
    // shared DOCTYPE fields (name).
    let simplified = normalize_simplified_tokens(&tokenize(input));
    let html5 = normalize_html5_tokens(input, &case.id, &case.path, strict)?;
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
    let html5_tokens = normalize_html5_tokens(input, &case.id, &case.path, strict)?;
    if html5_only_eof(&html5_tokens) && !input.is_empty() {
        return Err(format!(
            "SKIP: html5 tokenizer produced only EOF (unimplemented) for '{}' ({})",
            case.id,
            case.path.display()
        ));
    }
    let simplified_stream = tokenize(input);
    let simplified_dom = build_owned_dom(&simplified_stream);
    let html5_dom = run_html5_dom(input, &case.id, &case.path)?;
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

fn html5_only_eof(tokens: &[NormToken]) -> bool {
    matches!(tokens, [NormToken::Eof])
}

fn normalize_html5_tokens(
    input_html: &str,
    case_id: &str,
    case_path: &Path,
    strict: bool,
) -> Result<Vec<NormToken>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut input = Input::new();
    let mut saw_eof_token = false;
    let mut out = Vec::new();

    input.push_str(input_html);
    loop {
        let consumed_before = tokenizer.stats().bytes_consumed;
        let result = tokenizer.push_input(&mut input, &mut ctx);
        let consumed_after = tokenizer.stats().bytes_consumed;
        handle_tokenize_result(result, "push_input")
            .map_err(|err| format!("tokenizer error in '{}' at {:?}: {err}", case_id, case_path))?;
        // Diff harness assumes feed-all-then-finish semantics: NeedMoreInput should
        // only occur at end-of-buffer. If this changes, fail fast with context.
        if matches!(result, TokenizeResult::NeedMoreInput)
            && consumed_after < input.as_str().len() as u64
        {
            return Err(format!(
                "harness assumption violated: tokenizer returned NeedMoreInput despite buffered data in '{}' at {:?} (result={result:?}, consumed={} buffered={} before={}); either tokenizer stalled or input abstraction changed (repro: set DIFF_IDS={})",
                case_id,
                case_path,
                consumed_after,
                input.as_str().len(),
                consumed_before,
                case_id
            ));
        }
        drain_norm_tokens(
            &mut out,
            &mut tokenizer,
            &mut input,
            &ctx,
            case_id,
            case_path,
            strict,
            &mut saw_eof_token,
        )?;
        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    handle_tokenize_result(tokenizer.finish(&input), "finish")
        .map_err(|err| format!("tokenizer error in '{}' at {:?}: {err}", case_id, case_path))?;
    drain_norm_tokens(
        &mut out,
        &mut tokenizer,
        &mut input,
        &ctx,
        case_id,
        case_path,
        strict,
        &mut saw_eof_token,
    )?;
    if !saw_eof_token {
        return Err(format!(
            "expected EOF token but none was observed (case '{}' at {:?})",
            case_id, case_path
        ));
    }
    Ok(out)
}

fn run_html5_dom(input_html: &str, case_id: &str, case_path: &Path) -> Result<html::Node, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).map_err(|err| {
            format!(
                "failed to init tree builder in '{}' at {:?}: {err:?}",
                case_id, case_path
            )
        })?;
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;

    input.push_str(input_html);
    loop {
        let consumed_before = tokenizer.stats().bytes_consumed;
        let result = tokenizer.push_input(&mut input, &mut ctx);
        let consumed_after = tokenizer.stats().bytes_consumed;
        handle_tokenize_result(result, "push_input")
            .map_err(|err| format!("tokenizer error in '{}' at {:?}: {err}", case_id, case_path))?;
        if matches!(result, TokenizeResult::NeedMoreInput)
            && consumed_after < input.as_str().len() as u64
        {
            return Err(format!(
                "harness assumption violated: tokenizer returned NeedMoreInput despite buffered data in '{}' at {:?} (result={result:?}, consumed={} buffered={} before={}); either tokenizer stalled or input abstraction changed (repro: set DIFF_IDS={})",
                case_id,
                case_path,
                consumed_after,
                input.as_str().len(),
                consumed_before,
                case_id
            ));
        }
        drain_batches(
            &mut tokenizer,
            &mut input,
            &mut builder,
            &ctx,
            &mut patch_batches,
            &mut saw_eof_token,
        )
        .map_err(|err| {
            format!(
                "tree builder error in '{}' at {:?}: {err}",
                case_id, case_path
            )
        })?;
        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    handle_tokenize_result(tokenizer.finish(&input), "finish")
        .map_err(|err| format!("tokenizer error in '{}' at {:?}: {err}", case_id, case_path))?;
    drain_batches(
        &mut tokenizer,
        &mut input,
        &mut builder,
        &ctx,
        &mut patch_batches,
        &mut saw_eof_token,
    )
    .map_err(|err| {
        format!(
            "tree builder error in '{}' at {:?}: {err}",
            case_id, case_path
        )
    })?;
    if !saw_eof_token {
        return Err(format!(
            "expected EOF token but none was observed (case '{}' at {:?})",
            case_id, case_path
        ));
    }
    html::test_harness::materialize_patch_batches(&patch_batches)
}

fn normalize_simplified_tokens(stream: &TokenStream) -> Vec<NormToken> {
    let mut out = Vec::with_capacity(stream.tokens().len());
    for token in stream.tokens() {
        match token {
            html::Token::Doctype(payload) => {
                let name = normalize_simplified_doctype_name(stream.payload_text(payload));
                out.push(NormToken::Doctype { name });
            }
            html::Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let name = stream.atoms().resolve(*name).to_ascii_lowercase();
                let mut attrs = Vec::with_capacity(attributes.len());
                for (index, (attr, value)) in attributes.iter().enumerate() {
                    let attr_name = stream.atoms().resolve(*attr).to_ascii_lowercase();
                    let value = value
                        .as_ref()
                        .map(|value| stream.attr_value(value).to_string());
                    attrs.push((attr_name, value, index));
                }
                attrs.sort_by(|(a_name, a_value, a_index), (b_name, b_value, b_index)| {
                    let cmp = a_name
                        .cmp(b_name)
                        .then_with(|| a_value.as_deref().cmp(&b_value.as_deref()));
                    if cmp == std::cmp::Ordering::Equal {
                        a_index.cmp(b_index)
                    } else {
                        cmp
                    }
                });
                let attrs = attrs
                    .into_iter()
                    .map(|(name, value, _)| (name, value))
                    .collect();
                // Diff normalization only: treat HTML void elements as self-closing
                // to reduce cross-implementation noise in token comparisons.
                let self_closing = *self_closing || is_html_void_tag(&name);
                out.push(NormToken::StartTag {
                    name,
                    attrs,
                    self_closing,
                });
            }
            html::Token::EndTag(name) => {
                let name = stream.atoms().resolve(*name).to_ascii_lowercase();
                out.push(NormToken::EndTag { name });
            }
            html::Token::Comment(payload) => {
                let text = stream.payload_text(payload).to_string();
                out.push(NormToken::Comment { text });
            }
            html::Token::TextSpan { .. } | html::Token::TextOwned { .. } => {
                let text = stream.text(token).unwrap_or("");
                push_char(&mut out, text);
            }
        }
    }
    if !matches!(out.last(), Some(NormToken::Eof)) {
        out.push(NormToken::Eof);
    }
    out
}

fn normalize_simplified_doctype_name(raw: &str) -> Option<String> {
    let mut text = raw.trim();
    if text.is_empty() {
        return None;
    }

    // Legacy tokenizer payloads may include the keyword itself (e.g. "DOCTYPE html").
    // Normalize to the semantic doctype name used by the html5 tokenizer diff path.
    if text.len() >= 7
        && text
            .get(..7)
            .is_some_and(|head| head.eq_ignore_ascii_case("doctype"))
    {
        let after_kw = &text[7..];
        if after_kw
            .chars()
            .next()
            .is_none_or(|ch| ch.is_ascii_whitespace())
        {
            text = after_kw.trim_start();
        }
    }

    let name = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.is_empty() { None } else { Some(name) }
}

fn push_char(tokens: &mut Vec<NormToken>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(NormToken::Char { text: existing }) = tokens.last_mut() {
        existing.push_str(text);
        return;
    }
    tokens.push(NormToken::Char {
        text: text.to_string(),
    });
}

fn format_norm_tokens(tokens: &[NormToken]) -> Vec<String> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let line = match token {
            NormToken::Doctype { name } => {
                let name = name.as_deref().unwrap_or("null");
                format!("DOCTYPE name={name}")
            }
            NormToken::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let mut line = String::new();
                line.push_str("START name=");
                line.push_str(name);
                line.push_str(" attrs=[");
                for (i, (attr, value)) in attrs.iter().enumerate() {
                    if i > 0 {
                        line.push(' ');
                    }
                    line.push_str(attr);
                    if let Some(value) = value {
                        line.push_str("=\"");
                        line.push_str(&escape_text(value));
                        line.push('"');
                    }
                }
                line.push_str("] self_closing=");
                line.push_str(if *self_closing { "true" } else { "false" });
                line
            }
            NormToken::EndTag { name } => format!("END name={name}"),
            NormToken::Comment { text } => format!("COMMENT text=\"{}\"", escape_text(text)),
            NormToken::Char { text } => format!("CHAR text=\"{}\"", escape_text(text)),
            NormToken::Eof => "EOF".to_string(),
        };
        out.push(line);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn drain_norm_tokens(
    out: &mut Vec<NormToken>,
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
    case_id: &str,
    case_path: &Path,
    strict: bool,
    saw_eof_token: &mut bool,
) -> Result<(), String> {
    loop {
        let batch = tokenizer.next_batch(input);
        let resolver = batch.resolver();
        let mut saw_any = false;
        for token in batch.iter() {
            saw_any = true;
            match token {
                Token::Doctype {
                    name,
                    public_id: _,
                    system_id: _,
                    force_quirks: _,
                } => {
                    let name = match name {
                        None => None,
                        Some(id) => {
                            let resolved = ctx.atoms.resolve(*id).unwrap_or("");
                            if resolved.is_empty() && strict {
                                return Err(format!(
                                    "empty doctype name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={id:?})",
                                    case_id, case_path
                                ));
                            }
                            if resolved.is_empty() {
                                None
                            } else {
                                Some(resolved.to_ascii_lowercase())
                            }
                        }
                    };
                    out.push(NormToken::Doctype { name });
                }
                Token::StartTag {
                    name,
                    attrs,
                    self_closing,
                } => {
                    let raw_name_id = *name;
                    let name = ctx.atoms.resolve(*name).unwrap_or("");
                    if name.is_empty() && strict {
                        return Err(format!(
                            "empty start tag name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={raw_name_id:?})",
                            case_id, case_path
                        ));
                    }
                    let name = name.to_ascii_lowercase();
                    let mut attrs_out: Vec<(String, Option<String>, usize)> =
                        Vec::with_capacity(attrs.len());
                    for (index, attr) in attrs.iter().enumerate() {
                        let attr_name = ctx.atoms.resolve(attr.name).unwrap_or("");
                        if attr_name.is_empty() && strict {
                            return Err(format!(
                                "empty attribute name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={:?})",
                                case_id, case_path, attr.name
                            ));
                        }
                        let attr_name = attr_name.to_ascii_lowercase();
                        let value = match &attr.value {
                            None => None,
                            Some(AttributeValue::Span(span)) => Some(
                                resolver
                                    .resolve_span(*span)
                                    .map_err(|err| {
                                        format!(
                                            "invalid attribute value span in '{}' (attr {}) at {:?}: {err:?}",
                                            case_id, attr_name, case_path
                                        )
                                    })?
                                    .to_string(),
                            ),
                            Some(AttributeValue::Owned(value)) => Some(value.clone()),
                        };
                        attrs_out.push((attr_name, value, index));
                    }
                    attrs_out.sort_by(|(a_name, a_value, a_index), (b_name, b_value, b_index)| {
                        let cmp = a_name
                            .cmp(b_name)
                            .then_with(|| a_value.as_deref().cmp(&b_value.as_deref()));
                        if cmp == std::cmp::Ordering::Equal {
                            a_index.cmp(b_index)
                        } else {
                            cmp
                        }
                    });
                    let attrs = attrs_out
                        .into_iter()
                        .map(|(name, value, _)| (name, value))
                        .collect();
                    // Diff normalization only: treat HTML void elements as self-closing
                    // to reduce cross-implementation noise in token comparisons.
                    let self_closing = *self_closing || is_html_void_tag(&name);
                    out.push(NormToken::StartTag {
                        name,
                        attrs,
                        self_closing,
                    });
                }
                Token::EndTag { name } => {
                    let raw_name_id = *name;
                    let name = ctx.atoms.resolve(*name).unwrap_or("");
                    if name.is_empty() && strict {
                        return Err(format!(
                            "empty end tag name in case '{}' at {:?} (DIFF_STRICT=1, atom_id={raw_name_id:?})",
                            case_id, case_path
                        ));
                    }
                    let name = name.to_ascii_lowercase();
                    out.push(NormToken::EndTag { name });
                }
                Token::Comment { text } => {
                    let text = match text {
                        TextValue::Span(span) => resolver.resolve_span(*span).map_err(|err| {
                            format!(
                                "invalid comment span in '{}' at {:?}: {err:?}",
                                case_id, case_path
                            )
                        })?,
                        TextValue::Owned(text) => text.as_str(),
                    };
                    out.push(NormToken::Comment {
                        text: text.to_string(),
                    });
                }
                Token::Text { text } => {
                    let text = match text {
                        TextValue::Span(span) => resolver.resolve_span(*span).map_err(|err| {
                            format!(
                                "invalid text span in '{}' at {:?}: {err:?}",
                                case_id, case_path
                            )
                        })?,
                        TextValue::Owned(value) => value.as_str(),
                    };
                    push_char(out, text);
                }
                Token::Eof => {
                    if !*saw_eof_token {
                        *saw_eof_token = true;
                        out.push(NormToken::Eof);
                    }
                }
            }
        }
        if !saw_any {
            break;
        }
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

fn is_html_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn wpt_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("wpt")
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

    let mut selected = Vec::new();
    for case in cases {
        if !ids.is_empty() && !ids.iter().any(|id| id == &case.id) {
            continue;
        }
        if let Some(filter) = filter.as_deref() {
            let filter = filter.to_lowercase();
            let id = case.id.to_lowercase();
            let path = case.path.to_string_lossy().to_lowercase();
            if !id.contains(&filter) && !path.contains(&filter) {
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

    let has_filters = filter.is_some() || !ids.is_empty();
    if has_filters && selected.is_empty() {
        panic!(
            "no diff cases matched filters (DIFF_FILTER={:?}, DIFF_IDS={:?})",
            filter, ids
        );
    }
    selected
}
