#[test]
fn tree_builder_api_compiles() {
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let _ = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    );
}
