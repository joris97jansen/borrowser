use crate::html5::tokenizer::scan::{ScriptTagBoundaryMatch, match_script_tag_boundary_at};

fn assert_boundary_growth(candidate: &[u8], closing: bool) {
    for split in 1..candidate.len() {
        assert_eq!(
            match_script_tag_boundary_at(&candidate[..split], 0, closing),
            ScriptTagBoundaryMatch::NeedMoreInput,
            "candidate {:?} should stay partial at split={split}",
            String::from_utf8_lossy(candidate)
        );
    }
    assert_eq!(
        match_script_tag_boundary_at(candidate, 0, closing),
        ScriptTagBoundaryMatch::Matched {
            cursor_after: candidate.len(),
        }
    );
}

#[test]
fn script_tag_boundary_matches_opening_delimiter_variants_incrementally() {
    assert_boundary_growth(b"<script>", false);
    assert_boundary_growth(b"<script/", false);
    assert_boundary_growth(b"<script ", false);
}

#[test]
fn script_tag_boundary_matches_closing_delimiter_variants_incrementally() {
    assert_boundary_growth(b"</script>", true);
    assert_boundary_growth(b"</script/", true);
    assert_boundary_growth(b"</script ", true);
}

#[test]
fn script_tag_boundary_rejects_near_miss_names() {
    for candidate in [b"<scriptx>".as_slice(), b"<script-".as_slice()] {
        assert_eq!(
            match_script_tag_boundary_at(candidate, 0, false),
            ScriptTagBoundaryMatch::NoMatch
        );
    }
    for candidate in [b"</scriptx>".as_slice(), b"</script-".as_slice()] {
        assert_eq!(
            match_script_tag_boundary_at(candidate, 0, true),
            ScriptTagBoundaryMatch::NoMatch
        );
    }
}

#[test]
fn script_tag_boundary_needs_more_input_for_incomplete_prefixes() {
    for candidate in [
        b"<".as_slice(),
        b"<scr".as_slice(),
        b"<script".as_slice(),
        b"</".as_slice(),
        b"</scr".as_slice(),
        b"</script".as_slice(),
    ] {
        let closing = candidate.starts_with(b"</");
        assert_eq!(
            match_script_tag_boundary_at(candidate, 0, closing),
            ScriptTagBoundaryMatch::NeedMoreInput
        );
    }
}
