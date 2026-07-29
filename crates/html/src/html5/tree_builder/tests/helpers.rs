pub(super) struct EmptyResolver;

impl crate::html5::tree_builder::Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn insert_normal_html_element_for_test(
        &mut self,
        name: crate::html5::shared::AtomId,
        attrs: &[crate::html5::shared::Attribute],
        ctx: &mut crate::html5::shared::DocumentParseContext,
        text: &dyn crate::html5::tokenizer::TextResolver,
    ) -> Result<Option<crate::dom_patch::PatchKey>, crate::html5::tree_builder::TreeBuilderError>
    {
        let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
        let atoms = context.atoms();
        self.insert_normal_html_element(name, attrs, &mut context, atoms, text)
    }

    pub(in crate::html5::tree_builder) fn insert_void_html_element_for_test(
        &mut self,
        name: crate::html5::shared::AtomId,
        attrs: &[crate::html5::shared::Attribute],
        ctx: &mut crate::html5::shared::DocumentParseContext,
        text: &dyn crate::html5::tokenizer::TextResolver,
    ) -> Result<Option<crate::dom_patch::PatchKey>, crate::html5::tree_builder::TreeBuilderError>
    {
        let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
        let atoms = context.atoms();
        self.insert_void_html_element(name, attrs, &mut context, atoms, text)
    }

    pub(in crate::html5::tree_builder) fn insert_literal_text_for_test(
        &mut self,
        literal: &str,
        ctx: &mut crate::html5::shared::DocumentParseContext,
    ) -> Result<(), crate::html5::tree_builder::TreeBuilderError> {
        let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
        self.insert_literal_text(literal, &mut context)
    }

    pub(in crate::html5::tree_builder) fn insert_comment_for_test(
        &mut self,
        token_text: &crate::html5::shared::TextValue,
        ctx: &mut crate::html5::shared::DocumentParseContext,
        text: &dyn crate::html5::tokenizer::TextResolver,
    ) -> Result<(), crate::html5::tree_builder::TreeBuilderError> {
        let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
        self.insert_comment(token_text, &mut context, text)
    }

    pub(in crate::html5::tree_builder) fn reconstruct_active_formatting_elements_for_test(
        &mut self,
        ctx: &mut crate::html5::shared::DocumentParseContext,
    ) -> Result<usize, crate::html5::tree_builder::TreeBuilderError> {
        let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
        let atoms = context.atoms();
        self.reconstruct_active_formatting_elements(atoms, &mut context)
    }

    pub(in crate::html5::tree_builder) fn close_cell_for_test(
        &mut self,
        ctx: &mut crate::html5::shared::DocumentParseContext,
    ) -> bool {
        let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
        self.close_cell(&mut context)
    }

    pub(in crate::html5::tree_builder) fn run_adoption_agency_algorithm_for_test(
        &mut self,
        subject: crate::html5::shared::AtomId,
        ctx: &mut crate::html5::shared::DocumentParseContext,
    ) -> Result<
        crate::html5::tree_builder::adoption::AdoptionAgencyRunReport,
        crate::html5::tree_builder::TreeBuilderError,
    > {
        let mut context = crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx);
        let atoms = context.atoms();
        self.run_adoption_agency_algorithm(subject, atoms, &mut context)
    }
}

impl crate::html5::tokenizer::TextResolver for EmptyResolver {
    fn resolve_span(
        &self,
        span: crate::html5::shared::TextSpan,
    ) -> Result<&str, crate::html5::tokenizer::TextResolveError> {
        Err(crate::html5::tokenizer::TextResolveError::InvalidSpan { span })
    }
}

