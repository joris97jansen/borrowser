#[test]
fn tree_builder_api_compiles() {
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    );
    struct Sink;
    impl crate::html5::tree_builder::PatchSink for Sink {
        fn push(&mut self, _patch: crate::dom_patch::DomPatch) {}
    }
    struct Resolver;
    impl crate::html5::tokenizer::TextResolver for Resolver {
        fn resolve_span(&self, _span: crate::html5::shared::TextSpan) -> Option<&str> {
            None
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
