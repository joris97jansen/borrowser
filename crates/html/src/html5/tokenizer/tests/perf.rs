use super::helpers::{
    assert_push_ok, drain_all_fmt, run_script_data_chunks, run_style_rawtext_chunks,
    run_title_rcdata_chunks,
};
use crate::html5::shared::{DocumentParseContext, Input, TextValue, Token};
use crate::html5::tokenizer::{
    Html5Tokenizer, MAX_STEPS_PER_PUMP, TokenizeResult, TokenizerConfig,
};

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
