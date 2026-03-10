use super::capacity::estimate_token_capacity;
use super::scan::{
    SCRIPT_CLOSE_TAG, STYLE_CLOSE_TAG, clamp_char_boundary, find_rawtext_close_tag_counted,
};
use super::{Tokenizer, tokenize};
use crate::types::{AttributeValue, Token, TokenStream};
#[cfg(feature = "perf-tests")]
use std::time::{Duration, Instant};

fn text_eq(stream: &TokenStream, token: &Token, expected: &str) -> bool {
    stream.text(token) == Some(expected)
}

fn tokenize_in_chunks(input: &str, sizes: &[usize]) -> TokenStream {
    let bytes = input.as_bytes();
    let mut tokenizer = Tokenizer::new();
    let mut offset = 0usize;
    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        offset = end;
    }
    if offset < bytes.len() {
        tokenizer.feed(&bytes[offset..]);
    }
    tokenizer.finish();
    tokenizer.into_stream()
}

fn tokenize_with_push_str(input: &str, sizes: &[usize]) -> TokenStream {
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::with_capacity(estimate_token_capacity(input.len()));
    let mut offset = 0usize;
    for size in sizes {
        if offset >= input.len() {
            break;
        }
        let end = (offset + size).min(input.len());
        let end = clamp_char_boundary(input, end, offset);
        if end == offset {
            break;
        }
        tokenizer.push_str_into(&input[offset..end], &mut tokens);
        offset = end;
    }
    if offset < input.len() {
        tokenizer.push_str_into(&input[offset..], &mut tokens);
    }
    tokenizer.finish_into(&mut tokens);
    let (atoms, source, text_pool) = tokenizer.into_parts();
    TokenStream::new(tokens, atoms, source, text_pool)
}

fn tokenize_with_feed_bytes(bytes: &[u8], split: usize) -> TokenStream {
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::with_capacity(estimate_token_capacity(bytes.len()));
    tokenizer.feed(&bytes[..split]);
    tokenizer.drain_into(&mut tokens);
    tokenizer.feed(&bytes[split..]);
    tokenizer.finish();
    tokenizer.drain_into(&mut tokens);
    let (atoms, source, text_pool) = tokenizer.into_parts();
    TokenStream::new(tokens, atoms, source, text_pool)
}

#[test]
fn tokenize_preserves_utf8_text_nodes() {
    let stream = tokenize("<p>120×32</p>");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "120×32")),
        "expected UTF-8 text token, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_uppercase_doctype() {
    let stream = tokenize("<!DOCTYPE html>");
    assert!(
        stream.iter().any(|t| matches!(t, Token::Doctype(s)
                if stream.payload_text(s) == "DOCTYPE html")),
        "expected case-insensitive doctype, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_mixed_case_doctype() {
    let stream = tokenize("<!DoCtYpE html>");
    assert!(
        stream.iter().any(|t| matches!(t, Token::Doctype(s)
                if stream.payload_text(s) == "DoCtYpE html")),
        "expected mixed-case doctype to parse, got: {stream:?}"
    );
}

#[test]
fn tokenize_trims_doctype_whitespace_with_utf8() {
    let stream = tokenize("<!DOCTYPE  café  >");
    assert!(
        stream.iter().any(|t| matches!(t, Token::Doctype(s)
                if stream.payload_text(s) == "DOCTYPE  café")),
        "expected trimmed doctype payload, got: {stream:?}"
    );
}

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
fn tokenize_handles_non_ascii_text_around_tags() {
    let stream = tokenize("¡Hola <b>café</b> 😊");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "¡Hola ")),
        "expected leading UTF-8 text token, got: {stream:?}"
    );
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "café")),
        "expected UTF-8 text inside tag, got: {stream:?}"
    );
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, " 😊")),
        "expected trailing UTF-8 text token, got: {stream:?}"
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
fn tokenize_accepts_end_tag_with_trailing_junk() {
    let stream = tokenize("</div foo>");
    let atoms = stream.atoms();
    assert!(
        matches!(stream.tokens(), [Token::EndTag(name)] if atoms.resolve(*name) == "div"),
        "expected end tag to ignore trailing junk, got: {stream:?}"
    );
}

