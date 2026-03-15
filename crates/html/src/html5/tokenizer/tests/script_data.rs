use super::helpers::{
    assert_script_data_chunk_invariant, assert_text_mode_split_close_tag_regression,
    run_script_data_chunks,
};

#[test]
fn script_data_split_end_tag_is_chunk_invariant_at_every_boundary() {
    let input = "<script>if (a < b) c()</script>";
    let (whole, _) = run_script_data_chunks(&[input]);
    for offset in 1.."</script>".len() {
        let split = input.len() - "</script>".len() + offset;
        let (chunked, _) = run_script_data_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "script-data close-tag detection must be split-safe at offset={offset}"
        );
    }
    assert_eq!(
        whole,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"if (a < b) c()\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_literal_end_tag_in_js_string_still_terminates_script() {
    let (tokens, _) = run_script_data_chunks(&["<script>var s='</script>';after()</script>"]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"var s='\"".to_string(),
            "END name=script".to_string(),
            "CHAR text=\"';after()\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_end_tag_match_is_ascii_case_insensitive_and_allows_html_space() {
    let input = "<script>x</ScRiPt \t\r\n>";
    assert_script_data_chunk_invariant(input);
    let (tokens, _) = run_script_data_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"x\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_mismatched_end_tag_is_emitted_as_text_until_matching_close() {
    let (tokens, _) = run_script_data_chunks(&["<script>a</style>b</script>"]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</style>b\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_attribute_like_end_tag_remains_literal_until_plain_close() {
    let input = "<script>a</script type=text/plain>b</script>";
    assert_script_data_chunk_invariant(input);
    let (tokens, _) = run_script_data_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</script type=text/plain>b\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_incomplete_close_tail_at_eof_is_literal_text() {
    assert_script_data_chunk_invariant("<script>a</scr");
    let (tokens, _) = run_script_data_chunks(&["<script>a</scr"]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a</scr\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_large_near_miss_input_remains_linear() {
    let repeats = 16_384usize;
    let raw_body = "</scriptx>".repeat(repeats);
    let html = format!("<script>{raw_body}</script>");
    let (tokens, stats) = run_script_data_chunks(&[&html]);

    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], "START name=script attrs=[] self_closing=false");
    assert_eq!(tokens[2], "END name=script");
    assert_eq!(tokens[3], "EOF");
    assert!(tokens[1].contains("</scriptx>"));
    assert!(stats.steps <= (repeats as u64 * 3) + 64);
    assert!(stats.text_mode_end_tag_matcher_starts <= repeats as u64 + 1);
    assert_eq!(stats.text_mode_end_tag_matcher_resumes, 0);
    assert!(
        stats.text_mode_end_tag_match_progress_bytes
            <= (repeats as u64 * b"</script".len() as u64) + 16
    );
}

// Regression for G11: split script-data close tags with trailing HTML space
// must still terminate exactly once at the real matching end tag.
#[test]
fn g11_regression_script_data_whitespace_close_tag_splits_at_every_boundary() {
    let input = "<script>x</script \t\r\n>";
    assert_text_mode_split_close_tag_regression(
        run_script_data_chunks,
        input,
        "</script \t\r\n>",
        &[
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"x\"",
            "END name=script",
            "EOF",
        ],
        "G11",
        "script-data-whitespace-close-tag",
    );
}

// Regression for G11: attribute-like script close-tag noise must remain text
// across chunk splits inside the noisy candidate until the later plain close.
#[test]
fn g11_regression_script_data_attribute_like_noise_splits_at_every_boundary() {
    let input = "<script>a</script type=text/plain>b</script>";
    assert_text_mode_split_close_tag_regression(
        run_script_data_chunks,
        input,
        "</script type=text/plain>",
        &[
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"a</script type=text/plain>b\"",
            "END name=script",
            "EOF",
        ],
        "G11",
        "script-data-attribute-like-noise",
    );
}
