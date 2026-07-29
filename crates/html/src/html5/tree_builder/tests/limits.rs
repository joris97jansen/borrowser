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
    let mut ctx = DocumentParseContext::with_tree_observations_for_test();
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
        .insert_normal_html_element_for_test(div, &[], &mut ctx, &resolver)
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
        ctx.take_tree_implementation_diagnostic_descriptions_for_test()
            .contains(&"resource-limit-soe-depth")
    );
    assert_eq!(
        ctx.counters.parse_errors, 0,
        "configured resource limits are not recoverable HTML parse errors"
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
        .insert_literal_text_for_test("blocked", &mut ctx)
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
        ctx.take_tree_implementation_diagnostic_descriptions_for_test()
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
        .insert_literal_text_for_test("first", &mut ctx)
        .expect("first child should insert");
    builder
        .insert_comment_for_test(&TextValue::Owned("filler".to_string()), &mut ctx, &resolver)
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
        .insert_comment_for_test(
            &TextValue::Owned("blocked".to_string()),
            &mut ctx,
            &resolver,
        )
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
        ctx.take_tree_implementation_diagnostic_descriptions_for_test()
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
                &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
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
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut node_ctx),
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
        node_ctx
            .take_tree_implementation_diagnostic_descriptions_for_test()
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
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut child_ctx),
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
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut child_ctx),
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
        child_ctx
            .take_tree_implementation_diagnostic_descriptions_for_test()
            .contains(&"resource-limit-children-per-node")
    );
}

#[test]
fn select_family_recovery_remains_committed_when_following_insertions_hit_node_limit() {
    use crate::html5::shared::Token;

    for inserted_name in ["option", "input", "hr"] {
        let (mut builder, mut ctx, resolver) = builder_with_limits(TreeBuilderLimits::default());
        let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
        let select = ctx.atoms.intern_ascii_folded("select").expect("select");
        let option = ctx.atoms.intern_ascii_folded("option").expect("option");
        let inserted = ctx
            .atoms
            .intern_ascii_folded(inserted_name)
            .expect("inserted tag");
        for name in [select, option] {
            let _ = builder
                .process(
                    &Token::StartTag {
                        name,
                        attrs: Vec::new(),
                        self_closing: false,
                    },
                    &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
                    &resolver,
                )
                .expect("select setup should process");
        }
        builder.config.limits.max_nodes_created = 5;
        let _ = builder.drain_patches();

        let _ = builder
            .process(
                &Token::StartTag {
                    name: inserted,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
                &resolver,
            )
            .expect("resource-limited recovery remains recoverable");

        let state = builder.state_snapshot();
        match inserted_name {
            "option" => assert_eq!(
                state.open_element_names,
                vec![builder.known_tags.html, builder.known_tags.body, select]
            ),
            "input" => assert_eq!(
                state.open_element_names,
                vec![builder.known_tags.html, builder.known_tags.body]
            ),
            "hr" => assert_eq!(
                state.open_element_names,
                vec![builder.known_tags.html, builder.known_tags.body, select]
            ),
            _ => unreachable!(),
        }
        let diagnostics = ctx.take_tree_implementation_diagnostic_descriptions_for_test();
        assert_eq!(diagnostics.last(), Some(&"resource-limit-node-count"));
        assert!(
            builder.drain_patches().is_empty(),
            "rejected {inserted_name} must not emit detached structural patches"
        );
    }
}

#[test]
fn fostered_select_resource_failure_keeps_table_stack_and_cache_consistent() {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let (mut builder, mut ctx, resolver) = builder_with_limits(TreeBuilderLimits::default());
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let table = ctx.atoms.intern_ascii_folded("table").expect("table");
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let _ = builder
        .process(
            &Token::StartTag {
                name: table,
                attrs: Vec::new(),
                self_closing: false,
            },
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
            &resolver,
        )
        .expect("table setup");
    builder.config.limits.max_nodes_created = 4;
    let before = builder.state_snapshot();
    let _ = builder.drain_patches();

    let _ = builder
        .process(
            &Token::StartTag {
                name: select,
                attrs: Vec::new(),
                self_closing: false,
            },
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
            &resolver,
        )
        .expect("fostered select limit rejection remains recoverable");

    let after = builder.state_snapshot();
    assert_eq!(after.open_element_keys, before.open_element_keys);
    assert_eq!(after.current_table_key, before.current_table_key);
    assert_eq!(after.insertion_mode, InsertionMode::InTable);
    assert!(!builder.progress_witness().foster_parenting_enabled);
    let (parse_errors, diagnostics) = ctx.take_tree_diagnostic_descriptions_for_test();
    assert_eq!(
        parse_errors,
        vec!["in-table-anything-else-reprocess-in-body"]
    );
    assert_eq!(diagnostics, vec!["resource-limit-node-count"]);
    assert_eq!(
        ctx.counters.parse_errors, 1,
        "only the unacknowledged self-closing flag increments the HTML parse-error counter"
    );
    assert!(builder.drain_patches().is_empty());
}

#[test]
fn resource_rejected_non_void_select_start_still_finalizes_self_closing_once() {
    use crate::html5::shared::Token;

    let (mut builder, mut ctx, resolver) = builder_with_limits(TreeBuilderLimits::default());
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    builder.config.limits.max_nodes_created = 3;
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");

    let _ = builder
        .process(
            &Token::StartTag {
                name: select,
                attrs: Vec::new(),
                self_closing: true,
            },
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut ctx),
            &resolver,
        )
        .expect("resource-rejected select remains recoverable");

    let (parse_errors, implementation_diagnostics) =
        ctx.take_tree_diagnostic_descriptions_for_test();
    assert_eq!(
        parse_errors,
        vec!["non-void-html-element-start-tag-with-trailing-solidus"],
        "dispatch must finalize the original flag once after recoverable insertion rejection"
    );
    assert_eq!(
        implementation_diagnostics,
        vec!["resource-limit-node-count"]
    );
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body]
    );
    assert!(builder.drain_patches().is_empty());
}

