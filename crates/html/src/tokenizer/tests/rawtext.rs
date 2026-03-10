use super::super::scan::{SCRIPT_CLOSE_TAG, STYLE_CLOSE_TAG, find_rawtext_close_tag_counted};
use super::super::{Tokenizer, tokenize};
use super::helpers::text_eq;
use crate::types::Token;

#[test]
fn tokenize_finds_script_end_tag_case_insensitive() {
    let stream = tokenize("<script>let x = 1;</ScRiPt>");
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                if atoms.resolve(*name) == "script"
                    && text_eq(&stream, body, "let x = 1;")
                    && atoms.resolve(*end) == "script"
        ),
        "expected raw script text and matching end tag, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_large_rawtext_body_without_pathological_slowdown() {
    let mut body = String::new();
    for _ in 0..100_000 {
        body.push_str("let x = 1; < not a tag\n");
    }
    let input = format!("<script>{}</ScRiPt>", body);
    let stream = tokenize(&input);
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                if atoms.resolve(*name) == "script"
                    && text_eq(&stream, text, &body)
                    && atoms.resolve(*end) == "script"
        ),
        "expected large rawtext body to tokenize correctly, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_dense_near_match_rawtext_body() {
    let mut body = String::new();
    for _ in 0..50_000 {
        body.push_str("</scripX>");
    }
    let input = format!("<script>{}</ScRiPt>", body);
    let stream = tokenize(&input);
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                if atoms.resolve(*name) == "script"
                    && text_eq(&stream, text, &body)
                    && atoms.resolve(*end) == "script"
        ),
        "expected dense rawtext body to tokenize correctly, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_dense_near_match_style_rawtext_body() {
    let mut body = String::new();
    for _ in 0..50_000 {
        body.push_str("</stylX>");
    }
    let input = format!("<style>{}</StYle>", body);
    let stream = tokenize(&input);
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                if atoms.resolve(*name) == "style"
                    && text_eq(&stream, text, &body)
                    && atoms.resolve(*end) == "style"
        ),
        "expected dense style rawtext body to tokenize correctly, got: {stream:?}"
    );
}

#[test]
fn tokenize_allows_whitespace_before_rawtext_close_gt() {
    let stream = tokenize("<script>let x=1;</script >");
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                if atoms.resolve(*name) == "script"
                    && text_eq(&stream, body, "let x=1;")
                    && atoms.resolve(*end) == "script"
        ),
        "expected script end tag with whitespace before >, got: {stream:?}"
    );
}

#[test]
fn tokenize_allows_whitespace_before_rawtext_close_gt_case_insensitive() {
    let stream = tokenize("<style>body{}</STYLE\t>");
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                if atoms.resolve(*name) == "style"
                    && text_eq(&stream, body, "body{}")
                    && atoms.resolve(*end) == "style"
        ),
        "expected style end tag with whitespace before >, got: {stream:?}"
    );
}

#[test]
fn rawtext_close_tag_does_not_accept_near_matches() {
    let stream = tokenize("<script>ok</scriptx >no</script >");
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, body, Token::EndTag(end)]
                if atoms.resolve(*name) == "script"
                    && text_eq(&stream, body, "ok</scriptx >no")
                    && atoms.resolve(*end) == "script"
        ),
        "expected near-match not to close rawtext, got: {stream:?}"
    );
}

#[test]
fn rawtext_scan_steps_dense_near_match_is_linear() {
    let mut body = String::new();
    for _ in 0..50_000 {
        body.push_str("</scripX>");
    }
    let input = format!("{}</script>", body);
    let (found, steps) = find_rawtext_close_tag_counted(&input, SCRIPT_CLOSE_TAG);
    assert!(found.is_some(), "expected to find </script> close tag");
    let max_steps = input.len().saturating_mul(3);
    assert!(
        steps <= max_steps,
        "expected linear scan steps; steps={steps} len={} max={max_steps}",
        input.len()
    );
}

#[test]
fn rawtext_scan_steps_many_angle_brackets_is_linear() {
    let body = "<".repeat(200_000);
    let (found, steps) = find_rawtext_close_tag_counted(&body, SCRIPT_CLOSE_TAG);
    assert!(
        found.is_none(),
        "unexpected close tag in angle-bracket body"
    );
    let max_steps = body.len().saturating_mul(3);
    assert!(
        steps <= max_steps,
        "expected linear scan steps; steps={steps} len={} max={max_steps}",
        body.len()
    );
}

#[test]
fn rawtext_scan_steps_missing_close_is_linear() {
    let mut body = String::new();
    for _ in 0..100_000 {
        body.push_str("let x = 1; < not a tag\n");
    }
    let (found, steps) = find_rawtext_close_tag_counted(&body, STYLE_CLOSE_TAG);
    assert!(found.is_none(), "unexpected close tag in rawtext body");
    let max_steps = body.len().saturating_mul(3);
    assert!(
        steps <= max_steps,
        "expected linear scan steps; steps={steps} len={} max={max_steps}",
        body.len()
    );
}

