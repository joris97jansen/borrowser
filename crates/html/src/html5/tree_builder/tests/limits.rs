use super::helpers::{EmptyResolver, enter_in_body};
use crate::dom_patch::DomPatch;
use crate::html5::shared::{DocumentParseContext, TextValue};
use crate::html5::tree_builder::{
    Html5TreeBuilder, TreeBuilderConfig, TreeBuilderLimits, check_dom_invariants,
    check_patch_invariants,
};

fn builder_with_limits(
    limits: TreeBuilderLimits,
) -> (Html5TreeBuilder, DocumentParseContext, EmptyResolver) {
    let mut ctx = DocumentParseContext::new();
    let builder = Html5TreeBuilder::new(
        TreeBuilderConfig {
            limits,
            ..TreeBuilderConfig::default()
        },
        &mut ctx,
    )
    .expect("tree builder init");
    (builder, ctx, EmptyResolver)
}

#[test]
fn tree_builder_depth_limit_ignores_excess_nesting_and_preserves_invariants() {
    let (mut builder, mut ctx, resolver) = builder_with_limits(TreeBuilderLimits {
        max_open_elements_depth: 2,
        ..TreeBuilderLimits::default()
    });
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let _ = builder.drain_patches();
    let baseline = builder.dom_invariant_state();
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    let inserted = builder
        .insert_normal_html_element(div, &[], &ctx.atoms, &resolver)
        .expect("depth-limited insert should remain recoverable");

    assert!(
        inserted.is_none(),
        "depth-limited element should be ignored"
    );
    let patches = builder.drain_patches();
    let checked = check_patch_invariants(&patches, &baseline).expect("patches must stay valid");
    let live = builder.dom_invariant_state();
    check_dom_invariants(&live).expect("live DOM must stay valid");
    assert_eq!(live, checked);
    assert!(
        builder
            .take_parse_error_kinds_for_test()
            .contains(&"resource-limit-soe-depth")
    );
}

#[test]
fn tree_builder_node_limit_ignores_additional_text_nodes_and_preserves_invariants() {
    let (mut builder, mut ctx, resolver) = builder_with_limits(TreeBuilderLimits {
        max_nodes_created: 4,
        ..TreeBuilderLimits::default()
    });
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let _ = builder.drain_patches();
    let baseline = builder.dom_invariant_state();

    builder
        .insert_literal_text("blocked")
        .expect("node-limited text insert should remain recoverable");

    let patches = builder.drain_patches();
    let checked = check_patch_invariants(&patches, &baseline).expect("patches must stay valid");
    let live = builder.dom_invariant_state();
    check_dom_invariants(&live).expect("live DOM must stay valid");
    assert_eq!(live, checked);
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { .. })),
        "node-limited text insertion must not create a text node"
    );
    assert!(
        builder
            .take_parse_error_kinds_for_test()
            .contains(&"resource-limit-node-count")
    );
}

#[test]
fn tree_builder_child_limit_ignores_additional_children_and_preserves_invariants() {
    let (mut builder, mut ctx, resolver) = builder_with_limits(TreeBuilderLimits {
        max_children_per_node: 2,
        ..TreeBuilderLimits::default()
    });
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let _ = builder.drain_patches();

    builder
        .insert_literal_text("first")
        .expect("first child should insert");
    builder
        .insert_comment(&TextValue::Owned("filler".to_string()), &resolver)
        .expect("second child should insert up to the configured cap");
    let _ = builder.drain_patches();
    let baseline = builder.dom_invariant_state();
    let body_key = builder
        .state_snapshot()
        .open_element_keys
        .last()
        .copied()
        .expect("body should remain on SOE");

    builder
        .insert_comment(&TextValue::Owned("blocked".to_string()), &resolver)
        .expect("child-limited comment insert should remain recoverable");

    let patches = builder.drain_patches();
    let checked = check_patch_invariants(&patches, &baseline).expect("patches must stay valid");
    let live = builder.dom_invariant_state();
    check_dom_invariants(&live).expect("live DOM must stay valid");
    assert_eq!(live, checked);
    assert_eq!(builder.live_tree.child_count(body_key), 2);
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateComment { .. })),
        "child-limited insertion must not create a detached comment node"
    );
    assert!(
        builder
            .take_parse_error_kinds_for_test()
            .contains(&"resource-limit-children-per-node")
    );
}

