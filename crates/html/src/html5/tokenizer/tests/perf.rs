use super::helpers::{
    assert_push_ok, drain_all_fmt, run_script_data_chunks, run_style_rawtext_chunks,
    run_title_rcdata_chunks,
};
use crate::html5::shared::{DocumentParseContext, Input, TextValue, Token};
use crate::html5::tokenizer::{
    Html5Tokenizer, MAX_STEPS_PER_PUMP, TextModeSpec, TokenFmt, TokenizeResult, TokenizerConfig,
    TokenizerControl,
};
use html_test_support::escape_text;

const ADVERSARIAL_SCRIPT_UNIT: &str =
    "<<</scriX></scrip></scriptx><</scr!pt>if(a<b){c<<=1;value=left<right;}\n";

#[test]
fn long_comment_processing_is_linearish() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let payload = "x".repeat(20_000);
    let source = format!("<!--{payload}-->");
    let expected_max_steps = (source.len() as u64) * 3;

    let mut input = Input::new();
    input.push_str(&source);
    loop {
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert!(tokenizer.stats().steps <= expected_max_steps);
    assert!(tokenizer.stats().bytes_consumed <= source.len() as u64);
    assert!(tokenizer.stats().bytes_consumed <= tokenizer.cursor as u64);
}

#[test]
fn long_doctype_tail_processing_is_linearish() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let public_id = "P".repeat(16_000);
    let system_id = "S".repeat(16_000);
    let source = format!("<!DOCTYPE html PUBLIC \"{public_id}\" \"{system_id}\">");
    let expected_max_steps = (source.len() as u64) * 4;

    let mut input = Input::new();
    input.push_str(&source);
    loop {
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert!(tokenizer.stats().steps <= expected_max_steps);
    assert!(tokenizer.stats().bytes_consumed <= source.len() as u64);
}

#[test]
fn steady_state_text_and_comment_tokens_use_spans() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let source = format!(
        "{}<!--{}-->{}",
        "a".repeat(1024),
        "b".repeat(1024),
        "c".repeat(1024)
    );
    input.push_str(&source);

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);

    let mut saw_text = 0usize;
    let mut saw_comment = 0usize;
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        for token in batch.iter() {
            match token {
                Token::Text { text } => {
                    saw_text += 1;
                    assert!(matches!(text, TextValue::Span(_)));
                }
                Token::Comment { text } => {
                    saw_comment += 1;
                    assert!(matches!(text, TextValue::Span(_)));
                }
                _ => {}
            }
        }
    }
    assert!(saw_text >= 2);
    assert_eq!(saw_comment, 1);
}

#[test]
fn tokenizer_stats_counters_are_sane_and_observable() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let source = "<!DOCTYPE html><!--x--><div a=1>b</div>";
    input.push_str(source);

    loop {
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);

    let stats = tokenizer.stats();
    assert!(stats.steps > 0);
    assert!(stats.state_transitions > 0);
    assert!(stats.tokens_emitted >= 5);
    assert!(stats.bytes_consumed <= source.len() as u64);
}

#[test]
fn large_mixed_input_smoke_completes_and_emits_eof() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();

    let unit = "<!DOCTYPE html><div a=1>hello<!--x-->world</div>";
    let source = unit.repeat(5_000);
    input.push_str(&source);

    let mut pump_count = 0usize;
    loop {
        pump_count += 1;
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
        assert!(pump_count < 100_000);
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tail = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert!(tail.last().map(String::as_str) == Some("EOF"));
    assert!(tokenizer.stats().steps <= (source.len() as u64) * 12);
    assert_eq!(tokenizer.stats().bytes_consumed, source.len() as u64);
    let expected_max_budget_exhaustions =
        (source.len() as u64 / MAX_STEPS_PER_PUMP as u64).saturating_add(8);
    assert!(tokenizer.stats().budget_exhaustions <= expected_max_budget_exhaustions);
}