#[test]
fn rawtext_scan_steps_many_slash_prefixes_is_linear() {
    let mut body = String::new();
    for _ in 0..80_000 {
        body.push_str("</s");
    }
    body.push_str("</script>");
    let (found, steps) = find_rawtext_close_tag_counted(&body, SCRIPT_CLOSE_TAG);
    assert!(found.is_some(), "expected to find </script> close tag");
    let max_steps = body.len().saturating_mul(3);
    assert!(
        steps <= max_steps,
        "expected linear scan steps; steps={steps} len={} max={max_steps}",
        body.len()
    );
}

#[test]
fn rawtext_scan_steps_many_scri_prefixes_is_linear() {
    let mut body = String::new();
    for _ in 0..60_000 {
        body.push_str("</scri");
    }
    body.push_str("</script>");
    let (found, steps) = find_rawtext_close_tag_counted(&body, SCRIPT_CLOSE_TAG);
    assert!(found.is_some(), "expected to find </script> close tag");
    let max_steps = body.len().saturating_mul(3);
    assert!(
        steps <= max_steps,
        "expected linear scan steps; steps={steps} len={} max={max_steps}",
        body.len()
    );
}

#[test]
fn rawtext_scan_steps_many_slash_brackets_is_linear() {
    let mut body = String::new();
    for _ in 0..80_000 {
        body.push_str("</");
    }
    body.push_str("</script>");
    let (found, steps) = find_rawtext_close_tag_counted(&body, SCRIPT_CLOSE_TAG);
    assert!(found.is_some(), "expected to find </script> close tag");
    let max_steps = body.len().saturating_mul(3);
    assert!(
        steps <= max_steps,
        "expected linear scan steps; steps={steps} len={} max={max_steps}",
        body.len()
    );
}

#[test]
fn rawtext_streaming_dense_near_match_is_linear() {
    let mut body = String::new();
    for _ in 0..20_000 {
        body.push_str("</scripX>");
    }
    let input = format!("<script>{}</script>", body);
    let bytes = input.as_bytes();
    let mut tokenizer = Tokenizer::new();
    tokenizer.reset_rawtext_scan_steps();
    let mut offset = 0usize;
    let chunk = 3usize;
    while offset < bytes.len() {
        let end = (offset + chunk).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        offset = end;
    }
    tokenizer.finish();
    let steps = tokenizer.rawtext_scan_steps();
    let max_steps = bytes.len().saturating_mul(5);
    assert!(
        steps <= max_steps,
        "expected linear scan steps in streaming; steps={steps} len={} max={max_steps}",
        bytes.len()
    );
}

#[test]
fn rawtext_streaming_many_angle_brackets_is_linear() {
    let body = "<".repeat(100_000);
    let input = format!("<script>{}", body);
    let bytes = input.as_bytes();
    let mut tokenizer = Tokenizer::new();
    tokenizer.reset_rawtext_scan_steps();
    let mut offset = 0usize;
    let chunk = 2usize;
    while offset < bytes.len() {
        let end = (offset + chunk).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        offset = end;
    }
    tokenizer.finish();
    let steps = tokenizer.rawtext_scan_steps();
    let max_steps = bytes.len().saturating_mul(5);
    assert!(
        steps <= max_steps,
        "expected linear scan steps in streaming; steps={steps} len={} max={max_steps}",
        bytes.len()
    );
}

#[test]
fn rawtext_streaming_close_tag_boundary_is_linear() {
    let body = "x".repeat(50_000);
    let input = format!("<script>{}</script>", body);
    let bytes = input.as_bytes();
    let mut tokenizer = Tokenizer::new();
    tokenizer.reset_rawtext_scan_steps();
    let tail = SCRIPT_CLOSE_TAG.len() + 5;
    let head_len = bytes.len().saturating_sub(tail);
    if head_len > 0 {
        tokenizer.feed(&bytes[..head_len]);
    }
    let mut offset = head_len;
    while offset < bytes.len() {
        let end = (offset + 1).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        offset = end;
    }
    tokenizer.finish();
    let steps = tokenizer.rawtext_scan_steps();
    let max_steps = bytes.len().saturating_mul(5);
    assert!(
        steps <= max_steps,
        "expected linear scan steps on close-tag boundary; steps={steps} len={} max={max_steps}",
        bytes.len()
    );
}

#[test]
fn tokenize_handles_rawtext_without_close_tag() {
    let mut body = String::new();
    for _ in 0..100_000 {
        body.push_str("x<y>\n");
    }
    let input = format!("<script>{}", body);
    let stream = tokenize(&input);
    let atoms = stream.atoms();
    assert!(
        matches!(
            stream.tokens(),
            [Token::StartTag { name, .. }, text, Token::EndTag(end)]
                if atoms.resolve(*name) == "script"
                    && text_eq(&stream, text, &body)
                    && atoms.resolve(*end) == "script"
        ),
        "expected rawtext body without close tag to tokenize correctly, got: {stream:?}"
    );
}
