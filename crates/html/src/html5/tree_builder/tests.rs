#[test]
fn tree_builder_api_compiles() {
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    struct Sink;
    impl crate::html5::tree_builder::PatchSink for Sink {
        fn push(&mut self, _patch: crate::dom_patch::DomPatch) {}
    }
    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }
    let mut sink = Sink;
    let resolver = Resolver;
    let _ = builder.push_token(
        &crate::html5::shared::Token::Eof,
        &ctx.atoms,
        &resolver,
        &mut sink,
    );
}

#[test]
fn tree_builder_process_and_drain_emit_deterministic_patches() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    fn run_once() -> Vec<DomPatch> {
        let mut ctx = crate::html5::shared::DocumentParseContext::new();
        let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");
        let resolver = Resolver;
        let div = ctx
            .atoms
            .intern_ascii_folded("div")
            .expect("atom interning");
        let tokens = [
            Token::StartTag {
                name: div,
                attrs: Vec::new(),
                self_closing: false,
            },
            Token::Text {
                text: TextValue::Owned("hello".to_string()),
            },
            Token::EndTag { name: div },
            Token::Eof,
        ];
        for token in &tokens {
            let _ = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("process should not fail");
        }
        builder.drain_patches()
    }

    let first = run_once();
    let second = run_once();
    assert_eq!(first, second, "patch stream must be deterministic");
    assert!(matches!(
        first.first(),
        Some(DomPatch::CreateDocument { .. })
    ));
    assert!(
        first
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateElement { .. })),
        "expected at least one element creation patch"
    );
    assert!(
        first
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { .. })),
        "expected at least one text creation patch"
    );
}

#[test]
fn tree_builder_buffered_and_sink_paths_match() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::VecPatchSink;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    fn build_tokens(ctx: &mut crate::html5::shared::DocumentParseContext) -> [Token; 4] {
        let div = ctx
            .atoms
            .intern_ascii_folded("div")
            .expect("atom interning");
        [
            Token::StartTag {
                name: div,
                attrs: Vec::new(),
                self_closing: false,
            },
            Token::Text {
                text: TextValue::Owned("hello".to_string()),
            },
            Token::EndTag { name: div },
            Token::Eof,
        ]
    }

    let resolver = Resolver;

    let buffered = {
        let mut ctx = crate::html5::shared::DocumentParseContext::new();
        let tokens = build_tokens(&mut ctx);
        let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");
        for token in &tokens {
            let _ = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("process should not fail");
        }
        builder.drain_patches()
    };

    let sinked = {
        let mut ctx = crate::html5::shared::DocumentParseContext::new();
        let tokens = build_tokens(&mut ctx);
        let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");
        let mut patches: Vec<DomPatch> = Vec::new();
        let mut sink = VecPatchSink(&mut patches);
        for token in &tokens {
            let _ = builder
                .push_token(token, &ctx.atoms, &resolver, &mut sink)
                .expect("push_token should not fail");
        }
        patches
    };

    assert_eq!(buffered, sinked);
}

#[test]
fn tree_builder_rejects_foreign_atom_table() {
    use crate::html5::shared::Token;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut owner_ctx = crate::html5::shared::DocumentParseContext::new();
    let foreign_ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut owner_ctx,
    )
    .expect("tree builder init");

    let process_panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = builder.process(&Token::Eof, &foreign_ctx.atoms, &resolver);
    }))
    .expect_err("process must trip invariant assertion");
    assert_binding_mismatch_panic(process_panic.as_ref(), "process");

    let push_panic = catch_unwind(AssertUnwindSafe(|| {
        let mut out = Vec::new();
        let mut sink = crate::html5::tree_builder::VecPatchSink(&mut out);
        let _ = builder.push_token(&Token::Eof, &foreign_ctx.atoms, &resolver, &mut sink);
    }))
    .expect_err("push_token must trip invariant assertion");
    assert_binding_mismatch_panic(push_panic.as_ref(), "push_token");

    let recovery_result = builder.process(&Token::Eof, &owner_ctx.atoms, &resolver);
    assert!(
        recovery_result.is_ok(),
        "builder should remain usable with its bound atom table after rejection"
    );
}

#[test]
fn tree_builder_state_snapshot_exposes_core_v0_internal_model() {
    use crate::html5::shared::Token;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let tokens = [
        Token::Doctype {
            name: Some(html),
            public_id: None,
            system_id: None,
            force_quirks: true,
        },
        Token::StartTag {
            name: html,
            attrs: Vec::new(),
            self_closing: false,
        },
    ];
    for token in &tokens {
        let _ = builder
            .process(token, &ctx.atoms, &resolver)
            .expect("process should not fail");
    }

    let state = builder.state_snapshot();
    assert_eq!(state.open_element_names, vec![html]);
    assert_eq!(state.open_element_keys.len(), 1);
    assert_eq!(
        state.quirks_mode,
        crate::html5::tree_builder::QuirksMode::Quirks
    );
    assert!(state.frameset_ok);
}

