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
        .insert_element(div, &[], false, &ctx.atoms, &resolver)
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
        max_nodes_created: 3,
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
