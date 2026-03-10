use super::helpers::EmptyResolver;

#[test]
fn tree_builder_state_snapshot_exposes_core_v0_internal_model() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
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
        crate::html5::tree_builder::document::QuirksMode::Quirks
    );
    assert!(state.frameset_ok);
}
