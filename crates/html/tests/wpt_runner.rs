#![cfg(feature = "html5")]

use html::dom_snapshot::{DomSnapshot, DomSnapshotOptions};
use html::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderStepResult};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureStatus {
    Active,
    Xfail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaseKind {
    Dom,
    Tokens,
}

struct ExpectedDom {
    options: DomSnapshotOptions,
    lines: Vec<String>,
}

struct ExpectedTokens {
    lines: Vec<String>,
}

struct WptCase {
    id: String,
    path: PathBuf,
    expected: PathBuf,
    status: FixtureStatus,
    reason: Option<String>,
    kind: CaseKind,
}

#[test]
fn wpt_parsing_subset() {
    let manifest_path = wpt_root().join("manifest.txt");
    let cases = load_manifest(&manifest_path);
    assert!(!cases.is_empty(), "no WPT cases found in {manifest_path:?}");

    for case in cases {
        let input = fs::read_to_string(&case.path)
            .unwrap_or_else(|err| panic!("failed to read WPT input {:?}: {err}", case.path));
        match case.kind {
            CaseKind::Dom => {
                let expected = parse_dom_file(&case.expected);
                let actual = run_tree_builder_whole(&input, expected.options);
                match case.status {
                    FixtureStatus::Active => match actual {
                        Ok(lines) => {
                            if lines.as_slice() != expected.lines.as_slice() {
                                panic!(
                                    "WPT DOM mismatch for '{}' ({})\n{}\nexpected file: {:?}\ninput file: {:?}",
                                    case.id,
                                    case.path.display(),
                                    diff_lines(&expected.lines, &lines),
                                    case.expected,
                                    case.path
                                );
                            }
                        }
                        Err(err) => {
                            panic!(
                                "WPT case '{}' failed ({}) error: {err}",
                                case.id,
                                case.path.display()
                            );
                        }
                    },
                    FixtureStatus::Xfail => match actual {
                        Ok(lines) => {
                            if lines.as_slice() == expected.lines.as_slice() {
                                panic!(
                                    "WPT case '{}' matched expected DOM but is marked xfail; reason: {}",
                                    case.id,
                                    case.reason.as_deref().unwrap_or("<missing reason>")
                                );
                            }
                        }
                        Err(_) => {
                            // Expected to fail for now.
                        }
                    },
                }
            }
            CaseKind::Tokens => {
                let expected = parse_tokens_file(&case.expected);
                let actual = run_tokenizer_whole(&input);
                match case.status {
                    FixtureStatus::Active => match actual {
                        Ok(lines) => {
                            if lines.as_slice() != expected.lines.as_slice() {
                                panic!(
                                    "WPT token mismatch for '{}' ({})\n{}\nexpected file: {:?}\ninput file: {:?}",
                                    case.id,
                                    case.path.display(),
                                    diff_lines(&expected.lines, &lines),
                                    case.expected,
                                    case.path
                                );
                            }
                        }
                        Err(err) => {
                            panic!(
                                "WPT case '{}' failed ({}) error: {err}",
                                case.id,
                                case.path.display()
                            );
                        }
                    },
                    FixtureStatus::Xfail => match actual {
                        Ok(lines) => {
                            if lines.as_slice() == expected.lines.as_slice() {
                                panic!(
                                    "WPT case '{}' matched expected tokens but is marked xfail; reason: {}",
                                    case.id,
                                    case.reason.as_deref().unwrap_or("<missing reason>")
                                );
                            }
                        }
                        Err(_) => {
                            // Expected to fail for now.
                        }
                    },
                }
            }
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

fn load_manifest(path: &Path) -> Vec<WptCase> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read WPT manifest {path:?}: {err}"));
    let mut format = None::<String>;
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    let mut cases = Vec::new();

    let mut flush = |current: &mut BTreeMap<String, String>| {
        if current.is_empty() {
            return;
        }
        let id = current
            .remove("id")
            .unwrap_or_else(|| panic!("missing id in WPT manifest {path:?}"));
        let rel_path = current
            .remove("path")
            .unwrap_or_else(|| panic!("missing path for '{id}' in WPT manifest {path:?}"));
        let expected = current
            .remove("expected")
            .unwrap_or_else(|| panic!("missing expected for '{id}' in WPT manifest {path:?}"));
        let kind = match current.remove("kind").as_deref() {
            Some("tokens") => CaseKind::Tokens,
            Some("dom") | None => CaseKind::Dom,
            Some(other) => panic!("unsupported kind '{other}' for '{id}' in {path:?}"),
        };
        let status = match current.remove("status").as_deref() {
            Some("xfail") => FixtureStatus::Xfail,
            Some("active") | None => FixtureStatus::Active,
            Some(other) => panic!("unsupported status '{other}' for '{id}' in {path:?}"),
        };
        let reason = current.remove("reason");
        if status == FixtureStatus::Xfail && reason.as_deref().unwrap_or("").is_empty() {
            panic!("xfail case '{id}' missing reason in {path:?}");
        }
        if status != FixtureStatus::Xfail && reason.is_some() {
            panic!("case '{id}' has reason but is not xfail in {path:?}");
        }
        if !current.is_empty() {
            let keys = current.keys().cloned().collect::<Vec<_>>();
            panic!("unknown keys for '{id}' in {path:?}: {keys:?}");
        }
        let root = path
            .parent()
            .unwrap_or_else(|| panic!("manifest has no parent directory"));
        let input_path = root.join(rel_path);
        let expected_path = root.join(expected);
        if !input_path.is_file() {
            panic!("WPT input file missing for '{id}': {input_path:?}");
        }
        if !expected_path.is_file() {
            panic!("WPT expected file missing for '{id}': {expected_path:?}");
        }
        let expected_name = expected_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| {
                panic!("expected snapshot filename is not valid UTF-8: {expected_path:?}")
            });
        match kind {
            CaseKind::Tokens => {
                if !expected_name.ends_with(".tokens.txt") {
                    panic!(
                        "token case '{id}' must use .tokens.txt expected file: {expected_path:?}"
                    );
                }
            }
            CaseKind::Dom => {
                if !expected_name.ends_with(".dom.txt") {
                    panic!("dom case '{id}' must use .dom.txt expected file: {expected_path:?}");
                }
            }
        }
        cases.push(WptCase {
            id,
            path: input_path,
            expected: expected_path,
            status,
            reason,
            kind,
        });
        current.clear();
    };

    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            flush(&mut current);
            continue;
        }
        if let Some(stripped) = line.strip_prefix('#') {
            let header = stripped.trim();
            if let Some((key, value)) = header.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                if key == "format" {
                    format = Some(value);
                }
            }
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid manifest line in {path:?}: '{line}'"));
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if current.insert(key.clone(), value).is_some() {
            panic!("duplicate key '{key}' in {path:?}");
        }
    }
    flush(&mut current);

    match format.as_deref() {
        Some("wpt-manifest-v1") => {}
        Some(other) => panic!("unsupported manifest format '{other}' in {path:?}"),
        None => panic!("missing manifest format header in {path:?}"),
    }

    cases
}

