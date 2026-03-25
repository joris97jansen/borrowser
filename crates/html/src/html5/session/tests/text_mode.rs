use super::super::Html5ParseSession;
#[cfg(feature = "dom-snapshot")]
use super::support::finish_session_to_dom_lines;
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::{TextModeSpec, TokenizerConfig};
use crate::html5::tree_builder::TreeBuilderConfig;
use crate::html5::tree_builder::modes::InsertionMode;

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