pub(super) fn run_tree_builder_chunks(chunks: &[&str]) -> Vec<crate::dom_patch::DomPatch> {
    use crate::html5::shared::{DocumentParseContext, Input};
    use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};
    use crate::html5::tree_builder::{
        DomInvariantState, check_dom_invariants, check_patch_invariants,
    };
    use crate::test_harness::PatchValidationArena;

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let mut input = Input::new();

    for chunk in chunks {
        input.push_str(chunk);
        loop {
            builder.prepare_tokenizer_pump(&mut tokenizer);
            let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                assert!(
                    matches!(
                        result,
                        TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                    ),
                    "unexpected tokenizer state while draining chunk: {result:?}"
                );
                break;
            }
            let resolver = batch.resolver();
            for token in batch.iter() {
                let _ = builder
                    .process(
                        token,
                        &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
                        &resolver,
                    )
                    .expect("tree builder test run should remain recoverable");
            }
        }
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            let _ = builder
                .process(
                    token,
                    &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
                    &resolver,
                )
                .expect("tree builder EOF drain should remain recoverable");
        }
    }

    let patches = builder.drain_patches();
    let checked_state = check_patch_invariants(&patches, &DomInvariantState::default())
        .expect("tree builder patch output must satisfy patch invariants");
    let mut patch_arena = PatchValidationArena::default();
    patch_arena
        .apply_batch(&patches)
        .expect("tree builder patch output must apply without patch-contract violations");
    let live_state = builder.dom_invariant_state();
    check_dom_invariants(&live_state).expect("tree builder live DOM must satisfy DOM invariants");
    assert_eq!(
        live_state, checked_state,
        "patch-derived DOM state must match the tree builder live DOM"
    );

    patches
}

pub(super) fn materialized_dom_lines(chunks: &[&str]) -> Vec<String> {
    let patches = run_tree_builder_chunks(chunks);
    let dom = crate::test_harness::materialize_patch_batches(&[patches])
        .expect("patch batches should materialize");
    crate::html5::tree_builder::serialize_dom_for_test(&dom)
}

pub(super) fn enter_after_head(
    builder: &mut crate::html5::tree_builder::Html5TreeBuilder,
    ctx: &mut crate::html5::shared::DocumentParseContext,
    resolver: &EmptyResolver,
) -> Vec<crate::dom_patch::DomPatch> {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");

    for token in [
        Token::Doctype {
            name: Some(html),
            public_id: None,
            system_id: None,
            force_quirks: false,
        },
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
        Token::EndTag { name: head },
    ] {
        let _ = builder
            .process(
                &token,
                &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx),
                resolver,
            )
            .expect("after-head prelude should process");
    }
    let snap = builder.state_snapshot();
    assert_eq!(
        snap.insertion_mode,
        InsertionMode::AfterHead,
        "enter_after_head() must leave builder in AfterHead"
    );
    assert!(
        snap.open_element_names.contains(&html),
        "expected <html> on SOE after enter_after_head()"
    );
    assert!(
        !snap.open_element_names.contains(&head),
        "expected <head> to be popped from SOE after enter_after_head()"
    );
    assert_eq!(
        snap.quirks_mode,
        crate::DocumentMode::NoQuirks,
        "enter_after_head() should keep NoQuirks for a normal html doctype"
    );
    builder.drain_patches()
}

pub(super) fn enter_in_body(
    builder: &mut crate::html5::tree_builder::Html5TreeBuilder,
    ctx: &mut crate::html5::shared::DocumentParseContext,
    resolver: &EmptyResolver,
) -> Vec<crate::dom_patch::DomPatch> {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let mut patches = enter_after_head(builder, ctx, resolver);
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::StartTag {
                name: body,
                attrs: Vec::new(),
                self_closing: false,
            },
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx),
            resolver,
        )
        .expect("in-body prelude should process");

    let snap = builder.state_snapshot();
    assert_eq!(
        snap.insertion_mode,
        InsertionMode::InBody,
        "enter_in_body() must leave builder in InBody"
    );
    assert_eq!(
        snap.open_element_names.last().copied(),
        Some(body),
        "enter_in_body() must leave <body> on top of SOE"
    );
    patches.extend(builder.drain_patches());
    patches
}

pub(super) fn panic_message_contains(payload: &(dyn std::any::Any + Send), needle: &str) -> bool {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        return msg.contains(needle);
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.contains(needle);
    }
    false
}

pub(super) fn assert_binding_mismatch_panic(payload: &(dyn std::any::Any + Send), context: &str) {
    assert!(
        panic_message_contains(payload, "tree builder atom table mismatch"),
        "{context} panic must come from atom-table binding assertion"
    );
    assert!(
        panic_message_contains(payload, "expected=") && panic_message_contains(payload, "actual="),
        "{context} panic should include expected/actual atom table ids"
    );
}
