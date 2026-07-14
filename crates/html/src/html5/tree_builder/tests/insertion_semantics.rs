use super::helpers::{EmptyResolver, enter_in_body};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

#[test]
fn semantic_insertion_operations_enforce_implemented_void_categories() {
    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    let input = ctx.atoms.intern_ascii_folded("input").expect("input atom");
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");
    let keygen = ctx
        .atoms
        .intern_ascii_folded("keygen")
        .expect("keygen atom");

    assert!(
        builder
            .insert_void_html_element(input, &[], &ctx.atoms, &resolver)
            .expect("void input insertion")
            .is_some()
    );
    assert!(
        builder
            .insert_void_html_element(keygen, &[], &ctx.atoms, &resolver)
            .expect("void keygen insertion")
            .is_some()
    );
    assert!(
        builder
            .insert_normal_html_element(form, &[], &ctx.atoms, &resolver)
            .expect("normal form insertion")
            .is_some()
    );

    let wrong_void = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = builder.insert_void_html_element(form, &[], &ctx.atoms, &resolver);
    }));
    assert!(wrong_void.is_err());

    let wrong_normal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = builder.insert_normal_html_element(input, &[], &ctx.atoms, &resolver);
    }));
    assert!(wrong_normal.is_err());
}

#[test]
fn frozen_legacy_dispatch_paths_keep_pre_ae9_stack_and_metric_behavior() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let div = ctx.atoms.intern_ascii_folded("div").expect("div atom");
    let before = builder.state_snapshot();
    let before_stats = builder.debug_perf_stats();
    let _ = builder
        .process(
            &Token::StartTag {
                name: div,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("legacy generic self-closing path should remain recoverable");
    let after = builder.state_snapshot();
    let after_stats = builder.debug_perf_stats();
    assert_eq!(after.open_element_keys, before.open_element_keys);
    assert_eq!(after_stats.soe_push_ops, before_stats.soe_push_ops);
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops);

    let mut meta_ctx = crate::html5::shared::DocumentParseContext::new();
    let mut meta_builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut meta_ctx)
        .expect("tree builder init");
    let html = meta_ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("html atom");
    let head = meta_ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("head atom");
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
    ] {
        let _ = meta_builder
            .process(&token, &meta_ctx.atoms, &resolver)
            .expect("in-head legacy setup should remain recoverable");
    }
    let meta = meta_ctx
        .atoms
        .intern_ascii_folded("meta")
        .expect("meta atom");
    let meta_before = meta_builder.state_snapshot();
    let meta_before_stats = meta_builder.debug_perf_stats();
    let _ = meta_builder
        .process(
            &Token::StartTag {
                name: meta,
                attrs: Vec::new(),
                self_closing: false,
            },
            &meta_ctx.atoms,
            &resolver,
        )
        .expect("legacy known-void path should remain recoverable");
    let meta_after = meta_builder.state_snapshot();
    let meta_after_stats = meta_builder.debug_perf_stats();
    assert_eq!(meta_after.open_element_keys, meta_before.open_element_keys);
    assert_eq!(
        meta_after_stats.soe_push_ops,
        meta_before_stats.soe_push_ops
    );
    assert_eq!(meta_after_stats.soe_pop_ops, meta_before_stats.soe_pop_ops);
}

#[test]
fn ae9_void_dispatch_paths_record_real_push_pop_transitions() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let input = ctx.atoms.intern_ascii_folded("input").expect("input atom");
    let keygen = ctx
        .atoms
        .intern_ascii_folded("keygen")
        .expect("keygen atom");
    let before = builder.state_snapshot();
    let before_stats = builder.debug_perf_stats();

    for name in [input, keygen] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("AE9 void path should remain recoverable");
    }

    let after = builder.state_snapshot();
    let after_stats = builder.debug_perf_stats();
    assert_eq!(after.open_element_keys, before.open_element_keys);
    assert_eq!(after_stats.soe_push_ops, before_stats.soe_push_ops + 2);
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops + 2);
}
