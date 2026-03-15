use super::helpers::{
    assert_text_mode_split_close_tag_regression, assert_textarea_rcdata_chunk_invariant,
    assert_title_rcdata_chunk_invariant, run_textarea_rcdata_chunks, run_title_rcdata_chunks,
};

#[test]
fn rcdata_title_split_end_tag_is_chunk_invariant_at_every_boundary() {
    let input = "<title>Tom &amp; Jerry <b></title>";
    let (whole, _) = run_title_rcdata_chunks(&[input]);
    for offset in 1.."</title>".len() {
        let split = input.len() - "</title>".len() + offset;
        let (chunked, _) = run_title_rcdata_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "title rcdata close-tag detection must be split-safe at offset={offset}"
        );
    }
    assert_eq!(
        whole,
        vec![
            "START name=title attrs=[] self_closing=false".to_string(),
            "CHAR text=\"Tom & Jerry <b>\"".to_string(),
            "END name=title".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_textarea_decodes_character_references_and_keeps_other_end_tags_literal() {
    let (tokens, _) =
        run_textarea_rcdata_chunks(&["<textarea>A&amp;B</title>&lt;x&gt;</textarea>"]);
    assert_eq!(
        tokens,
        vec![
            "START name=textarea attrs=[] self_closing=false".to_string(),
            "CHAR text=\"A&B</title><x>\"".to_string(),
            "END name=textarea".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_title_end_tag_match_is_ascii_case_insensitive_and_allows_html_space() {
    let input = "<title>x</TiTlE \t\r\n>";
    assert_title_rcdata_chunk_invariant(input);
    let (tokens, _) = run_title_rcdata_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=title attrs=[] self_closing=false".to_string(),
            "CHAR text=\"x\"".to_string(),
            "END name=title".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_title_attribute_like_end_tag_remains_literal_until_plain_close() {
    let input = "<title>a</title class=x>b</title>";
    assert_title_rcdata_chunk_invariant(input);
    let (tokens, _) = run_title_rcdata_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=title attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</title class=x>b\"".to_string(),
            "END name=title".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_textarea_slash_bearing_end_tag_remains_literal_until_plain_close() {
    let input = "<textarea>a</textarea/>b</textarea>";
    assert_textarea_rcdata_chunk_invariant(input);
    let (tokens, _) = run_textarea_rcdata_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=textarea attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</textarea/>b\"".to_string(),
            "END name=textarea".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_textarea_handles_character_reference_split_across_chunks() {
    let whole = run_textarea_rcdata_chunks(&["<textarea>Tom &amp; Jerry</textarea>"]).0;
    let split = run_textarea_rcdata_chunks(&["<textarea>Tom &am", "p; Jerry</textarea>"]).0;
    assert_eq!(split, whole);
    assert_eq!(
        whole,
        vec![
            "START name=textarea attrs=[] self_closing=false".to_string(),
            "CHAR text=\"Tom & Jerry\"".to_string(),
            "END name=textarea".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_textarea_incomplete_close_tail_at_eof_is_literal_text() {
    assert_textarea_rcdata_chunk_invariant("<textarea>a</text");
    let (tokens, _) = run_textarea_rcdata_chunks(&["<textarea>a</text"]);
    assert_eq!(
        tokens,
        vec![
            "START name=textarea attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</text\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_title_incomplete_character_reference_at_eof_is_literal_and_chunk_invariant() {
    let input = "<title>&am";
    assert_title_rcdata_chunk_invariant(input);
    let (tokens, _) = run_title_rcdata_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=title attrs=[] self_closing=false".to_string(),
            "CHAR text=\"&am\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rcdata_title_large_near_miss_input_remains_linear() {
    let repeats = 16_384usize;
    let raw_body = "</titlex>&amp;".repeat(repeats);
    let html = format!("<title>{raw_body}</title>");
    let (tokens, stats) = run_title_rcdata_chunks(&[&html]);

    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], "START name=title attrs=[] self_closing=false");
    assert_eq!(tokens[2], "END name=title");
    assert_eq!(tokens[3], "EOF");
    assert!(tokens[1].contains("</titlex>&"));
    assert!(stats.steps <= (repeats as u64 * 4) + 64);
    assert!(stats.text_mode_end_tag_matcher_starts <= repeats as u64 + 1);
    assert_eq!(stats.text_mode_end_tag_matcher_resumes, 0);
    assert!(
        stats.text_mode_end_tag_match_progress_bytes
            <= (repeats as u64 * b"</title".len() as u64) + 16
    );
}

// Regression for G11: split RCDATA close tags with trailing HTML space once
// failed to terminate cleanly when chunked inside the close-tag tail.
#[test]
fn g11_regression_rcdata_title_whitespace_close_tag_splits_at_every_boundary() {
    let input = "<title>x</title \t\r\n>";
    assert_text_mode_split_close_tag_regression(
        run_title_rcdata_chunks,
        input,
        "</title \t\r\n>",
        &[
            "START name=title attrs=[] self_closing=false",
            "CHAR text=\"x\"",
            "END name=title",
            "EOF",
        ],
        "G11",
        "rcdata-title-whitespace-close-tag",
    );
}

// Regression for G11: attribute-like close-tag noise in RCDATA must stay text
// across every split inside the noisy candidate until the later plain close.
#[test]
fn g11_regression_rcdata_title_attribute_like_noise_splits_at_every_boundary() {
    let input = "<title>a</title class=x>b</title>";
    assert_text_mode_split_close_tag_regression(
        run_title_rcdata_chunks,
        input,
        "</title class=x>",
        &[
            "START name=title attrs=[] self_closing=false",
            "CHAR text=\"a</title class=x>b\"",
            "END name=title",
            "EOF",
        ],
        "G11",
        "rcdata-title-attribute-like-noise",
    );
}
