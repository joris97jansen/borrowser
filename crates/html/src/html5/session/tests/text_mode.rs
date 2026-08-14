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

#[cfg(feature = "parser-conformance")]
#[test]
fn parser_observations_do_not_change_text_mode_tokenizer_controls() {
    use crate::html5::shared::{
        ErrorPolicy, ParseErrorCode, ParserObservationCapture, ParserObservationConfig,
        SurfaceCaptureRequest, TreeConstructionParseErrorCode, WhatwgParseErrorCode,
    };
    use crate::html5::tokenizer::{TextModeKind, TextModeNamespace, TokenizerControl};

    #[derive(Debug, PartialEq, Eq)]
    enum AppliedControl {
        EnterTextMode {
            kind: TextModeKind,
            end_tag_name: String,
            namespace: TextModeNamespace,
        },
        ExitTextMode,
    }

    struct Run {
        controls: Vec<AppliedControl>,
        active_text_mode: Option<TextModeSpec>,
        insertion_mode: InsertionMode,
        open_element_names: Vec<String>,
        open_element_keys: Vec<crate::PatchKey>,
        document_mode: crate::DocumentMode,
        patches: Vec<crate::DomPatch>,
        dom_summary: Vec<String>,
        counters: crate::html5::shared::Counters,
        legacy_errors: Vec<crate::html5::shared::ParseError>,
        capture: Option<ParserObservationCapture>,
    }

    fn run(observed: bool) -> Run {
        let ctx = if observed {
            DocumentParseContext::with_observations(
                ErrorPolicy::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 64 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 64 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 64 },
                    ..ParserObservationConfig::default()
                },
            )
        } else {
            DocumentParseContext::new()
        };
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        )
        .expect("session init");

        for chunk in [
            "<!doctype html><html><head><title a=x a=y>",
            "text</title>",
            "</head><body><div/></body>",
        ] {
            session.push_str_for_test(chunk);
            session.pump().expect("text-mode chunk should pump");
        }
        session
            .finish_for_test()
            .expect("text-mode session should finish");

        let controls = session
            .applied_tokenizer_controls_for_test()
            .iter()
            .map(|control| match control {
                TokenizerControl::EnterTextMode(spec) => AppliedControl::EnterTextMode {
                    kind: spec.kind,
                    end_tag_name: session
                        .ctx
                        .atoms
                        .resolve(spec.end_tag_name)
                        .expect("control atom belongs to the session")
                        .to_owned(),
                    namespace: spec.namespace,
                },
                TokenizerControl::ExitTextMode => AppliedControl::ExitTextMode,
            })
            .collect();
        let active_text_mode = session.tokenizer_active_text_mode_for_test();
        let state = session.tree_builder_state_snapshot_for_test();
        let open_element_names = state
            .open_element_names
            .iter()
            .map(|name| {
                session
                    .ctx
                    .atoms
                    .resolve(*name)
                    .expect("stack atom belongs to the session")
                    .to_owned()
            })
            .collect();
        let counters = session.counters();
        let legacy_errors = session.parse_errors();
        let capture = observed
            .then(|| session.take_observations_for_conformance())
            .transpose()
            .expect("observation drain")
            .flatten();
        let patches = session.take_patches().expect("session patch drain");
        let dom = crate::test_harness::materialize_patch_batches(std::slice::from_ref(&patches))
            .expect("session patches should materialize");
        let dom_summary = crate::html5::serialize_dom_for_test(&dom);

        Run {
            controls,
            active_text_mode,
            insertion_mode: state.insertion_mode,
            open_element_names,
            open_element_keys: state.open_element_keys,
            document_mode: state
                .quirks_mode
                .selected()
                .expect("mode selected in text-mode test"),
            patches,
            dom_summary,
            counters,
            legacy_errors,
            capture,
        }
    }

    let unobserved = run(false);
    let observed = run(true);

    assert_eq!(observed.controls, unobserved.controls);
    assert_eq!(
        observed.controls,
        vec![
            AppliedControl::EnterTextMode {
                kind: TextModeKind::Rcdata,
                end_tag_name: "title".to_owned(),
                namespace: TextModeNamespace::Html,
            },
            AppliedControl::ExitTextMode,
        ]
    );
    assert_eq!(observed.active_text_mode, None);
    assert_eq!(unobserved.active_text_mode, None);

    assert_eq!(observed.patches, unobserved.patches);
    assert_eq!(observed.dom_summary, unobserved.dom_summary);
    assert_eq!(observed.counters, unobserved.counters);
    assert_eq!(observed.legacy_errors, unobserved.legacy_errors);
    assert_eq!(observed.insertion_mode, unobserved.insertion_mode);
    assert_eq!(observed.open_element_names, unobserved.open_element_names);
    assert_eq!(observed.open_element_keys, unobserved.open_element_keys);
    assert_eq!(observed.document_mode, unobserved.document_mode);

    let capture = observed.capture.expect("explicit observation capture");
    assert_eq!(capture.failure, None);
    assert_eq!(capture.tokens.dropped, 0);
    assert_eq!(capture.parse_errors.dropped, 0);
    assert_eq!(capture.implementation_diagnostics.dropped, 0);
    assert_eq!(
        capture
            .parse_errors
            .items
            .iter()
            .map(|event| (event.occurrence, event.code))
            .collect::<Vec<_>>(),
        vec![
            (
                1,
                ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute),
            ),
            (
                2,
                ParseErrorCode::TreeConstruction(
                    TreeConstructionParseErrorCode::UnacknowledgedSelfClosingFlag,
                ),
            ),
        ],
        "real tokenizer controls must not perturb parse-error occurrence order"
    );
    assert_eq!(
        capture
            .implementation_diagnostics
            .items
            .iter()
            .map(|event| event.occurrence())
            .collect::<Vec<_>>(),
        vec![1],
        "implementation diagnostics retain their independent occurrence sequence"
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

#[cfg(feature = "parser-conformance")]
#[test]
fn terminal_final_audit_requires_both_text_modes_and_controls_to_be_clear() {
    use crate::html5::tokenizer::TextModeSpec;
    use crate::html5::tree_builder::modes::InsertionMode;

    fn terminal_audit(
        tokenizer_mode: Option<TextModeSpec>,
        builder_mode: Option<TextModeSpec>,
        original_mode: Option<InsertionMode>,
        pending_control: Option<crate::html5::tokenizer::TokenizerControl>,
        insertion_mode: InsertionMode,
    ) -> bool {
        let context = DocumentParseContext::new();
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            context,
        )
        .expect("session init");
        session.push_str_for_test("<html><body><p>x</p>");
        session.pump().expect("terminal fixture pump");
        session.finish_for_test().expect("terminal fixture finish");
        session.set_terminal_text_state_for_test(
            tokenizer_mode,
            builder_mode,
            original_mode,
            pending_control,
            insertion_mode,
        );
        let mut reserve = |_| Ok(());
        session
            .final_audit_for_conformance(&mut reserve)
            .expect("terminal audit should return a report")
            .tree_builder
            .insertion_mode_valid
    }

    let mode = {
        let mut names = crate::names::NameInterner::new();
        TextModeSpec::rcdata_textarea(
            names
                .intern_ascii_folded("textarea")
                .expect("textarea atom"),
        )
    };
    let other_mode = {
        let mut names = crate::names::NameInterner::new();
        TextModeSpec::script_data(names.intern_ascii_folded("script").expect("script atom"))
    };

    for (label, tokenizer_mode, builder_mode, original_mode, pending_control, insertion_mode) in [
        (
            "tokenizer-only",
            Some(mode),
            None,
            None,
            None,
            InsertionMode::InBody,
        ),
        (
            "tree-builder-only",
            None,
            Some(mode),
            None,
            None,
            InsertionMode::InBody,
        ),
        (
            "both-equal",
            Some(mode),
            Some(mode),
            None,
            None,
            InsertionMode::InBody,
        ),
        (
            "both-unequal",
            Some(mode),
            Some(other_mode),
            None,
            None,
            InsertionMode::InBody,
        ),
        ("terminal-text", None, None, None, None, InsertionMode::Text),
        (
            "terminal-table-text",
            None,
            None,
            None,
            None,
            InsertionMode::InTableText,
        ),
        (
            "original-mode",
            None,
            None,
            Some(InsertionMode::InBody),
            None,
            InsertionMode::InBody,
        ),
        (
            "pending-control",
            None,
            None,
            None,
            Some(crate::html5::tokenizer::TokenizerControl::ExitTextMode),
            InsertionMode::InBody,
        ),
    ] {
        assert!(
            !terminal_audit(
                tokenizer_mode,
                builder_mode,
                original_mode,
                pending_control,
                insertion_mode,
            ),
            "{label} must fail terminal insertion-mode validity"
        );
    }

    assert!(terminal_audit(
        None,
        None,
        None,
        None,
        InsertionMode::InBody
    ));
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
fn session_self_closing_textarea_remains_non_void_and_enters_text_mode() {
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
        Some(TextModeSpec::rcdata_textarea(
            session
                .tree_builder_state_snapshot_for_test()
                .active_text_mode
                .expect("textarea text mode")
                .end_tag_name,
        )),
        "trailing-solidus textarea syntax must still enter tokenizer text mode"
    );
    assert_eq!(
        builder_state.active_text_mode,
        Some(TextModeSpec::rcdata_textarea(
            builder_state
                .active_text_mode
                .expect("textarea text mode")
                .end_tag_name,
        )),
        "trailing-solidus textarea syntax must remain a textarea text-mode element"
    );
    assert_eq!(
        builder_state.insertion_mode,
        InsertionMode::Text,
        "trailing-solidus textarea syntax must stay in Text mode until recovery or an end tag"
    );

    let lines = finish_session_to_dom_lines(&mut session);
    assert_eq!(
        lines,
        vec![
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"textarea\" attrs=[]".to_string(),
            "        \"ok\"".to_string(),
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
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "      element ns=html local=\"title\" attrs=[]".to_string(),
            "        \"Hello & goodbye\"".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      element ns=html local=\"p\" attrs=[]".to_string(),
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
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "      element ns=html local=\"style\" attrs=[]".to_string(),
            "        \"a</title>b\"".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      \"ok\"".to_string(),
        ]
    );
}