#[test]
fn void_element_at_retained_depth_limit_restores_stack_and_records_real_transition() {
    use crate::html5::shared::Token;

    let (mut builder, mut ctx, resolver) = builder_with_limits(TreeBuilderLimits {
        max_open_elements_depth: 2,
        ..TreeBuilderLimits::default()
    });
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let input = ctx.atoms.intern_ascii_folded("input").expect("input atom");
    let before = builder.state_snapshot();
    let before_stats = builder.debug_perf_stats();

    for self_closing in [false, true] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: input,
                    attrs: Vec::new(),
                    self_closing,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("void input at retained limit should remain insertable");
    }

    let after = builder.state_snapshot();
    let after_stats = builder.debug_perf_stats();
    assert_eq!(after.open_element_keys, before.open_element_keys);
    assert_eq!(after.open_element_names, before.open_element_names);
    assert_eq!(after_stats.soe_push_ops, before_stats.soe_push_ops + 2);
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops + 2);
    assert_eq!(
        builder.max_open_elements_depth(),
        3,
        "high-water records the actual bounded void push above retained limit"
    );
}

#[test]
fn void_element_resource_failures_do_not_begin_stack_transition() {
    use crate::html5::shared::Token;

    let (mut node_limited, mut node_ctx, resolver) = builder_with_limits(TreeBuilderLimits {
        max_open_elements_depth: 2,
        ..TreeBuilderLimits::default()
    });
    let _ = enter_in_body(&mut node_limited, &mut node_ctx, &resolver);
    node_limited.config.limits.max_nodes_created = 3;
    let input = node_ctx
        .atoms
        .intern_ascii_folded("input")
        .expect("input atom");
    let before = node_limited.state_snapshot();
    let before_stats = node_limited.debug_perf_stats();
    let _ = node_limited
        .process(
            &Token::StartTag {
                name: input,
                attrs: Vec::new(),
                self_closing: false,
            },
            &node_ctx.atoms,
            &resolver,
        )
        .expect("node-limited void insertion remains recoverable");
    assert_eq!(
        node_limited.state_snapshot().open_element_keys,
        before.open_element_keys
    );
    let after_stats = node_limited.debug_perf_stats();
    assert_eq!(after_stats.soe_push_ops, before_stats.soe_push_ops);
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops);
    assert!(
        node_limited
            .take_parse_error_kinds_for_test()
            .contains(&"resource-limit-node-count")
    );

    let (mut child_limited, mut child_ctx, resolver) =
        builder_with_limits(TreeBuilderLimits::default());
    let _ = enter_in_body(&mut child_limited, &mut child_ctx, &resolver);
    child_limited.config.limits.max_children_per_node = 1;
    let input = child_ctx
        .atoms
        .intern_ascii_folded("input")
        .expect("input atom");
    let _ = child_limited
        .process(
            &Token::StartTag {
                name: input,
                attrs: Vec::new(),
                self_closing: false,
            },
            &child_ctx.atoms,
            &resolver,
        )
        .expect("first child at limit should insert");
    let before = child_limited.state_snapshot();
    let before_stats = child_limited.debug_perf_stats();
    let _ = child_limited
        .process(
            &Token::StartTag {
                name: input,
                attrs: Vec::new(),
                self_closing: true,
            },
            &child_ctx.atoms,
            &resolver,
        )
        .expect("child-limited void insertion remains recoverable");
    assert_eq!(
        child_limited.state_snapshot().open_element_keys,
        before.open_element_keys
    );
    let after_stats = child_limited.debug_perf_stats();
    assert_eq!(after_stats.soe_push_ops, before_stats.soe_push_ops);
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops);
    assert!(
        child_limited
            .take_parse_error_kinds_for_test()
            .contains(&"resource-limit-children-per-node")
    );
}
