use super::Html5ParseSession;
use crate::dom_patch::{DomPatch, DomPatchBatch, PatchKey};
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::{TextModeSpec, TokenizerConfig};
use crate::html5::tree_builder::TreeBuilderConfig;
use crate::html5::tree_builder::modes::InsertionMode;
use std::collections::BTreeMap;

#[cfg(feature = "dom-snapshot")]
fn finish_session_to_dom_lines(session: &mut Html5ParseSession) -> Vec<String> {
    session
        .finish_for_test()
        .expect("session finish should remain recoverable");
    let patches = session.take_patches();
    let dom = crate::test_harness::materialize_patch_batches(&[patches])
        .expect("session patches should materialize into a DOM");
    crate::html5::serialize_dom_for_test(&dom)
}

fn run_session_collect_patches(chunks: &[&str], context: &str) -> Vec<DomPatch> {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    let chunk_error = format!("{context} chunk should remain recoverable");
    for chunk in chunks {
        session.push_str_for_test(chunk);
        session.pump().expect(&chunk_error);
    }

    let finish_error = format!("{context} scenario should finish cleanly");
    session.finish_for_test().expect(&finish_error);
    session.take_patches()
}

fn create_count_by_key(patches: &[DomPatch]) -> BTreeMap<PatchKey, usize> {
    let mut counts = BTreeMap::new();
    for patch in patches {
        let key = match patch {
            DomPatch::CreateDocument { key, .. }
            | DomPatch::CreateElement { key, .. }
            | DomPatch::CreateText { key, .. }
            | DomPatch::CreateComment { key, .. } => *key,
            DomPatch::Clear
            | DomPatch::AppendChild { .. }
            | DomPatch::InsertBefore { .. }
            | DomPatch::RemoveNode { .. }
            | DomPatch::SetAttributes { .. }
            | DomPatch::SetText { .. }
            | DomPatch::AppendText { .. } => continue,
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn assert_no_remove_node_moves(patches: &[DomPatch], context: &str) {
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "{context} must use canonical AppendChild/InsertBefore moves rather than RemoveNode detaches"
    );
}

#[test]
fn session_smoke() {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");
    assert!(session.push_bytes(&[]).is_ok());
    assert!(session.pump().is_ok());
    let _ = session.take_patches();
    assert!(session.take_patch_batch().is_none());
    let counters = session.debug_counters();
    assert_eq!(counters.patches_emitted, 0);
    assert_eq!(counters.decode_errors, 0);
    assert_eq!(counters.adapter_invariant_violations, 0);
    assert_eq!(counters.tree_builder_invariant_errors, 0);
}

#[test]
fn session_patch_batches_are_version_monotonic_and_atomic() {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    // Empty drains must not create or advance batches.
    assert!(session.take_patch_batch().is_none());
    assert!(session.take_patch_batch().is_none());

    // First atomic batch.
    session.inject_patch_for_test(DomPatch::CreateDocument {
        key: PatchKey(1),
        doctype: None,
    });
    let batch0: DomPatchBatch = session
        .take_patch_batch()
        .expect("first injected patch should produce batch");
    assert_eq!(batch0.from, 0);
    assert_eq!(batch0.to, 1);
    assert_eq!(
        batch0.patches,
        vec![DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None
        }]
    );
    assert!(
        session.take_patch_batch().is_none(),
        "empty drain must not advance version"
    );

    // Second atomic batch.
    session.inject_patch_for_test(DomPatch::CreateComment {
        key: PatchKey(2),
        text: "x".to_string(),
    });
    let batch1: DomPatchBatch = session
        .take_patch_batch()
        .expect("second injected patch should produce batch");
    assert_eq!(batch1.from, 1);
    assert_eq!(batch1.to, 2);
    assert_eq!(
        batch1.patches,
        vec![DomPatch::CreateComment {
            key: PatchKey(2),
            text: "x".to_string()
        }]
    );
    assert!(
        session.take_patch_batch().is_none(),
        "empty drain must not advance version"
    );
}

#[test]
fn session_applies_text_mode_controls_across_chunk_boundaries() {
    let mut ctx = DocumentParseContext::new();
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><body><textarea>hel");
    session.pump().expect("first chunk should pump");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        Some(TextModeSpec::rcdata_textarea(textarea)),
        "start tag insertion must switch tokenizer into text mode before later chunks"
    );
    assert_eq!(
        session
            .tree_builder_state_snapshot_for_test()
            .insertion_mode,
        InsertionMode::Text,
        "builder should remain in text insertion mode while close tag is incomplete"
    );

    for chunk in ["lo<", "/", "t", "e", "x", "t"] {
        session.push_str_for_test(chunk);
        session.pump().expect("split close tag prefix should pump");
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            Some(TextModeSpec::rcdata_textarea(textarea)),
            "incomplete end tag across chunk boundaries must not exit text mode early"
        );
    }

    session.push_str_for_test("area>");
    session.pump().expect("final close tag chunk should pump");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        None,
        "matching end tag completion must reset tokenizer text mode"
    );
    assert_eq!(
        session
            .tree_builder_state_snapshot_for_test()
            .insertion_mode,
        InsertionMode::InBody,
        "builder should restore the original insertion mode after text-mode close"
    );
}

