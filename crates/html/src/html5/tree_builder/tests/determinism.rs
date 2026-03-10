use super::helpers::EmptyResolver;

#[test]
fn tree_builder_process_and_drain_emit_deterministic_patches() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    fn run_once() -> (Vec<DomPatch>, Vec<String>) {
        let mut ctx = crate::html5::shared::DocumentParseContext::new();
        let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
            crate::html5::tree_builder::TreeBuilderConfig::default(),
            &mut ctx,
        )
        .expect("tree builder init");
        let resolver = EmptyResolver;
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
        let patches = builder.drain_patches();
        let errors = builder
            .take_parse_error_kinds_for_test()
            .into_iter()
            .map(str::to_owned)
            .collect();
        (patches, errors)
    }

    let (first, first_errors) = run_once();
    let (second, second_errors) = run_once();
    assert_eq!(first, second, "patch stream must be deterministic");
    assert_eq!(
        first_errors, second_errors,
        "parse-error stream must be deterministic"
    );
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

    #[cfg(feature = "dom-snapshot")]
    {
        use crate::dom_snapshot::DomSnapshotOptions;
        use crate::html5::tree_builder::serialize_dom_for_test_with_options;
        use crate::test_harness::materialize_patch_batches;

        let first_dom =
            materialize_patch_batches(std::slice::from_ref(&first)).expect("materialize first dom");
        let second_dom = materialize_patch_batches(std::slice::from_ref(&second))
            .expect("materialize second dom");
        let options = DomSnapshotOptions {
            ignore_ids: true,
            ignore_empty_style: true,
        };
        let first_lines = serialize_dom_for_test_with_options(&first_dom, options);
        let second_lines = serialize_dom_for_test_with_options(&second_dom, options);
        assert_eq!(
            first_lines, second_lines,
            "deterministic patches should materialize to deterministic DOM snapshots"
        );
    }
}