#[test]
fn multi_mb_script_data_with_many_angle_brackets_remains_linearish() {
    let unit = "if (a < b && c << 1) { value = left < right; }\n";
    let repeats = 48_000usize;
    let body = unit.repeat(repeats);
    let html = format!("<script>{body}</script>");
    let lt_count = body.as_bytes().iter().filter(|byte| **byte == b'<').count() as u64;

    let (tokens, stats) = run_script_data_chunks(&[&html]);

    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], "START name=script attrs=[] self_closing=false");
    assert_eq!(tokens[2], "END name=script");
    assert_eq!(tokens[3], "EOF");
    assert!(tokens[1].contains("c << 1"));
    assert!(stats.steps <= (html.len() as u64) * 4);
    assert!(stats.text_mode_end_tag_matcher_starts <= lt_count + 1);
    assert_eq!(stats.text_mode_end_tag_matcher_resumes, 0);
    assert!(stats.text_mode_end_tag_match_progress_bytes <= (lt_count * 2) + 16);
}

#[test]
fn adversarial_script_near_miss_storm_keeps_match_progress_linear_under_multi_mb_random_chunking() {
    let repeats = 32_768usize;
    let (html, lt_count) = adversarial_script_html(repeats);
    let whole = run_script_data_ascii_chunk_sizes(&html, &[html.len().max(1)]);
    let random_sizes = deterministic_random_chunk_sizes(html.len(), 0x0005_EED5_EEDA_11CE_u64, 64);
    let random = run_script_data_ascii_chunk_sizes(&html, &random_sizes);

    assert_script_storm_output(&whole.tokens, repeats);
    assert_eq!(random.tokens, whole.tokens);
    assert!(
        random.stats.text_mode_end_tag_matcher_starts <= (lt_count * 2) + 16,
        "random chunking may cause short prefix reprobes at chunk edges, but matcher starts must remain linearly bounded"
    );
    let random_extra_starts = random
        .stats
        .text_mode_end_tag_matcher_starts
        .saturating_sub(whole.stats.text_mode_end_tag_matcher_starts);
    assert!(
        random.stats.text_mode_end_tag_match_progress_bytes
            <= whole.stats.text_mode_end_tag_match_progress_bytes + random_extra_starts + 16,
        "random chunking may re-probe a tiny prefix at chunk edges, but matcher progress must not inflate beyond that linear overhead"
    );
    assert!(
        random.stats.text_mode_end_tag_matcher_resumes > 0,
        "random chunking should force matcher resumes on dense near-miss tails"
    );
    assert!(whole.stats.text_mode_end_tag_matcher_starts <= lt_count + 1);
    assert!(whole.stats.text_mode_end_tag_match_progress_bytes <= (lt_count * 9) + 16);
    assert!(whole.stats.steps <= (html.len() as u64) * 6);
    assert!(random.stats.steps <= (html.len() as u64) * 14);
}

#[test]
fn adversarial_script_near_miss_storm_does_not_restart_under_one_byte_chunking() {
    let repeats = 8_192usize;
    let (html, lt_count) = adversarial_script_html(repeats);
    let whole = run_script_data_ascii_chunk_sizes(&html, &[html.len().max(1)]);
    let bytewise = run_script_data_ascii_chunk_sizes(&html, &[1]);

    assert_script_storm_output(&whole.tokens, repeats);
    assert_eq!(bytewise.tokens, whole.tokens);
    assert!(
        bytewise.stats.text_mode_end_tag_matcher_starts <= (lt_count * 3) + 16,
        "1-byte chunking may cause short prefix reprobes at chunk edges, but matcher starts must remain linearly bounded"
    );
    let bytewise_extra_starts = bytewise
        .stats
        .text_mode_end_tag_matcher_starts
        .saturating_sub(whole.stats.text_mode_end_tag_matcher_starts);
    assert!(
        bytewise.stats.text_mode_end_tag_match_progress_bytes
            <= whole.stats.text_mode_end_tag_match_progress_bytes + bytewise_extra_starts + 16,
        "1-byte chunking may re-probe a tiny prefix at chunk edges, but matcher progress must not inflate beyond that linear overhead"
    );
    assert!(
        bytewise.stats.text_mode_end_tag_matcher_resumes > 0,
        "1-byte chunking should exercise matcher resume paths on near-miss tails"
    );
    assert!(whole.stats.text_mode_end_tag_matcher_starts <= lt_count + 1);
    assert!(whole.stats.text_mode_end_tag_match_progress_bytes <= (lt_count * 9) + 16);
    assert!(whole.stats.steps <= (html.len() as u64) * 6);
    assert!(bytewise.stats.steps <= (html.len() as u64) * 20);
}

