use super::helpers::EmptyResolver;

#[test]
fn tree_builder_api_compiles() {
    let mut ctx = crate::html5::shared::DocumentParseContext::with_tree_observations_for_test();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    struct Sink;
    impl crate::html5::tree_builder::PatchSink for Sink {
        fn push(&mut self, _patch: crate::dom_patch::DomPatch) {}
    }
    let mut sink = Sink;
    let resolver = EmptyResolver;
    let _ = builder
        .push_token(
            &crate::html5::shared::Token::Eof,
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
            &resolver,
            &mut sink,
        )
        .expect("push_token should not fail");
}

#[test]
fn tree_builder_buffered_and_sink_paths_match() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::{
        DomInvariantState, VecPatchSink, check_dom_invariants, check_patch_invariants,
    };

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

    let resolver = EmptyResolver;

    let buffered = {
        let mut ctx = crate::html5::shared::DocumentParseContext::with_tree_observations_for_test();
        let tokens = build_tokens(&mut ctx);
        let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");
        for token in &tokens {
            let _ = builder
                .process(
                    token,
                    &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
                    &resolver,
                )
                .expect("process should not fail");
        }
        builder.drain_patches()
    };

    let sinked = {
        let mut ctx = crate::html5::shared::DocumentParseContext::with_tree_observations_for_test();
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
                .push_token(
                    token,
                    &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
                    &resolver,
                    &mut sink,
                )
                .expect("push_token should not fail");
        }
        patches
    };

    let checked = check_patch_invariants(&buffered, &DomInvariantState::default())
        .expect("buffered patch stream must satisfy patch invariants");
    check_dom_invariants(&checked).expect("buffered patch stream must yield a valid DOM state");
    assert_eq!(buffered, sinked);
}

#[test]
fn direct_tree_error_occurrence_overflow_is_a_recorder_invariant() {
    use crate::html5::shared::{
        ObservationOccurrenceSequence, ParserObservationInvariant, TextValue, Token,
    };

    let mut ctx = crate::html5::shared::DocumentParseContext::with_tree_observations_for_test();
    ctx.enable_tree_observations_for_test();
    ctx.set_next_parse_error_occurrence_for_test(u64::MAX);
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let resolver = EmptyResolver;
    let _ = builder
        .process(
            &Token::Text {
                text: TextValue::Owned("x".to_owned()),
            },
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
            &resolver,
        )
        .expect("tree recovery remains successful");
    let capture = ctx.take_observations().expect("tree capture");
    assert_eq!(capture.parse_errors.items.len(), 1);
    assert_eq!(capture.parse_errors.items[0].occurrence, u64::MAX);
    assert_eq!(
        capture.failure,
        Some(crate::html5::shared::ParserObservationFailure::Invariant(
            ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::ParseErrors,
            )
        ))
    );
}