#[test]
fn tokenize_accepts_attributes_after_invalid_name_char() {
    let stream = tokenize("<div @id=one></div>");
    let atoms = stream.atoms();
    assert!(
        stream.iter().any(|t| matches!(
            t,
            Token::StartTag { name, attributes, .. }
                if atoms.resolve(*name) == "div"
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "id"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("one")
                    })
        )),
        "expected permissive attribute parsing after invalid name char, got: {stream:?}"
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
    // Explicit reset for clarity in case more counters are added later.
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
fn tokenize_handles_non_ascii_attribute_values() {
    let stream = tokenize("<p data=naïve>ok</p>");
    let atoms = stream.atoms();
    assert!(
        stream.iter().any(|t| matches!(
            t,
            Token::StartTag { name, attributes, .. }
                if atoms.resolve(*name) == "p"
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "data"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("naïve")
                    })
        )),
        "expected UTF-8 attribute value, got: {stream:?}"
    );
}

#[test]
fn tokenize_decodes_entities_in_unquoted_attributes() {
    let stream = tokenize("<p data=Tom&amp;Jerry title=&#x3C;ok&#x3E;>ok</p>");
    let atoms = stream.atoms();
    assert!(
        stream.iter().any(|t| matches!(
            t,
            Token::StartTag { name, attributes, .. }
                if atoms.resolve(*name) == "p"
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "data"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("Tom&Jerry")
                    })
                    && attributes.iter().any(|(k, v)| {
                        atoms.resolve(*k) == "title"
                            && v.as_ref().map(|v| stream.attr_value(v)) == Some("<ok>")
                    })
        )),
        "expected entity-decoded unquoted attributes, got: {stream:?}"
    );
}

#[test]
fn tokenize_attribute_values_use_span_when_unchanged() {
    let stream = tokenize("<p data=plain title=\"also-plain\" data-empty=>ok</p>");
    let atoms = stream.atoms();
    let mut spans = 0usize;
    for token in stream.iter() {
        if let Token::StartTag {
            name, attributes, ..
        } = token
            && atoms.resolve(*name) == "p"
        {
            for (key, value) in attributes {
                let key_name = atoms.resolve(*key);
                if (key_name.starts_with("data") || key_name == "title")
                    && matches!(value, Some(AttributeValue::Span { .. }))
                {
                    spans += 1;
                }
            }
        }
    }
    assert!(
        spans >= 2,
        "expected unchanged attribute values to use spans, got {spans}"
    );
}

#[test]
fn tokenize_attribute_values_allocate_when_decoded() {
    let stream = tokenize("<p data=Tom&amp;Jerry>ok</p>");
    let atoms = stream.atoms();
    let mut owned = 0usize;
    for token in stream.iter() {
        if let Token::StartTag {
            name, attributes, ..
        } = token
            && atoms.resolve(*name) == "p"
        {
            for (key, value) in attributes {
                if atoms.resolve(*key) == "data" && matches!(value, Some(AttributeValue::Owned(_)))
                {
                    owned += 1;
                }
            }
        }
    }
    assert!(
        owned >= 1,
        "expected decoded attribute value to allocate, got {owned}"
    );
}

#[test]
fn tokenize_text_preserves_literal_ampersand() {
    let stream = tokenize("<p>Tom&Jerry</p>");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "Tom&Jerry")),
        "expected literal '&' text to remain unchanged, got: {stream:?}"
    );
}

#[test]
fn tokenize_text_decodes_entities() {
    let stream = tokenize("<p>Tom&amp;Jerry</p>");
    assert!(
        stream.iter().any(|t| text_eq(&stream, t, "Tom&Jerry")),
        "expected entity-decoded text, got: {stream:?}"
    );
    assert!(
        stream.iter().any(|t| matches!(t, Token::TextOwned { .. })),
        "expected decoded text to be owned, got: {stream:?}"
    );
}