#[test]
fn tree_builder_mode_dispatch_transitions_for_representative_sequence() {
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");

    let cases = [
        (
            Token::Doctype {
                name: Some(html),
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            InsertionMode::Initial,
        ),
        (
            Token::StartTag {
                name: html,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::BeforeHead,
        ),
        (
            Token::StartTag {
                name: head,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::InHead,
        ),
        (Token::EndTag { name: head }, InsertionMode::AfterHead),
        (
            Token::StartTag {
                name: body,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::InBody,
        ),
        (
            Token::StartTag {
                name: textarea,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::Text,
        ),
        (
            Token::Text {
                text: TextValue::Owned("x".to_string()),
            },
            InsertionMode::Text,
        ),
        (Token::EndTag { name: textarea }, InsertionMode::InBody),
        (Token::EndTag { name: body }, InsertionMode::InBody),
        (Token::Eof, InsertionMode::InBody),
    ];

    for (token, expected_mode) in &cases {
        let _ = builder
            .process(token, &ctx.atoms, &resolver)
            .expect("process should not fail");
        assert_eq!(
            builder.state_snapshot().insertion_mode,
            *expected_mode,
            "unexpected insertion mode after token: {token:?}"
        );
    }
}

#[test]
fn tree_builder_recovers_from_malformed_token_ordering_without_panic() {
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    // Malformed ordering: closing tag before any matching open element.
    let _ = builder
        .process(&Token::EndTag { name: div }, &ctx.atoms, &resolver)
        .expect("malformed end tag should be recoverable");
    let state_after_malformed = builder.state_snapshot();
    assert!(
        matches!(
            state_after_malformed.insertion_mode,
            InsertionMode::BeforeHtml
                | InsertionMode::BeforeHead
                | InsertionMode::InHead
                | InsertionMode::AfterHead
                | InsertionMode::InBody
                | InsertionMode::Initial
                | InsertionMode::Text
        ),
        "builder must remain in a valid insertion mode after malformed ordering"
    );

    // Builder should continue processing subsequent tokens deterministically.
    for token in [
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("ok".to_string()),
        },
        Token::EndTag { name: div },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("post-recovery token should process");
    }

    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateDocument { .. })),
        "recovered run must still produce a document"
    );
}

#[test]
fn tree_builder_recovers_from_early_end_tags_in_pre_body_modes() {
    use crate::html5::shared::Token;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");
    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");

    for token in [
        Token::EndTag { name: head },
        Token::EndTag { name: body },
        Token::EndTag { name: html },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("early end tags should stay recoverable");
    }

    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateDocument { .. })),
        "recovered pre-body malformed run must still produce a document"
    );
}

#[test]
fn tree_builder_text_mode_unexpected_start_tag_does_not_push_stack() {
    use crate::html5::shared::Token;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: html,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: head,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: textarea,
            attrs: Vec::new(),
            self_closing: false,
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("text-mode sequence should remain recoverable");
    }

    let before_unexpected = builder.state_snapshot();
    let before_depth = before_unexpected.open_element_names.len();
    assert_eq!(
        before_unexpected.open_element_names.last().copied(),
        Some(textarea)
    );

    let _ = builder
        .process(
            &Token::StartTag {
                name: div,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("unexpected text-mode start tag should stay recoverable");

    let after_unexpected = builder.state_snapshot();
    assert_eq!(
        after_unexpected.open_element_names.len(),
        before_depth,
        "unexpected start tag in Text mode must not push SOE"
    );
    assert_eq!(
        after_unexpected.open_element_names.last().copied(),
        Some(textarea),
        "unexpected start tag in Text mode must keep current text node context"
    );

    let _ = builder
        .process(&Token::EndTag { name: textarea }, &ctx.atoms, &resolver)
        .expect("closing textarea should remain recoverable");
    let after_close = builder.state_snapshot();
    assert!(
        !after_close.open_element_names.contains(&div),
        "unexpected start tag in Text mode must not leave pushed element behind"
    );
    assert!(
        after_close.open_element_names.len() <= before_depth,
        "closing text node context should not increase SOE depth"
    );
}

#[test]
fn tree_builder_recovers_when_head_end_tag_seen_before_head_opened() {
    use crate::html5::shared::Token;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");

    let _ = builder
        .process(&Token::EndTag { name: head }, &ctx.atoms, &resolver)
        .expect("early </head> should stay recoverable");
    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("builder should continue after early </head>");

    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateDocument { .. })),
        "early </head> recovery path should still materialize a document"
    );
}