#[test]
fn resource_suppressed_legacy_self_closing_insertion_never_claims_altered_stack() {
    use crate::html5::shared::{
        EventPosition, ImplementationDiagnosticCode, ParseErrorCode, ParserRecoveryAction,
        ParserResourceLimit, PositionUnavailableReason, Token,
        TreeConstructionImplementationDiagnosticCode, TreeConstructionParseErrorCode,
    };

    enum LimitCase {
        OpenElements,
        Nodes,
        Children,
    }

    for (case, expected_limit) in [
        (
            LimitCase::OpenElements,
            ParserResourceLimit::TreeOpenElementsDepth,
        ),
        (LimitCase::Nodes, ParserResourceLimit::TreeNodeCount),
        (
            LimitCase::Children,
            ParserResourceLimit::TreeChildrenPerNode,
        ),
    ] {
        let (mut builder, mut context, resolver) =
            builder_with_limits(TreeBuilderLimits::default());
        let _ = enter_in_body(&mut builder, &mut context, &resolver);
        let _ = builder.drain_patches();
        match case {
            LimitCase::OpenElements => {
                builder.config.limits.max_open_elements_depth =
                    builder.state_snapshot().open_element_keys.len();
            }
            LimitCase::Nodes => {
                builder.config.limits.max_nodes_created = 3;
            }
            LimitCase::Children => {
                builder.config.limits.max_children_per_node = 0;
            }
        }
        let div = context.atoms.intern_ascii_folded("div").expect("div atom");
        let before = builder.dom_invariant_state();
        let step = builder
            .process(
                &Token::StartTag {
                    name: div,
                    attrs: Vec::new(),
                    self_closing: true,
                },
                &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(&mut context),
                &resolver,
            )
            .expect("configured suppression remains recoverable");

        assert!(matches!(
            step.flow,
            crate::html5::tree_builder::TreeBuilderControlFlow::Continue
        ));
        assert_eq!(builder.dom_invariant_state(), before);
        assert!(builder.drain_patches().is_empty());
        assert_eq!(context.counters.parse_errors, 1);

        let capture = context.take_observations().expect("explicit capture");
        assert_eq!(capture.parse_errors.dropped, 0);
        assert_eq!(capture.implementation_diagnostics.dropped, 0);
        assert_eq!(capture.parse_errors.items.len(), 1);
        let parse_error = &capture.parse_errors.items[0];
        assert_eq!(parse_error.occurrence, 1);
        assert_eq!(
            parse_error.code,
            ParseErrorCode::TreeConstruction(
                TreeConstructionParseErrorCode::UnacknowledgedSelfClosingFlag,
            )
        );
        assert_eq!(
            parse_error.recovery,
            Some(ParserRecoveryAction::IgnoreSelfClosingFlag)
        );
        assert_eq!(
            parse_error.position,
            EventPosition::Unavailable(PositionUnavailableReason::ParserDidNotProvidePosition,)
        );

        assert_eq!(capture.implementation_diagnostics.items.len(), 1);
        let diagnostic = &capture.implementation_diagnostics.items[0];
        assert_eq!(diagnostic.occurrence(), 1);
        assert_eq!(
            diagnostic.code(),
            ImplementationDiagnosticCode::ParserResourceLimitActivated(expected_limit)
        );
        assert_ne!(
            diagnostic.code(),
            ImplementationDiagnosticCode::TreeConstruction(
                TreeConstructionImplementationDiagnosticCode::
                    NonVoidHtmlSelfClosingFlagAlteredStackDisposition,
            )
        );
    }
}
