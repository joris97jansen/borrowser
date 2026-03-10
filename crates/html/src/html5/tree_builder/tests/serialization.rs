use super::helpers::EmptyResolver;

#[test]
fn serialize_dom_for_test_emits_deterministic_html5_dom_v1_lines() {
    use crate::dom_patch::DomPatch;
    use crate::dom_snapshot::DomSnapshotOptions;
    use crate::html5::shared::{Attribute, AttributeValue, Token};
    use crate::html5::tree_builder::serialize_dom_for_test_with_options;

    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let resolver = EmptyResolver;

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