fn parse_dom_file(path: &Path) -> ExpectedDom {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read expected DOM file {path:?}: {err}"));
    let mut lines = Vec::new();
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(stripped) = line.strip_prefix('#') {
            let header = stripped.trim();
            if header.is_empty() {
                continue;
            }
            if let Some((key, value)) = header.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                if headers.insert(key.clone(), value).is_some() {
                    panic!("duplicate header '{key}' in {path:?}");
                }
            } else {
                lines.push(line.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }

    let format = headers
        .get("format")
        .unwrap_or_else(|| panic!("missing format header in {path:?}"));
    assert_eq!(format, "html5-dom-v1", "unsupported format in {path:?}");

    let options = DomSnapshotOptions {
        ignore_ids: header_bool(&headers, "ignore_ids", true, path),
        ignore_empty_style: header_bool(&headers, "ignore_empty_style", true, path),
    };

    if lines.is_empty() {
        panic!("expected DOM file {path:?} has no snapshot lines");
    }
    if !lines[0].starts_with("#document") {
        panic!("expected DOM file {path:?} must start with #document");
    }

    ExpectedDom { options, lines }
}

fn parse_tokens_file(path: &Path) -> ExpectedTokens {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read expected tokens file {path:?}: {err}"));
    let mut lines = Vec::new();
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(stripped) = line.strip_prefix('#') {
            let header = stripped.trim();
            if header.is_empty() {
                continue;
            }
            if let Some((key, value)) = header.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                if headers.insert(key.clone(), value).is_some() {
                    panic!("duplicate header '{key}' in {path:?}");
                }
            } else {
                lines.push(line.to_string());
            }
        } else {
            lines.push(line.to_string());
        }
    }

    let format = headers
        .get("format")
        .unwrap_or_else(|| panic!("missing format header in {path:?}"));
    assert_eq!(format, "html5-token-v1", "unsupported format in {path:?}");
    if headers.contains_key("status") || headers.contains_key("reason") {
        panic!(
            "status/reason headers are not supported in {path:?}; use manifest.txt as the source of truth"
        );
    }

    if lines.is_empty() {
        panic!("expected tokens file {path:?} has no token lines");
    }
    if lines.last().map(String::as_str) != Some("EOF") {
        panic!("expected tokens file {path:?} must end with EOF");
    }

    ExpectedTokens { lines }
}