#[test]
fn text_mode_end_tag_matcher_resumes_without_restart_across_many_tail_extensions() {
    let trailing_spaces = 8_192usize;
    let mut chunks = Vec::<String>::with_capacity(trailing_spaces + 2);
    chunks.push("<script>x</script".to_string());
    for _ in 0..trailing_spaces {
        chunks.push(" ".to_string());
    }
    chunks.push(">".to_string());
    let chunk_refs = chunks.iter().map(String::as_str).collect::<Vec<_>>();

    let (script_tokens, script_stats) = run_script_data_chunks(&chunk_refs);
    assert_eq!(
        script_tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"x\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(script_stats.text_mode_end_tag_matcher_starts, 1);
    assert!(
        script_stats.text_mode_end_tag_matcher_resumes > trailing_spaces as u64
            && script_stats.text_mode_end_tag_matcher_resumes <= trailing_spaces as u64 + 2
    );
    assert!(
        script_stats.text_mode_end_tag_match_progress_bytes
            <= trailing_spaces as u64 + b"</script>".len() as u64 + 8
    );

    chunks[0] = "<style>x</style".to_string();
    let chunk_refs = chunks.iter().map(String::as_str).collect::<Vec<_>>();
    let (style_tokens, style_stats) = run_style_rawtext_chunks(&chunk_refs);
    assert_eq!(
        style_tokens,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"x\"".to_string(),
            "END name=style".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(style_stats.text_mode_end_tag_matcher_starts, 1);
    assert!(
        style_stats.text_mode_end_tag_matcher_resumes > trailing_spaces as u64
            && style_stats.text_mode_end_tag_matcher_resumes <= trailing_spaces as u64 + 2
    );
    assert!(
        style_stats.text_mode_end_tag_match_progress_bytes
            <= trailing_spaces as u64 + b"</style>".len() as u64 + 8
    );

    chunks[0] = "<title>x</title".to_string();
    let chunk_refs = chunks.iter().map(String::as_str).collect::<Vec<_>>();
    let (title_tokens, title_stats) = run_title_rcdata_chunks(&chunk_refs);
    assert_eq!(
        title_tokens,
        vec![
            "START name=title attrs=[] self_closing=false".to_string(),
            "CHAR text=\"x\"".to_string(),
            "END name=title".to_string(),
            "EOF".to_string(),
        ]
    );
    assert_eq!(title_stats.text_mode_end_tag_matcher_starts, 1);
    assert!(
        title_stats.text_mode_end_tag_matcher_resumes > trailing_spaces as u64
            && title_stats.text_mode_end_tag_matcher_resumes <= trailing_spaces as u64 + 2
    );
    assert!(
        title_stats.text_mode_end_tag_match_progress_bytes
            <= trailing_spaces as u64 + b"</title>".len() as u64 + 8
    );
}

struct TextModePerfRun {
    tokens: Vec<String>,
    stats: crate::html5::tokenizer::TokenizerStats,
}

fn adversarial_script_html(repeats: usize) -> (String, u64) {
    let body = ADVERSARIAL_SCRIPT_UNIT.repeat(repeats);
    let html = format!("<script>{body}</script>");
    let lt_count = body.as_bytes().iter().filter(|byte| **byte == b'<').count() as u64;
    (html, lt_count)
}

fn deterministic_random_chunk_sizes(
    total_len: usize,
    seed: u64,
    max_chunk_len: usize,
) -> Vec<usize> {
    assert!(max_chunk_len > 0, "max_chunk_len must be > 0");
    let mut remaining = total_len;
    let mut state = seed;
    let mut sizes = Vec::new();
    while remaining > 0 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let next = 1 + ((state >> 32) as usize % max_chunk_len);
        let size = next.min(remaining);
        sizes.push(size);
        remaining -= size;
    }
    sizes
}

fn run_script_data_ascii_chunk_sizes(input: &str, chunk_sizes: &[usize]) -> TextModePerfRun {
    assert!(
        input.is_ascii(),
        "adversarial script perf input must stay ASCII"
    );
    assert!(
        !chunk_sizes.is_empty() && chunk_sizes.iter().all(|size| *size > 0),
        "chunk-size plan must contain only positive sizes"
    );

    let mut ctx = DocumentParseContext::new();
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
        .expect("script atom interning must succeed in perf harness");
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input_buf = Input::new();
    let mut out = Vec::new();
    let mut text_mode_active = false;
    let mut offset = 0usize;
    let mut size_index = 0usize;

    while offset < input.len() {
        let chunk_len = chunk_sizes[size_index % chunk_sizes.len()];
        let end = (offset + chunk_len).min(input.len());
        input_buf.push_str(&input[offset..end]);
        offset = end;
        size_index = size_index.saturating_add(1);

        loop {
            let res = tokenizer.push_input_until_token(&mut input_buf, &mut ctx);
            assert_push_ok(res);
            let mut pending_control = None;
            {
                let batch = tokenizer.next_batch(&mut input_buf);
                if !batch.tokens().is_empty() {
                    assert_eq!(
                        batch.tokens().len(),
                        1,
                        "script perf harness must observe exactly one token per pump"
                    );
                    let resolver = batch.resolver();
                    let fmt = TokenFmt::new(&ctx.atoms, &resolver);
                    let token = batch
                        .iter()
                        .next()
                        .expect("non-empty script perf batch must contain one token");
                    out.push(
                        fmt.format_token(token).expect(
                            "token formatting in script perf harness must be deterministic",
                        ),
                    );
                    match token {
                        Token::StartTag { name, .. } if *name == script && !text_mode_active => {
                            text_mode_active = true;
                            pending_control = Some(TokenizerControl::EnterTextMode(
                                TextModeSpec::script_data(script),
                            ));
                        }
                        Token::EndTag { name } if *name == script && text_mode_active => {
                            text_mode_active = false;
                            pending_control = Some(TokenizerControl::ExitTextMode);
                        }
                        _ => {}
                    }
                }
            }
            if let Some(control) = pending_control {
                tokenizer.apply_control(control);
            }
            if matches!(res, TokenizeResult::NeedMoreInput) {
                break;
            }
        }
    }

    assert_eq!(tokenizer.finish(&input_buf), TokenizeResult::EmittedEof);
    out.extend(drain_all_fmt(&mut tokenizer, &mut input_buf, &ctx));
    TextModePerfRun {
        tokens: out,
        stats: tokenizer.stats(),
    }
}

fn assert_script_storm_output(tokens: &[String], repeats: usize) {
    let expected_text = ADVERSARIAL_SCRIPT_UNIT.repeat(repeats);
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], "START name=script attrs=[] self_closing=false");
    assert_eq!(
        tokens[1],
        format!("CHAR text=\"{}\"", escape_text(&expected_text))
    );
    assert_eq!(tokens[2], "END name=script");
    assert_eq!(tokens[3], "EOF");
}

#[test]
fn data_flushes_text_before_tag_in_same_pump() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("Hello<div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    let first_batch = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        first_batch,
        vec![
            "CHAR text=\"Hello\"".to_string(),
            "START name=div attrs=[] self_closing=false".to_string(),
        ]
    );
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tail = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tail, vec!["EOF".to_string()]);
}