#[test]
fn tokenize_text_preserves_malformed_entities() {
    let stream = tokenize("<p>Tom&amp</p><p>&#xZZ;</p><p>&unknown;</p>");
    let texts: Vec<&str> = stream.iter().filter_map(|t| stream.text(t)).collect();
    assert!(
        texts.contains(&"Tom&amp"),
        "expected incomplete entity to remain unchanged, got: {texts:?}"
    );
    assert!(
        texts.contains(&"&#xZZ;"),
        "expected malformed numeric entity to remain unchanged, got: {texts:?}"
    );
    assert!(
        texts.contains(&"&unknown;"),
        "expected unknown entity to remain unchanged, got: {texts:?}"
    );
}

#[test]
fn tokenize_handles_utf8_adjacent_to_angle_brackets() {
    let stream = tokenize("é<b>ï</b>ö");
    assert!(stream.iter().any(|t| text_eq(&stream, t, "é")));
    assert!(stream.iter().any(|t| text_eq(&stream, t, "ï")));
    assert!(stream.iter().any(|t| text_eq(&stream, t, "ö")));
}

#[test]
fn tokenize_interns_case_insensitive_tag_and_attr_names() {
    let stream = tokenize("<DiV id=one></div><div ID=two></DIV>");
    let atoms = stream.atoms();
    let mut div_ids = Vec::new();
    let mut id_ids = Vec::new();

    for token in stream.iter() {
        match token {
            Token::StartTag {
                name, attributes, ..
            } => {
                div_ids.push(*name);
                for (attr_name, _) in attributes {
                    id_ids.push(*attr_name);
                }
            }
            Token::EndTag(name) => div_ids.push(*name),
            _ => {}
        }
    }

    assert!(
        div_ids.windows(2).all(|w| w[0] == w[1]),
        "expected all div atoms to match, got: {div_ids:?}"
    );
    assert!(
        id_ids.windows(2).all(|w| w[0] == w[1]),
        "expected all id atoms to match, got: {id_ids:?}"
    );
    assert_eq!(atoms.resolve(div_ids[0]), "div");
    assert_eq!(atoms.resolve(id_ids[0]), "id");
    assert_eq!(atoms.len(), 2, "expected only two interned names");
}

#[test]
fn tokenize_allows_custom_element_and_namespaced_tags() {
    let stream = tokenize("<my-component></my-component><svg:rect></svg:rect>");
    let atoms = stream.atoms();
    let mut names = Vec::new();

    for token in stream.iter() {
        match token {
            Token::StartTag { name, .. } | Token::EndTag(name) => names.push(*name),
            _ => {}
        }
    }

    assert_eq!(atoms.resolve(names[0]), "my-component");
    assert_eq!(atoms.resolve(names[1]), "my-component");
    assert_eq!(atoms.resolve(names[2]), "svg:rect");
    assert_eq!(atoms.resolve(names[3]), "svg:rect");
}

