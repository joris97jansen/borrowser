use super::helpers::{
    assert_text_mode_close_tag_boundary_exhaustive_at_occurrence, run_script_data_chunks,
    run_style_rawtext_chunks, run_textarea_rcdata_chunks, run_title_rcdata_chunks,
};

// L4 guard: an exact RAWTEXT </style> close tag must terminate identically when
// split at any byte inside the candidate, and exiting RAWTEXT must not buffer
// following data-mode text or later end tags.
#[test]
fn l4_rawtext_style_close_tag_matches_whole_input_at_every_boundary() {
    let input = "lead<style>alpha<beta</style>tail</style>";
    assert_text_mode_close_tag_boundary_exhaustive_at_occurrence(
        run_style_rawtext_chunks,
        input,
        "</style>",
        0,
        &[
            "CHAR text=\"lead\"",
            "START name=style attrs=[] self_closing=false",
            "CHAR text=\"alpha<beta\"",
            "END name=style",
            "CHAR text=\"tail\"",
            "END name=style",
            "EOF",
        ],
        "l4-rawtext-style-exact-close-tag",
    );
}

// L4 guard: an exact RCDATA </textarea> close tag must match identically at
// every byte boundary while preserving RCDATA entity decoding before exit and
// resuming ordinary tokenization immediately after the close.
#[test]
fn l4_rcdata_textarea_close_tag_matches_whole_input_at_every_boundary() {
    let input = "lead<textarea>A&amp;B <x></textarea>tail</textarea>";
    assert_text_mode_close_tag_boundary_exhaustive_at_occurrence(
        run_textarea_rcdata_chunks,
        input,
        "</textarea>",
        0,
        &[
            "CHAR text=\"lead\"",
            "START name=textarea attrs=[] self_closing=false",
            "CHAR text=\"A&B <x>\"",
            "END name=textarea",
            "CHAR text=\"tail\"",
            "END name=textarea",
            "EOF",
        ],
        "l4-rcdata-textarea-exact-close-tag",
    );
}

// L4 guard: an exact RCDATA </title> close tag must match identically at every
// byte boundary, including when literal tag-like text and character references
// appear before the terminator.
#[test]
fn l4_rcdata_title_close_tag_matches_whole_input_at_every_boundary() {
    let input = "lead<title>Tom &amp; Jerry <b></title>tail</title>";
    assert_text_mode_close_tag_boundary_exhaustive_at_occurrence(
        run_title_rcdata_chunks,
        input,
        "</title>",
        0,
        &[
            "CHAR text=\"lead\"",
            "START name=title attrs=[] self_closing=false",
            "CHAR text=\"Tom & Jerry <b>\"",
            "END name=title",
            "CHAR text=\"tail\"",
            "END name=title",
            "EOF",
        ],
        "l4-rcdata-title-exact-close-tag",
    );
}

// L4 guard: an exact script </script> close tag must terminate identically at
// every byte boundary, and script-mode exit must not retain buffered candidate
// state that would corrupt following text or later end tags.
#[test]
fn l4_script_close_tag_matches_whole_input_at_every_boundary() {
    let input = "lead<script>if (a < b) c()</script>tail</script>";
    assert_text_mode_close_tag_boundary_exhaustive_at_occurrence(
        run_script_data_chunks,
        input,
        "</script>",
        0,
        &[
            "CHAR text=\"lead\"",
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"if (a < b) c()\"",
            "END name=script",
            "CHAR text=\"tail\"",
            "END name=script",
            "EOF",
        ],
        "l4-script-exact-close-tag",
    );
}