#[test]
fn session_keeps_text_mode_active_for_mismatched_end_tag() {
    let mut ctx = DocumentParseContext::new();
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><body><textarea>x</title>");
    session
        .pump()
        .expect("mismatched end tag sequence should remain recoverable");

    let builder_state = session.tree_builder_state_snapshot_for_test();
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        Some(TextModeSpec::rcdata_textarea(textarea)),
        "mismatched end tags must not exit the active text mode"
    );
    assert_eq!(
        builder_state.active_text_mode,
        Some(TextModeSpec::rcdata_textarea(textarea)),
        "builder should keep the exact active text-mode element"
    );
    assert_eq!(
        builder_state.insertion_mode,
        InsertionMode::Text,
        "mismatched end tags must keep the builder in text mode"
    );
}

#[test]
fn session_exits_script_text_mode_only_after_one_byte_close_tag_completion() {
    let mut ctx = DocumentParseContext::new();
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
        .expect("atom interning");
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><body><script>var x = 1;");
    session.pump().expect("script prelude should pump");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        Some(TextModeSpec::script_data(script)),
        "script start tag should enter script-data text mode"
    );

    for chunk in ["<", "/", "s", "c", "r", "i", "p", "t"] {
        session.push_str_for_test(chunk);
        session
            .pump()
            .expect("one-byte script close prefix should pump");
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            Some(TextModeSpec::script_data(script)),
            "script text mode must stay active until the full close tag has arrived"
        );
    }

    session.push_str_for_test(">");
    session
        .pump()
        .expect("final script close-tag byte should pump");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        None,
        "script text mode must exit only when </script> is complete"
    );
}

#[test]
fn session_head_script_restores_in_head_after_matching_close() {
    let mut ctx = DocumentParseContext::new();
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
        .expect("atom interning");
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><head><script>var x = 1;");
    session
        .pump()
        .expect("head script prelude should remain recoverable");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        Some(TextModeSpec::script_data(script)),
        "head-context script must enter script-data text mode"
    );
    assert_eq!(
        session
            .tree_builder_state_snapshot_for_test()
            .insertion_mode,
        InsertionMode::Text,
        "builder must switch to Text mode while a head-context script is active"
    );

    session.push_str_for_test("</script>");
    session
        .pump()
        .expect("head-context script close should remain recoverable");
    let builder_state = session.tree_builder_state_snapshot_for_test();
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        None,
        "matching </script> must clear tokenizer script-data mode in head context"
    );
    assert_eq!(
        builder_state.active_text_mode, None,
        "matching </script> must clear the builder active text-mode element in head context"
    );
    assert_eq!(
        builder_state.insertion_mode,
        InsertionMode::InHead,
        "closing a head-context script must restore the builder to InHead"
    );
}

