use super::helpers::EmptyResolver;

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
    let mut sink = Sink;
    let resolver = EmptyResolver;
    let _ = builder
        .push_token(
            &crate::html5::shared::Token::Eof,
            &ctx.atoms,
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

    let checked = check_patch_invariants(&buffered, &DomInvariantState::default())
        .expect("buffered patch stream must satisfy patch invariants");
    check_dom_invariants(&checked).expect("buffered patch stream must yield a valid DOM state");
    assert_eq!(buffered, sinked);
}
