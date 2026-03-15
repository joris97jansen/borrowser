use super::helpers::{
    assert_style_rawtext_chunk_invariant, assert_text_mode_split_close_tag_regression,
    run_style_rawtext_chunks,
};

#[test]
fn rawtext_style_split_end_tag_is_chunk_invariant_at_every_boundary() {
    let input = "<style>body{content:\"&amp;\";}<b></style>";
    let (whole, _) = run_style_rawtext_chunks(&[input]);
    for offset in 1.."</style>".len() {
        let split = input.len() - "</style>".len() + offset;
        let (chunked, _) = run_style_rawtext_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "style rawtext close-tag detection must be split-safe at offset={offset}"
        );
    }
    assert_eq!(
        whole,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"body{content:\\\"&amp;\\\";}<b>\"".to_string(),
            "END name=style".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rawtext_style_preserves_character_references_literally() {
    let (tokens, _) = run_style_rawtext_chunks(&["<style>&amp;&lt;&gt;</style>"]);
    assert_eq!(
        tokens,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"&amp;&lt;&gt;\"".to_string(),
            "END name=style".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rawtext_style_end_tag_match_is_ascii_case_insensitive_and_allows_html_space() {
    let input = "<style>x</StYlE \t\r\n>";
    assert_style_rawtext_chunk_invariant(input);
    let (tokens, _) = run_style_rawtext_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"x\"".to_string(),
            "END name=style".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rawtext_style_mismatched_end_tag_is_emitted_as_text_until_matching_close() {
    let (tokens, _) = run_style_rawtext_chunks(&["<style>a</title>b</style>"]);
    assert_eq!(
        tokens,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</title>b\"".to_string(),
            "END name=style".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rawtext_style_attribute_like_end_tag_remains_literal_until_plain_close() {
    let input = "<style>a</style class=x>b</style>";
    assert_style_rawtext_chunk_invariant(input);
    let (tokens, _) = run_style_rawtext_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</style class=x>b\"".to_string(),
            "END name=style".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rawtext_style_incomplete_close_tail_at_eof_is_literal_text() {
    assert_style_rawtext_chunk_invariant("<style>a</sty");
    let (tokens, _) = run_style_rawtext_chunks(&["<style>a</sty"]);
    assert_eq!(
        tokens,
        vec![
            "START name=style attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</sty\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn rawtext_style_large_near_miss_input_remains_linear() {
    let repeats = 16_384usize;
    let raw_body = "</stylex>".repeat(repeats);
    let html = format!("<style>{raw_body}</style>");
    let (tokens, stats) = run_style_rawtext_chunks(&[&html]);

    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], "START name=style attrs=[] self_closing=false");
    assert_eq!(tokens[2], "END name=style");
    assert_eq!(tokens[3], "EOF");
    assert!(tokens[1].contains("</stylex>"));
    assert!(stats.steps <= (repeats as u64 * 3) + 64);
    assert!(stats.text_mode_end_tag_matcher_starts <= repeats as u64 + 1);
    assert_eq!(stats.text_mode_end_tag_matcher_resumes, 0);
    assert!(
        stats.text_mode_end_tag_match_progress_bytes
            <= (repeats as u64 * b"</style".len() as u64) + 16
    );
}

#[test]
fn rawtext_style_prefix_near_misses_remain_literal_and_chunk_invariant() {
    for tail in ["<<<<<<<<<<<", "</s", "</st", "</stylex", "</StyleX"] {
        let input = format!("<style>{tail}</style>");
        assert_style_rawtext_chunk_invariant(&input);
        let (tokens, stats) = run_style_rawtext_chunks(&[&input]);
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[1], format!("CHAR text=\"{tail}\""));
        assert!(stats.steps <= (tail.len() as u64 * 4) + 32);
    }
}

// Regression for G11: split RAWTEXT close tags with trailing HTML space once
// failed to terminate correctly when chunked inside the close-tag tail.
#[test]
fn g11_regression_rawtext_style_whitespace_close_tag_splits_at_every_boundary() {
    let input = "<style>x</style \t\r\n>";
    assert_text_mode_split_close_tag_regression(
        run_style_rawtext_chunks,
        input,
        "</style \t\r\n>",
        &[
            "START name=style attrs=[] self_closing=false",
            "CHAR text=\"x\"",
            "END name=style",
            "EOF",
        ],
        "G11",
        "rawtext-style-whitespace-close-tag",
    );
}

// Regression for G11: attribute-like close-tag noise in RAWTEXT must stay text
// across every split inside the noisy candidate until a later plain close tag.
#[test]
fn g11_regression_rawtext_style_attribute_like_noise_splits_at_every_boundary() {
    let input = "<style>a</style class=x>b</style>";
    assert_text_mode_split_close_tag_regression(
        run_style_rawtext_chunks,
        input,
        "</style class=x>",
        &[
            "START name=style attrs=[] self_closing=false",
            "CHAR text=\"a</style class=x>b\"",
            "END name=style",
            "EOF",
        ],
        "G11",
        "rawtext-style-attribute-like-noise",
    );
}