#[test]
fn tree_builder_coalescing_does_not_cross_structural_mutations() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig {
            coalesce_text: true,
        },
        &mut ctx,
    )
    .expect("tree builder init");

    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");
    let span = ctx
        .atoms
        .intern_ascii_folded("span")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("a".to_string()),
        },
        Token::StartTag {
            name: span,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::EndTag { name: span },
        Token::Text {
            text: TextValue::Owned("b".to_string()),
        },
        Token::EndTag { name: div },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("coalescing structural boundary sequence should process");
    }

    let text_values: Vec<_> = builder
        .drain_patches()
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text),
            _ => None,
        })
        .collect();

    assert_eq!(text_values, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn tree_builder_coalescing_does_not_cross_parent_change_after_pop() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig {
            coalesce_text: true,
        },
        &mut ctx,
    )
    .expect("tree builder init");

    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("a".to_string()),
        },
        Token::EndTag { name: div },
        Token::Text {
            text: TextValue::Owned("b".to_string()),
        },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("parent-change coalescing sequence should process");
    }

    let text_values: Vec<_> = builder
        .drain_patches()
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text),
            _ => None,
        })
        .collect();

    assert_eq!(text_values, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn tree_builder_text_mode_literalization_does_not_coalesce_with_real_text() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig {
            coalesce_text: true,
        },
        &mut ctx,
    )
    .expect("tree builder init");

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: html,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: head,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: textarea,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("a".to_string()),
        },
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("b".to_string()),
        },
        Token::EndTag { name: textarea },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("text-mode literalization sequence should process");
    }

    let text_values: Vec<_> = builder
        .drain_patches()
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_values,
        vec!["a".to_string(), "<div>".to_string(), "b".to_string()]
    );
}

#[test]
fn tree_builder_text_mode_unexpected_end_tag_literalization_normalizes_name() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::Token;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let resolver = Resolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let mixed_div = ctx
        .atoms
        .intern_ascii_folded("DiV")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: html,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: head,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: textarea,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::EndTag { name: mixed_div },
        Token::EndTag { name: textarea },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("unexpected end-tag literalization sequence should process");
    }

    let text_values: Vec<_> = builder
        .drain_patches()
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        text_values.iter().any(|text| text == "</div>"),
        "unexpected end-tag literalization should use folded tag name"
    );
    assert!(
        !text_values.iter().any(|text| text == "</DiV>"),
        "unexpected end-tag literalization must not preserve mixed-case source"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn serialize_dom_for_test_emits_deterministic_html5_dom_v1_lines() {
    use crate::dom_patch::DomPatch;
    use crate::dom_snapshot::DomSnapshotOptions;
    use crate::html5::shared::{Attribute, AttributeValue, Token};
    use crate::html5::tree_builder::serialize_dom_for_test_with_options;

    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
            Ok("")
        }
    }

    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let resolver = Resolver;

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");
    let class = ctx
        .atoms
        .intern_ascii_folded("class")
        .expect("atom interning");
    let hidden = ctx
        .atoms
        .intern_ascii_folded("hidden")
        .expect("atom interning");

    let tokens = [
        Token::Doctype {
            name: Some(html),
            public_id: None,
            system_id: None,
            force_quirks: false,
        },
        Token::StartTag {
            name: div,
            attrs: vec![
                Attribute {
                    name: class,
                    value: Some(AttributeValue::Owned("x".to_string())),
                },
                Attribute {
                    name: hidden,
                    value: None,
                },
            ],
            self_closing: false,
        },
        Token::Text {
            text: crate::html5::shared::TextValue::Owned("a\nb".to_string()),
        },
        Token::Comment {
            text: crate::html5::shared::TextValue::Owned("c".to_string()),
        },
        Token::EndTag { name: div },
        Token::Eof,
    ];

    for token in &tokens {
        let _ = builder
            .process(token, &ctx.atoms, &resolver)
            .expect("process should not fail");
    }
    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateDocument { .. })),
        "expected document patch"
    );

    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize dom");
    let lines = serialize_dom_for_test_with_options(
        &dom,
        DomSnapshotOptions {
            ignore_ids: true,
            ignore_empty_style: true,
        },
    );
    assert_eq!(
        lines,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <div class=\"x\" hidden>".to_string(),
            "        \"a\\nb\"".to_string(),
            "        <!-- c -->".to_string(),
        ]
    );
}

fn panic_message_contains(payload: &(dyn std::any::Any + Send), needle: &str) -> bool {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        return msg.contains(needle);
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.contains(needle);
    }
    false
}

fn assert_binding_mismatch_panic(payload: &(dyn std::any::Any + Send), context: &str) {
    assert!(
        panic_message_contains(payload, "tree builder atom table mismatch"),
        "{context} panic must come from atom-table binding assertion"
    );
    assert!(
        panic_message_contains(payload, "expected=") && panic_message_contains(payload, "actual="),
        "{context} panic should include expected/actual atom table ids"
    );
}
