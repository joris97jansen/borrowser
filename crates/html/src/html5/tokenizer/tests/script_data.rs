use super::helpers::{
    assert_script_data_chunk_invariant, assert_text_mode_split_close_tag_regression,
    run_script_data_chunks, run_script_data_chunks_with_errors,
};

const ESCAPED_SCRIPT_SAMPLE: &str = "<script><!--\nif (window.x) {\n  document.write(\"<script>nested</script>\");\n}\n//--></script>";

fn assert_script_family_transition_marker_is_split_safe(
    input: &str,
    target: &'static str,
    label: &'static str,
) {
    assert!(
        target.is_ascii(),
        "script-family marker targets must be ASCII"
    );
    let (whole, _) = run_script_data_chunks(&[input]);
    let start = input
        .find(target)
        .unwrap_or_else(|| panic!("target '{target}' must exist in {label}"));
    let end = start + target.len();

    for offset in 1..target.len() {
        let split = start + offset;
        let (chunked, _) = run_script_data_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "script-family marker '{target}' in {label} must be split-safe at offset={offset}"
        );
    }

    let mut bytewise_chunks = Vec::<&str>::with_capacity(target.len() + 2);
    if start > 0 {
        bytewise_chunks.push(&input[..start]);
    }
    for idx in start..end {
        bytewise_chunks.push(&input[idx..idx + 1]);
    }
    if end < input.len() {
        bytewise_chunks.push(&input[end..]);
    }

    let (bytewise, _) = run_script_data_chunks(&bytewise_chunks);
    assert_eq!(
        bytewise, whole,
        "script-family marker '{target}' in {label} must be bytewise split-safe"
    );
}

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
fn script_data_attribute_like_end_tag_closes_like_html() {
    let input = "<script>a</script type=text/plain>b</script>";
    assert_script_data_chunk_invariant(input);
    let (tokens, _) = run_script_data_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a\"".to_string(),
            "END name=script".to_string(),
            "CHAR text=\"b\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_slash_bearing_end_tag_closes_like_html() {
    let input = "<script>a</script/>b</script>";
    assert_script_data_chunk_invariant(input);
    let (tokens, _) = run_script_data_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"a\"".to_string(),
            "END name=script".to_string(),
            "CHAR text=\"b\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_attribute_like_end_tag_records_parse_error() {
    let (_, _, errors) =
        run_script_data_chunks_with_errors(&["<script>a</script type=text/plain>"]);
    assert!(
        errors
            .iter()
            .any(|error| { error.detail == Some("text-mode-end-tag-attributes-ignored") })
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

#[test]
fn script_data_escaped_family_keeps_nested_script_literal_until_final_close() {
    assert_script_data_chunk_invariant(ESCAPED_SCRIPT_SAMPLE);
    let (tokens, _) = run_script_data_chunks(&[ESCAPED_SCRIPT_SAMPLE]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"<!--\\nif (window.x) {\\n  document.write(\\\"<script>nested</script>\\\");\\n}\\n//-->\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_double_escaped_inner_script_close_does_not_close_outer_script() {
    let input = "<script><!--<script>nested</script>//--></script>";
    assert_script_data_chunk_invariant(input);
    let (tokens, _) = run_script_data_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"<!--<script>nested</script>//-->\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_escaped_near_miss_script_open_stays_literal_until_real_close() {
    let input = "<script><!--<scriptx>nested</script></script>";
    assert_script_data_chunk_invariant(input);
    let (tokens, _) = run_script_data_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"<!--<scriptx>nested\"".to_string(),
            "END name=script".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_double_escaped_near_miss_script_close_stays_literal_until_real_exit() {
    let input = "<script><!--<script>x</scriptx>y</script>//--></script>";
    assert_script_data_chunk_invariant(input);
    let (tokens, _) = run_script_data_chunks(&[input]);
    assert_eq!(
        tokens,
        vec![
            "START name=script attrs=[] self_closing=false".to_string(),
            "CHAR text=\"<!--<script>x</scriptx>y</script>//-->\"".to_string(),
            "END name=script".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn script_data_escaped_comment_start_is_split_safe_including_bytewise_growth() {
    let input = "<script><!--x--></script>";
    assert_script_family_transition_marker_is_split_safe(input, "<!--", "escaped-comment-start");
}

#[test]
fn script_data_double_escape_entry_and_exit_are_split_safe_including_bytewise_growth() {
    let input = "<script><!--<script>nested</script>//--></script>";
    for target in ["<script>", "</script>", "-->"] {
        assert_script_family_transition_marker_is_split_safe(
            input,
            target,
            "double-escape-entry-exit",
        );
    }
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

// Regression for G8: attribute-bearing script end tags must terminate the
// element across chunk splits inside the candidate tail.
#[test]
fn g8_regression_script_data_attribute_like_close_tag_splits_at_every_boundary() {
    let input = "<script>a</script type=text/plain>b</script>";
    assert_text_mode_split_close_tag_regression(
        run_script_data_chunks,
        input,
        "</script type=text/plain>",
        &[
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"a\"",
            "END name=script",
            "CHAR text=\"b\"",
            "END name=script",
            "EOF",
        ],
        "G8",
        "script-data-attribute-like-close-tag",
    );
}

#[test]
fn g8_regression_script_data_slash_bearing_close_tag_splits_at_every_boundary() {
    let input = "<script>a</script/>b</script>";
    assert_text_mode_split_close_tag_regression(
        run_script_data_chunks,
        input,
        "</script/>",
        &[
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"a\"",
            "END name=script",
            "CHAR text=\"b\"",
            "END name=script",
            "EOF",
        ],
        "G8",
        "script-data-slash-bearing-close-tag",
    );
}