fn header_bool(headers: &BTreeMap<String, String>, key: &str, default: bool, path: &Path) -> bool {
    match headers.get(key).map(|s| s.as_str()) {
        None => default,
        Some("true") => true,
        Some("false") => false,
        Some(other) => panic!("invalid boolean '{other}' for {key} in {path:?}"),
    }
}

fn run_tree_builder_whole(
    input_html: &str,
    options: DomSnapshotOptions,
) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;

    input.push_str(input_html);
    handle_tokenize_result(tokenizer.push_input(&mut input), "push_input")?;
    drain_batches(
        &mut tokenizer,
        &mut input,
        &mut builder,
        &ctx,
        &mut patch_batches,
        &mut saw_eof_token,
    )?;

    handle_tokenize_result(tokenizer.finish(), "finish")?;
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

fn run_tokenizer_whole(input_html: &str) -> Result<Vec<String>, String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut input = Input::new();
    let mut saw_eof_token = false;
    input.push_str(input_html);
    handle_tokenize_result(tokenizer.push_input(&mut input), "push_input")?;
    let mut out = Vec::new();
    drain_tokens(
        &mut out,
        &mut tokenizer,
        &mut input,
        &ctx,
        &mut saw_eof_token,
    )?;
    handle_tokenize_result(tokenizer.finish(), "finish")?;
    drain_tokens(
        &mut out,
        &mut tokenizer,
        &mut input,
        &ctx,
        &mut saw_eof_token,
    )?;
    if !saw_eof_token {
        return Err("expected EOF token but none was observed".to_string());
    }
    Ok(out)
}

fn drain_tokens(
    out: &mut Vec<String>,
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
    saw_eof_token: &mut bool,
) -> Result<(), String> {
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        out.extend(format_tokens(
            batch.tokens(),
            &resolver,
            ctx,
            saw_eof_token,
        )?);
    }
    Ok(())
}