#[test]
fn session_exits_text_mode_on_eof_recovery() {
    let mut ctx = DocumentParseContext::new();
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
        .expect("atom interning");
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><body><script>unfinished");
    session.pump().expect("script prelude should pump");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        Some(TextModeSpec::script_data(script)),
        "script start tag should enter script-data text mode before EOF"
    );

    session
        .finish_for_test()
        .expect("EOF recovery should finish cleanly");
    let builder_state = session.tree_builder_state_snapshot_for_test();
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        None,
        "EOF recovery must clear tokenizer text mode"
    );
    assert_eq!(
        builder_state.active_text_mode, None,
        "EOF recovery must clear the builder's active text-mode element"
    );
    assert_eq!(
        builder_state.insertion_mode,
        InsertionMode::InBody,
        "EOF recovery should restore the original insertion mode"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn session_self_closing_text_container_does_not_enter_text_mode() {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><body><textarea/>ok");
    session
        .pump()
        .expect("self-closing textarea syntax should remain recoverable");
    let builder_state = session.tree_builder_state_snapshot_for_test();
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        None,
        "self-closing text container syntax must not enter tokenizer text mode"
    );
    assert_eq!(
        builder_state.active_text_mode, None,
        "self-closing text container syntax must not enter builder text mode"
    );
    assert_eq!(
        builder_state.insertion_mode,
        InsertionMode::InBody,
        "self-closing text container syntax must leave the builder in surrounding body mode"
    );

    let lines = finish_session_to_dom_lines(&mut session);
    assert_eq!(
        lines,
        vec![
            "#document".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <textarea>".to_string(),
            "      \"ok\"".to_string(),
        ]
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn session_title_in_implicit_head_enters_rcdata_and_builds_expected_dom() {
    let mut ctx = DocumentParseContext::new();
    let title = ctx
        .atoms
        .intern_ascii_folded("title")
        .expect("atom interning");
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><title>Hello &amp; good");
    session.pump().expect("title prelude should pump");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        Some(TextModeSpec::rcdata_title(title)),
        "implicit-head title must enter RCDATA text mode"
    );

    session.push_str_for_test("bye</title><body><p>x</p>");
    session
        .pump()
        .expect("title close and body content should pump");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        None,
        "matching </title> must restore tokenizer data mode"
    );

    let lines = finish_session_to_dom_lines(&mut session);
    assert_eq!(
        lines,
        vec![
            "#document".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "      <title>".to_string(),
            "        \"Hello & goodbye\"".to_string(),
            "    <body>".to_string(),
            "      <p>".to_string(),
            "        \"x\"".to_string(),
        ]
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn session_style_rawtext_malformed_end_tag_does_not_get_stuck_and_builds_expected_dom() {
    let mut ctx = DocumentParseContext::new();
    let style = ctx
        .atoms
        .intern_ascii_folded("style")
        .expect("atom interning");
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<html><head><style>a</ti");
    session
        .pump()
        .expect("style rawtext prelude should remain recoverable");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        Some(TextModeSpec::rawtext_style(style)),
        "style start tag must enter RAWTEXT mode"
    );

    session.push_str_for_test("tle>b</style><body>ok");
    session
        .pump()
        .expect("malformed style close sequence should remain recoverable");
    assert_eq!(
        session.tokenizer_active_text_mode_for_test(),
        None,
        "matching </style> must clear RAWTEXT mode even after malformed inner endings"
    );
    assert_eq!(
        session
            .tree_builder_state_snapshot_for_test()
            .insertion_mode,
        InsertionMode::InBody,
        "builder must not stay stuck in Text mode after style close"
    );

    let lines = finish_session_to_dom_lines(&mut session);
    assert_eq!(
        lines,
        vec![
            "#document".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "      <style>".to_string(),
            "        \"a</title>b\"".to_string(),
            "    <body>".to_string(),
            "      \"ok\"".to_string(),
        ]
    );
}

#[test]
fn session_reconstruction_is_chunk_equivalent_after_generic_ancestor_pop() {
    let whole = run_session_collect_patches(
        &["<!doctype html><div><b class=\"x\">one</div>two"],
        "reconstruction",
    );
    let chunked = run_session_collect_patches(
        &[
            "<!doctype html><div><b class=\"x\">",
            "one</div>",
            "tw",
            "o",
        ],
        "reconstruction",
    );

    assert_eq!(
        whole, chunked,
        "reconstruction must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole reconstruction patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked reconstruction patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <div>".to_string(),
            "        <b class=\"x\">".to_string(),
            "          \"one\"".to_string(),
            "      <b class=\"x\">".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input reconstruction should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked reconstruction should build the expected DOM"
        );
    }
}

