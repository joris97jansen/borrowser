use super::helpers::EmptyResolver;

#[test]
fn tree_builder_coalescing_does_not_cross_structural_mutations() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    let resolver = EmptyResolver;
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
fn tree_builder_coalescing_merges_adjacent_text_tokens_under_same_parent() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    let resolver = EmptyResolver;
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
        Token::Text {
            text: TextValue::Owned("b".to_string()),
        },
        Token::Text {
            text: TextValue::Owned("c".to_string()),
        },
        Token::EndTag { name: div },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("adjacent text coalescing sequence should process");
    }

    let patches = builder.drain_patches();
    let text_values: Vec<_> = patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_values,
        vec!["a".to_string()],
        "coalescing keeps initial CreateText payload and evolves via AppendText"
    );

    let append_text_values: Vec<_> = patches
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::AppendText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        append_text_values,
        vec!["b".to_string(), "c".to_string()],
        "adjacent in-parent coalescing should emit incremental AppendText updates"
    );
}

#[test]
fn tree_builder_coalescing_does_not_cross_parent_change_after_pop() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    let resolver = EmptyResolver;
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
fn tree_builder_coalescing_does_not_cross_br_element_boundary() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    let resolver = EmptyResolver;
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
    let br = ctx.atoms.intern_ascii_folded("br").expect("atom interning");

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
            name: br,
            attrs: Vec::new(),
            self_closing: true,
        },
        Token::Text {
            text: TextValue::Owned("b".to_string()),
        },
        Token::EndTag { name: div },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("coalescing around <br> should stay recoverable");
    }

    let patches = builder.drain_patches();
    let text_values: Vec<_> = patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_values,
        vec!["a".to_string(), "b".to_string()],
        "coalescing must not merge text across <br> structural boundary"
    );
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::AppendText { .. })),
        "coalescing must not emit AppendText when text is split by an element boundary"
    );
}

#[test]
fn tree_builder_coalescing_patch_log_is_batch_boundary_invariant() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    fn run_with_drains(drain_each_token: bool) -> Vec<DomPatch> {
        let resolver = EmptyResolver;
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
        let tokens = [
            Token::StartTag {
                name: div,
                attrs: Vec::new(),
                self_closing: false,
            },
            Token::Text {
                text: TextValue::Owned("a".to_string()),
            },
            Token::Text {
                text: TextValue::Owned("b".to_string()),
            },
            Token::Text {
                text: TextValue::Owned("c".to_string()),
            },
            Token::EndTag { name: div },
            Token::Eof,
        ];
        let mut out = Vec::new();
        for token in &tokens {
            let _ = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("batch-boundary coalescing sequence should process");
            if drain_each_token {
                out.extend(builder.drain_patches());
            }
        }
        out.extend(builder.drain_patches());
        out
    }

    let whole_batch = run_with_drains(false);
    let drained_per_token = run_with_drains(true);
    assert_eq!(
        whole_batch, drained_per_token,
        "coalescing patch log must remain stable when batch boundaries change"
    );
}