fn format_tokens(
    tokens: &[html::html5::Token],
    resolver: &impl html::html5::TextResolver,
    ctx: &DocumentParseContext,
    saw_eof_token: &mut bool,
) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        if matches!(token, html::html5::Token::Eof) {
            *saw_eof_token = true;
        }
        let line = match token {
            html::html5::Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                let name = match name {
                    None => "null".to_string(),
                    Some(id) => ctx
                        .atoms
                        .resolve(*id)
                        .ok_or_else(|| format!("unknown atom id in doctype: {id:?}"))?
                        .to_string(),
                };
                let public_id = public_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                let system_id = system_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |s| format!("\"{}\"", escape_text(s)));
                format!(
                    "DOCTYPE name={name} public_id={public_id} system_id={system_id} force_quirks={force_quirks}"
                )
            }
            html::html5::Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                let name = ctx
                    .atoms
                    .resolve(*name)
                    .ok_or_else(|| format!("unknown atom id in start tag: {name:?}"))?;
                let mut line = String::new();
                line.push_str("START name=");
                line.push_str(name);
                line.push_str(" attrs=[");
                for (i, attr) in attributes.iter().enumerate() {
                    if i > 0 {
                        line.push(' ');
                    }
                    line.push_str(&format_attr(attr, resolver, ctx)?);
                }
                line.push_str("] self_closing=");
                line.push_str(if *self_closing { "true" } else { "false" });
                line
            }
            html::html5::Token::EndTag { name } => {
                let name = ctx
                    .atoms
                    .resolve(*name)
                    .ok_or_else(|| format!("unknown atom id in end tag: {name:?}"))?;
                format!("END name={name}")
            }
            html::html5::Token::Comment { text } => {
                let text = resolver
                    .resolve_span(*text)
                    .ok_or_else(|| "invalid text span (comment)".to_string())?;
                format!("COMMENT text=\"{}\"", escape_text(text))
            }
            html::html5::Token::Character { span } => {
                let text = resolver
                    .resolve_span(*span)
                    .ok_or_else(|| "invalid text span (char)".to_string())?;
                format!("CHAR text=\"{}\"", escape_text(text))
            }
            html::html5::Token::Eof => "EOF".to_string(),
        };
        out.push(line);
    }
    Ok(out)
}

fn format_attr(
    attr: &html::html5::Attribute,
    resolver: &impl html::html5::TextResolver,
    ctx: &DocumentParseContext,
) -> Result<String, String> {
    let name = ctx
        .atoms
        .resolve(attr.name)
        .ok_or_else(|| format!("unknown atom id in attribute: {:?}", attr.name))?;
    match &attr.value {
        None => Ok(name.to_string()),
        Some(html::html5::AttributeValue::Span(span)) => {
            let value = resolver
                .resolve_span(*span)
                .ok_or_else(|| "invalid attribute value span".to_string())?;
            Ok(format!("{name}=\"{}\"", escape_text(value)))
        }
        Some(html::html5::AttributeValue::Owned(value)) => {
            Ok(format!("{name}=\"{}\"", escape_text(value)))
        }
    }
}

fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => {
                use std::fmt::Write;
                let _ = write!(&mut out, "\\u{{{:02X}}}", ch as u32);
            }
            _ => out.push(ch),
        }
    }
    out
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
        (
            "finish",
            TokenizeResult::EmittedEof | TokenizeResult::Progress | TokenizeResult::NeedMoreInput,
        ) => Ok(()),
        ("push_input", TokenizeResult::NeedMoreInput | TokenizeResult::Progress) => Ok(()),
        _ => Err(format!(
            "unexpected tokenizer state stage={stage} result={result:?}"
        )),
    }
}

fn diff_lines(expected: &[String], actual: &[String]) -> String {
    let max = expected.len().max(actual.len());
    let mut out = String::new();
    use std::fmt::Write;
    let mut mismatch = None;
    for i in 0..max {
        let left = expected.get(i).map(String::as_str).unwrap_or("<none>");
        let right = actual.get(i).map(String::as_str).unwrap_or("<none>");
        if left != right {
            mismatch = Some(i);
            break;
        }
    }
    if let Some(i) = mismatch {
        let start = i.saturating_sub(2);
        let end = (i + 3).min(max);
        let _ = writeln!(
            &mut out,
            "first mismatch at line {} (showing {}..={}):",
            i + 1,
            start + 1,
            end
        );
        for line_idx in start..end {
            let left = expected
                .get(line_idx)
                .map(String::as_str)
                .unwrap_or("<none>");
            let right = actual.get(line_idx).map(String::as_str).unwrap_or("<none>");
            let marker = if line_idx == i { ">" } else { " " };
            let _ = writeln!(&mut out, "{marker} {:>4}  expected: {left}", line_idx + 1);
            let _ = writeln!(&mut out, "{marker} {:>4}    actual: {right}", line_idx + 1);
        }
    }
    if expected.len() != actual.len() && mismatch.is_none() {
        let _ = writeln!(
            &mut out,
            "prefix matched but lengths differ (expected {} lines, actual {} lines)",
            expected.len(),
            actual.len()
        );
    }
    let _ = writeln!(
        &mut out,
        "expected {} lines, actual {} lines",
        expected.len(),
        actual.len()
    );
    out
}