#[test]
fn session_reconstruction_of_multiple_missing_formatting_elements_is_chunk_equivalent() {
    let whole = run_session_collect_patches(
        &["<!doctype html><div><b class=\"x\"><i class=\"y\">one</div>two"],
        "multi-element reconstruction",
    );
    let chunked = run_session_collect_patches(
        &[
            "<!doctype html><div><b class=\"x\">",
            "<i class=\"y\">one",
            "</div>t",
            "wo",
        ],
        "multi-element reconstruction",
    );

    assert_eq!(
        whole, chunked,
        "multi-element reconstruction must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole multi-element reconstruction patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked multi-element reconstruction patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <div>".to_string(),
            "        <b class=\"x\">".to_string(),
            "          <i class=\"y\">".to_string(),
            "            \"one\"".to_string(),
            "      <b class=\"x\">".to_string(),
            "        <i class=\"y\">".to_string(),
            "          \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input multi-element reconstruction should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked multi-element reconstruction should build the expected DOM"
        );
    }
}

#[test]
fn session_special_anchor_recovery_is_chunk_equivalent() {
    let whole = run_session_collect_patches(&["<!doctype html><a>one<a>two"], "anchor recovery");
    let chunked =
        run_session_collect_patches(&["<!doctype html><a>", "one<a>", "two"], "anchor recovery");

    assert_eq!(
        whole, chunked,
        "special anchor recovery must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole anchor recovery patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked anchor recovery patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "        \"one\"".to_string(),
            "      <a>".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input special anchor recovery should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked special anchor recovery should build the expected DOM"
        );
    }
}

#[test]
fn session_special_nobr_recovery_is_chunk_equivalent() {
    let whole =
        run_session_collect_patches(&["<!doctype html><nobr>one<nobr>two"], "nobr recovery");
    let chunked = run_session_collect_patches(
        &["<!doctype html><nobr>", "one<nobr>", "two"],
        "nobr recovery",
    );

    assert_eq!(
        whole, chunked,
        "special nobr recovery must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole nobr recovery patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked nobr recovery patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <nobr>".to_string(),
            "        \"one\"".to_string(),
            "      <nobr>".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input special nobr recovery should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked special nobr recovery should build the expected DOM"
        );
    }
}

#[test]
fn session_integrated_aaa_misnested_formatting_is_chunk_equivalent() {
    let whole =
        run_session_collect_patches(&["<!doctype html><b><i>one</b>two</i>"], "AAA misnesting");
    let chunked = run_session_collect_patches(
        &["<!doctype html><b>", "<i>one</b>", "two</i>"],
        "AAA misnesting",
    );

    assert_eq!(
        whole, chunked,
        "integrated AAA end-tag dispatch must preserve exact patch order and key allocation across chunking"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole AAA misnesting patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked AAA misnesting patches should materialize");

        let whole_lines = crate::html5::serialize_dom_for_test(&whole_dom);
        let chunked_lines = crate::html5::serialize_dom_for_test(&chunked_dom);
        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <b>".to_string(),
            "        <i>".to_string(),
            "          \"one\"".to_string(),
            "      <i>".to_string(),
            "        \"two\"".to_string(),
        ];

        assert_eq!(
            whole_lines, expected_lines,
            "whole-input integrated AAA misnesting should build the expected DOM"
        );
        assert_eq!(
            chunked_lines, expected_lines,
            "chunked integrated AAA misnesting should build the expected DOM"
        );
    }
}