#[test]
fn tokenize_handles_many_simple_tags_linearly() {
    let mut input = String::new();
    for _ in 0..20_000 {
        input.push_str("<a></a>");
    }
    let stream = tokenize(&input);
    assert_eq!(stream.tokens().len(), 40_000);
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

#[test]
fn tokenize_handles_many_comments_and_doctypes() {
    let mut input = String::new();
    for _ in 0..5_000 {
        input.push_str("<!--x-->");
    }
    for _ in 0..5_000 {
        input.push_str("<!DOCTYPE html>");
    }

    let stream = tokenize(&input);
    let mut comment_count = 0;
    let mut doctype_count = 0;
    for token in stream.iter() {
        match token {
            Token::Comment(_) => comment_count += 1,
            Token::Doctype(_) => doctype_count += 1,
            _ => {}
        }
    }

    assert_eq!(comment_count, 5_000);
    assert_eq!(doctype_count, 5_000);
}

#[test]
fn tokenize_does_not_emit_empty_text_tokens() {
    let stream = tokenize("<p></p>");
    assert!(
        !stream
            .tokens()
            .iter()
            .any(|t| matches!(t, Token::TextSpan { .. } | Token::TextOwned { .. })),
        "expected no text tokens for empty element, got: {stream:?}"
    );
}

#[test]
fn tokenize_handles_tons_of_angle_brackets() {
    let input = "<".repeat(200_000);
    let stream = tokenize(&input);
    assert!(stream.tokens().len() <= input.len());
}

#[test]
fn tokenize_incremental_matches_full_for_small_chunks() {
    let input = "<!DOCTYPE html><!--c--><div class=one>Hi &amp; \
                     <script>let x = 1;</script><style>p{}</style>é</div>";
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[1, 2, 3, 7, 64]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_matches_full_for_small_chunks() {
    let input = "<!DOCTYPE html><!--c--><div class=one>Hi &amp; \
                     <script>let x = 1;</script><style>p{}</style>é</div>";
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[1, 2, 3, 7, 64]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_matches_full_for_utf8_splits() {
    let input = "<p>café 😊 &amp; naïve</p>";
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[1, 1, 1, 2, 1, 4, 1]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_script_end_tag() {
    let input = "<script>hi</script>";
    let split = "<script>hi</scr".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_script_end_tag() {
    let input = "<script>hi</script>";
    let split = "<script>hi</scr".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_end_tag_prefix() {
    let input = "<div></div>";
    let split = "<div></".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_tag_name() {
    let input = "<div>ok</div>";
    let split = "<d".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_comment_terminator() {
    let input = "<!--x-->";
    let split = "<!--x--".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_comment_terminator() {
    let input = "<!--x-->";
    let split = "<!--x--".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_comment_terminator_dash() {
    let input = "<!--x-->";
    let split = "<!--x-".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_comment_terminator_arrow() {
    let input = "<!--x-->";
    let split = "<!--x".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_doctype_end() {
    let input = "<!DOCTYPE html>";
    let split = "<!DOCTYPE html".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_doctype_end() {
    let input = "<!DOCTYPE html>";
    let split = "<!DOCTYPE html".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_attribute_name() {
    let input = "<p data-value=ok>hi</p>";
    let split = "<p da".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_attribute_value() {
    let input = "<p data=\"value\">ok</p>";
    let split = "<p data=\"va".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_rawtext_close_tag() {
    let input = "<style>body{}</style>";
    let split = "<style>body{}</sty".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_rawtext_close_tag_with_whitespace() {
    let input = "<style>body{}</style \t>";
    let split = "<style>body{}</style \t".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_fuzz_boundaries_matches_full() {
    let input = "<!DOCTYPE html><!--c--><div class=one data-x=\"y\">Hi &amp; é \
                     <script>let x = 1;</script><style>p{}</style></div>";
    let full = tokenize(input);
    let expected = crate::test_utils::token_snapshot(&full);

    for split in 0..=input.len() {
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(
            expected,
            crate::test_utils::token_snapshot(&chunked),
            "boundary split at {split} should match full tokenization"
        );
    }
}

#[test]
fn tokenize_feed_bytes_fuzz_boundaries_matches_full() {
    let input = "<!DOCTYPE html><!--c--><div class=one data-x=\"y\">Hi &amp; é \
                     <script>let x = 1;</script><style>p{}</style></div>";
    let full = tokenize(input);
    let expected = crate::test_utils::token_snapshot(&full);
    let bytes = input.as_bytes();

    for split in 0..=bytes.len() {
        let chunked = tokenize_with_feed_bytes(bytes, split);
        assert_eq!(
            expected,
            crate::test_utils::token_snapshot(&chunked),
            "byte boundary split at {split} should match full tokenization"
        );
    }
}

#[test]
fn tokenize_incremental_drain_view_matches_full() {
    let input = "<!DOCTYPE html><!--c--><div class=one>Tom&amp;Jerry\
                     <script>let x = 1;</script><style>p{}</style>é</div>";
    let full = tokenize(input);
    let expected = crate::test_utils::token_snapshot(&full);

    let bytes = input.as_bytes();
    let sizes = [1, 2, 3, 7, 64];
    let mut tokenizer = Tokenizer::new();
    let mut offset = 0usize;
    let mut drained = Vec::new();
    let mut snapshot = Vec::new();

    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        offset = end;
        drained.clear();
        tokenizer.drain_into(&mut drained);
        let view = tokenizer.view();
        snapshot.extend(crate::test_utils::token_snapshot_with_view(view, &drained));
    }

    if offset < bytes.len() {
        tokenizer.feed(&bytes[offset..]);
    }
    tokenizer.finish();
    drained.clear();
    tokenizer.drain_into(&mut drained);
    let view = tokenizer.view();
    snapshot.extend(crate::test_utils::token_snapshot_with_view(view, &drained));

    assert_eq!(expected, snapshot);
}

#[test]
fn streaming_does_not_reallocate_internal_tokens_pathologically() {
    let input = "<a></a>".repeat(50_000);
    let mut tokenizer = Tokenizer::new();
    let mut grows = 0usize;
    let mut last_cap = tokenizer.tokens_capacity();

    for b in input.as_bytes() {
        tokenizer.feed(std::slice::from_ref(b));
        let cap = tokenizer.tokens_capacity();
        if cap != last_cap {
            grows += 1;
            last_cap = cap;
        }
    }

    tokenizer.finish();

    assert!(grows <= 32, "too many internal token vec growths: {grows}");
}

#[test]
fn streaming_does_not_reallocate_internal_tokens_with_drains_pathologically() {
    let input = "<a></a>".repeat(50_000);
    let mut tokenizer = Tokenizer::new();
    let mut sink = Vec::new();
    let mut grows = 0usize;
    let mut last_cap = tokenizer.tokens_capacity();

    for b in input.as_bytes() {
        tokenizer.feed(std::slice::from_ref(b));
        tokenizer.drain_into(&mut sink);
        let cap = tokenizer.tokens_capacity();
        if cap != last_cap {
            grows += 1;
            last_cap = cap;
        }
    }

    tokenizer.finish();
    tokenizer.drain_into(&mut sink);

    assert!(
        grows <= 32,
        "too many internal token vec growths with drains: {grows}"
    );
}

#[cfg(feature = "perf-tests")]
#[test]
fn tokenize_scales_roughly_linearly_on_repeated_tags() {
    fn build_input(repeats: usize) -> String {
        let mut input = String::new();
        for _ in 0..repeats {
            input.push_str("<a></a>");
        }
        input
    }

    fn measure_total(input: &str) -> Duration {
        let _ = tokenize(input);
        let mut total = Duration::ZERO;
        for _ in 0..5 {
            let start = Instant::now();
            let _ = tokenize(input);
            total += start.elapsed();
        }
        total
    }

    let small = build_input(5_000);
    let large = build_input(20_000);

    let t_small = measure_total(&small);
    let t_large = measure_total(&large);
    assert!(!t_small.is_zero(), "timer resolution too coarse for test");
    // Allow generous slack to avoid flakiness while still catching quadratic regressions.
    assert!(
        t_large <= t_small.saturating_mul(12),
        "expected near-linear scaling; t_small={t_small:?} t_large={t_large:?}"
    );
}

#[cfg(feature = "perf-tests")]
#[test]
fn tokenize_scales_roughly_linearly_on_comment_scan() {
    fn build_input(repeats: usize, body_len: usize) -> String {
        let mut input = String::new();
        for _ in 0..repeats {
            input.push_str("<!--");
            input.extend(std::iter::repeat_n('-', body_len));
            input.push('x');
            input.push_str("-->");
        }
        input
    }

    fn measure_total(input: &str) -> Duration {
        let _ = tokenize(input);
        let mut total = Duration::ZERO;
        for _ in 0..5 {
            let start = Instant::now();
            let _ = tokenize(input);
            total += start.elapsed();
        }
        total
    }

    let small = build_input(500, 400);
    let large = build_input(2_000, 400);

    let t_small = measure_total(&small);
    let t_large = measure_total(&large);
    assert!(!t_small.is_zero(), "timer resolution too coarse for test");
    assert!(
        t_large <= t_small.saturating_mul(12),
        "expected near-linear comment scan; t_small={t_small:?} t_large={t_large:?}"
    );
}