#[test]
fn session_aaa_furthest_block_reparenting_is_chunk_equivalent() {
    let whole = run_session_collect_patches(
        &["<!doctype html><a><p>one</a>"],
        "AAA furthest-block reparenting",
    );
    let chunked = run_session_collect_patches(
        &["<!doctype html><a>", "<p>one</a>"],
        "AAA furthest-block reparenting",
    );

    assert_eq!(
        whole, chunked,
        "AAA furthest-block reparenting must preserve exact patch order and key allocation across chunking"
    );

    let create_counts = create_count_by_key(&whole);
    assert_no_remove_node_moves(&whole, "AAA furthest-block reparenting");
    assert_eq!(
        create_counts.get(&PatchKey(6)),
        Some(&1),
        "moved furthest block should retain its original PatchKey"
    );
    assert_eq!(
        create_counts.get(&PatchKey(7)),
        Some(&1),
        "moved text should retain its original PatchKey"
    );
    assert!(
        whole.contains(&DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(6),
        }),
        "AAA furthest-block reparenting must emit the canonical AppendChild move"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole AAA furthest-block patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked AAA furthest-block patches should materialize");

        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "      <p>".to_string(),
            "        <a>".to_string(),
            "          \"one\"".to_string(),
        ];

        assert_eq!(
            crate::html5::serialize_dom_for_test(&whole_dom),
            expected_lines,
            "whole-input AAA furthest-block reparenting should build the expected DOM"
        );
        assert_eq!(
            crate::html5::serialize_dom_for_test(&chunked_dom),
            expected_lines,
            "chunked AAA furthest-block reparenting should build the expected DOM"
        );
    }
}

#[test]
fn session_aaa_foster_parent_insert_before_is_chunk_equivalent() {
    let whole = run_session_collect_patches(
        &["<!doctype html><table><a><tr>x</a>"],
        "AAA foster-parent reparenting",
    );
    let chunked = run_session_collect_patches(
        &["<!doctype html><table><a>", "<tr>x</a>"],
        "AAA foster-parent reparenting",
    );

    assert_eq!(
        whole, chunked,
        "AAA foster-parent reparenting must preserve exact patch order and key allocation across chunking"
    );

    let create_counts = create_count_by_key(&whole);
    assert_no_remove_node_moves(&whole, "AAA foster-parent reparenting");
    assert_eq!(
        create_counts.get(&PatchKey(9)),
        Some(&1),
        "foster-parented node should retain its original PatchKey"
    );
    assert_eq!(
        create_counts.get(&PatchKey(10)),
        Some(&1),
        "moved text should retain its original PatchKey"
    );
    assert!(
        whole.contains(&DomPatch::InsertBefore {
            parent: PatchKey(4),
            child: PatchKey(9),
            before: PatchKey(5),
        }),
        "AAA foster-parent reparenting must emit the canonical InsertBefore move"
    );

    #[cfg(feature = "dom-snapshot")]
    {
        let whole_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
                .expect("whole AAA foster-parent patches should materialize");
        let chunked_dom =
            crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
                .expect("chunked AAA foster-parent patches should materialize");

        let expected_lines = vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "      <a>".to_string(),
            "      \"x\"".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <tr>".to_string(),
        ];

        assert_eq!(
            crate::html5::serialize_dom_for_test(&whole_dom),
            expected_lines,
            "whole-input AAA foster-parent reparenting should build the expected DOM"
        );
        assert_eq!(
            crate::html5::serialize_dom_for_test(&chunked_dom),
            expected_lines,
            "chunked AAA foster-parent reparenting should build the expected DOM"
        );
    }
}

#[test]
fn session_noahs_ark_keeps_active_formatting_depth_bounded_for_duplicate_flood() {
    let mut html = String::from("<!doctype html>");
    html.push_str(&"<b>".repeat(64));

    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test(&html);
    session
        .pump()
        .expect("duplicate formatting flood should remain recoverable");
    session
        .finish_for_test()
        .expect("duplicate formatting flood should finish cleanly");

    let counters = session.debug_counters();
    assert_eq!(
        counters.max_active_formatting_depth, 3,
        "Noah's Ark duplicate trimming should keep AFE depth bounded to the newest three matching entries"
    );
}
