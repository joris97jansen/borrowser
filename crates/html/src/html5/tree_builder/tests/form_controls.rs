use super::helpers::{EmptyResolver, enter_in_body};
use crate::dom_patch::DomPatch;
use crate::html5::shared::{Attribute, AttributeValue, DocumentParseContext, TextValue, Token};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderLimits};

fn in_body_builder() -> (Html5TreeBuilder, DocumentParseContext, EmptyResolver) {
    let mut ctx = DocumentParseContext::with_tree_observations_for_test();
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("tree builder init");
    let resolver = EmptyResolver;
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let _ = builder.drain_patches();
    (builder, ctx, resolver)
}

fn process(
    builder: &mut Html5TreeBuilder,
    token: Token,
    ctx: &mut DocumentParseContext,
    resolver: &EmptyResolver,
) {
    let _ = builder
        .process(
            &token,
            &mut crate::html5::tree_builder::TreeBuilderProcessContext::new(ctx),
            resolver,
        )
        .expect("form-control token should remain recoverable");
}

#[test]
fn direct_supported_void_rule_groups_acknowledge_self_closing_flags() {
    use crate::html5::shared::{
        ImplementationDiagnosticCode, ParseErrorCode, TreeConstructionImplementationDiagnosticCode,
        TreeConstructionParseErrorCode,
    };

    fn assert_no_self_closing_failure(mut ctx: DocumentParseContext, label: &str) {
        let capture = ctx.take_observations().expect("explicit direct capture");
        assert!(
            !capture.parse_errors.items.iter().any(|event| {
                event.code
                    == ParseErrorCode::TreeConstruction(
                        TreeConstructionParseErrorCode::UnacknowledgedSelfClosingFlag,
                    )
            }),
            "direct rule did not acknowledge a supported void token: {label}"
        );
        assert!(
            !capture
                .implementation_diagnostics
                .items
                .iter()
                .any(|event| {
                    event.code()
                        == ImplementationDiagnosticCode::TreeConstruction(
                            TreeConstructionImplementationDiagnosticCode::
                                NonVoidHtmlSelfClosingFlagAlteredStackDisposition,
                        )
                }),
            "direct supported void rule selected LegacySkipPush: {label}"
        );
    }

    {
        let (mut builder, mut ctx, resolver) = in_body_builder();
        for local_name in [
            "area", "base", "basefont", "bgsound", "br", "embed", "img", "param", "source",
            "track", "wbr", "input", "hr", "keygen",
        ] {
            let name = ctx
                .atoms
                .intern_ascii_folded(local_name)
                .expect("void atom");
            process(
                &mut builder,
                Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: true,
                },
                &mut ctx,
                &resolver,
            );
        }
        assert_no_self_closing_failure(ctx, "in-body void groups");
    }

    {
        let mut ctx = DocumentParseContext::with_tree_observations_for_test();
        let mut builder =
            Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("head builder");
        let resolver = EmptyResolver;
        let html = ctx.atoms.intern_ascii_folded("html").expect("html");
        let head = ctx.atoms.intern_ascii_folded("head").expect("head");
        process(
            &mut builder,
            Token::Doctype {
                name: Some(html),
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            &mut ctx,
            &resolver,
        );
        process(
            &mut builder,
            Token::StartTag {
                name: html,
                attrs: Vec::new(),
                self_closing: false,
            },
            &mut ctx,
            &resolver,
        );
        process(
            &mut builder,
            Token::StartTag {
                name: head,
                attrs: Vec::new(),
                self_closing: false,
            },
            &mut ctx,
            &resolver,
        );
        for local_name in ["base", "link", "meta"] {
            let name = ctx
                .atoms
                .intern_ascii_folded(local_name)
                .expect("head void atom");
            process(
                &mut builder,
                Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: true,
                },
                &mut ctx,
                &resolver,
            );
        }
        assert_no_self_closing_failure(ctx, "in-head void group");
    }

    {
        let (mut builder, mut ctx, resolver) = in_body_builder();
        for (local_name, self_closing) in [("table", false), ("colgroup", false), ("col", true)] {
            let name = ctx
                .atoms
                .intern_ascii_folded(local_name)
                .expect("table atom");
            process(
                &mut builder,
                Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing,
                },
                &mut ctx,
                &resolver,
            );
        }
        assert_no_self_closing_failure(ctx, "in-column-group col rule");
    }

    {
        let (mut builder, mut ctx, resolver) = in_body_builder();
        for (local_name, self_closing) in [("svg", false), ("path", true)] {
            let name = ctx
                .atoms
                .intern_ascii_folded(local_name)
                .expect("foreign atom");
            process(
                &mut builder,
                Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing,
                },
                &mut ctx,
                &resolver,
            );
        }
        assert_no_self_closing_failure(ctx, "foreign-content self-closing rule");
    }
}

#[test]
fn form_pointer_sets_after_creation_rejects_duplicate_and_clears_on_end_tag() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");

    assert_eq!(builder.state_snapshot().form_element_pointer, None);
    process(
        &mut builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    let set = builder.state_snapshot();
    let form_key = set
        .form_element_pointer
        .expect("form pointer after insertion");
    assert_eq!(
        builder.progress_witness().form_element_pointer,
        Some(form_key)
    );
    assert_eq!(set.open_element_keys.last().copied(), Some(form_key));

    process(
        &mut builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    assert_eq!(
        builder.state_snapshot().form_element_pointer,
        Some(form_key)
    );
    assert!(
        ctx.take_tree_parse_error_descriptions_for_test()
            .contains(&"in-body-form-start-tag-with-active-form-pointer")
    );

    let patches_before_end = builder.drain_patches();
    process(
        &mut builder,
        Token::EndTag { name: form },
        &mut ctx,
        &resolver,
    );
    let after = builder.state_snapshot();
    assert_eq!(after.form_element_pointer, None);
    assert!(!after.open_element_keys.contains(&form_key));
    let end_patches = builder.drain_patches();
    assert!(
        patches_before_end
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateElement { key, .. } if *key == form_key))
    );
    assert!(
        end_patches
            .iter()
            .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. }))
    );
}

#[test]
fn form_end_clears_pointer_before_failed_scope_validation() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");
    process(
        &mut builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    let form_key = builder
        .state_snapshot()
        .form_element_pointer
        .expect("pointer set");
    let _ = builder
        .remove_open_element_exact(form_key)
        .expect("test removes pointed form from stack only");

    process(
        &mut builder,
        Token::EndTag { name: form },
        &mut ctx,
        &resolver,
    );
    assert_eq!(builder.state_snapshot().form_element_pointer, None);
    assert!(
        ctx.take_tree_parse_error_descriptions_for_test()
            .contains(&"in-body-form-end-tag-pointer-not-in-scope")
    );
}

#[test]
fn form_end_removes_exact_non_current_stack_entry_without_dom_removal() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");
    let div = ctx.atoms.intern_ascii_folded("div").expect("div atom");
    process(
        &mut builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    let form_key = builder
        .state_snapshot()
        .form_element_pointer
        .expect("pointer set");
    let div_key = builder
        .insert_normal_html_element_for_test(div, &[], &mut ctx, &resolver)
        .expect("div insertion")
        .expect("div created");
    let before_stats = builder.debug_perf_stats();
    let _ = builder.drain_patches();

    process(
        &mut builder,
        Token::EndTag { name: form },
        &mut ctx,
        &resolver,
    );
    let state = builder.state_snapshot();
    assert_eq!(state.form_element_pointer, None);
    assert!(!state.open_element_keys.contains(&form_key));
    assert_eq!(state.open_element_keys.last().copied(), Some(div_key));
    assert!(
        ctx.take_tree_parse_error_descriptions_for_test()
            .contains(&"in-body-form-end-tag-non-current-form")
    );
    assert!(
        builder
            .drain_patches()
            .iter()
            .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. }))
    );
    let after_stats = builder.debug_perf_stats();
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops + 1);
    assert_eq!(
        after_stats.text_coalescing_invalidations,
        before_stats.text_coalescing_invalidations + 1,
        "exact non-current removal is a structural text-coalescing boundary"
    );
}

#[test]
fn unmatched_form_end_is_recoverable_and_unclosed_pointer_survives_eof() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");

    process(
        &mut builder,
        Token::EndTag { name: form },
        &mut ctx,
        &resolver,
    );
    assert!(
        ctx.take_tree_parse_error_descriptions_for_test()
            .contains(&"in-body-form-end-tag-without-form-pointer")
    );

    process(
        &mut builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    let pointer = builder
        .state_snapshot()
        .form_element_pointer
        .expect("form pointer after successful creation");
    process(&mut builder, Token::Eof, &mut ctx, &resolver);
    assert_eq!(
        builder.state_snapshot().form_element_pointer,
        Some(pointer),
        "EOF does not perform artificial form-pointer cleanup"
    );
}

#[test]
fn form_resource_limit_failure_does_not_set_pointer() {
    let mut ctx = DocumentParseContext::with_tree_observations_for_test();
    let mut builder = Html5TreeBuilder::new(
        TreeBuilderConfig {
            limits: TreeBuilderLimits {
                max_open_elements_depth: 2,
                ..TreeBuilderLimits::default()
            },
            ..TreeBuilderConfig::default()
        },
        &mut ctx,
    )
    .expect("tree builder init");
    let resolver = EmptyResolver;
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");
    process(
        &mut builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    assert_eq!(builder.state_snapshot().form_element_pointer, None);
    assert!(
        ctx.take_tree_implementation_diagnostic_descriptions_for_test()
            .contains(&"resource-limit-soe-depth")
    );
}

#[test]
fn in_table_form_resource_failure_leaves_pointer_stack_and_patches_unchanged() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let table = ctx.atoms.intern_ascii_folded("table").expect("table atom");
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");
    let _ = builder
        .insert_normal_html_element_for_test(table, &[], &mut ctx, &resolver)
        .expect("table insertion")
        .expect("table creation");
    builder.insertion_mode = InsertionMode::InTable;
    // A completed dispatch refreshes the externally exposed performance
    // snapshot; the following comment leaves the open-elements stack intact.
    process(
        &mut builder,
        Token::Comment {
            text: TextValue::Owned("sync perf snapshot".to_string()),
        },
        &mut ctx,
        &resolver,
    );
    let _ = builder.drain_patches();
    builder.config.limits.max_nodes_created = builder.non_document_nodes_created;
    let before = builder.state_snapshot();
    let before_stats = builder.debug_perf_stats();

    process(
        &mut builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );

    let after = builder.state_snapshot();
    let after_stats = builder.debug_perf_stats();
    assert_eq!(after.form_element_pointer, None);
    assert_eq!(after.open_element_keys, before.open_element_keys);
    assert_eq!(after_stats.soe_push_ops, before_stats.soe_push_ops);
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops);
    assert!(builder.drain_patches().is_empty());
    let (parse_errors, diagnostics) = ctx.take_tree_diagnostic_descriptions_for_test();
    assert_eq!(parse_errors, vec!["in-table-form-start-tag"]);
    assert_eq!(diagnostics, vec!["resource-limit-node-count"]);
}

#[test]
fn exact_same_name_stack_removal_keeps_the_other_dom_identity_and_emits_no_patch() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let form = ctx.atoms.intern_ascii_folded("form").expect("form atom");
    let first = builder
        .insert_normal_html_element_for_test(form, &[], &mut ctx, &resolver)
        .expect("first direct form insertion")
        .expect("first form creation");
    let second = builder
        .insert_normal_html_element_for_test(form, &[], &mut ctx, &resolver)
        .expect("second direct form insertion")
        .expect("second form creation");
    let before = builder.state_snapshot();
    let _ = builder.drain_patches();

    let removed = builder
        .remove_open_element_exact(first)
        .expect("exact same-name identity removal");
    let after = builder.state_snapshot();
    assert_eq!(removed.removed.key(), first);
    assert_eq!(removed.removed.name(), form);
    assert!(!after.open_element_keys.contains(&first));
    assert!(after.open_element_keys.contains(&second));
    assert_eq!(
        after.open_element_keys,
        before
            .open_element_keys
            .into_iter()
            .filter(|key| *key != first)
            .collect::<Vec<_>>()
    );
    assert!(builder.drain_patches().is_empty());
}

#[test]
fn ae9_self_closing_finalization_reports_each_non_void_path_once_after_recovery() {
    let (mut form_builder, mut form_ctx, resolver) = in_body_builder();
    let form = form_ctx
        .atoms
        .intern_ascii_folded("form")
        .expect("form atom");
    process(
        &mut form_builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut form_ctx,
        &resolver,
    );
    let form_key = form_builder
        .state_snapshot()
        .form_element_pointer
        .expect("self-closing form still creates a normal form element");
    assert_eq!(
        form_ctx.take_tree_parse_error_descriptions_for_test(),
        vec!["non-void-html-element-start-tag-with-trailing-solidus"]
    );
    form_ctx.enable_tree_observations_for_test();
    let before_duplicate = form_builder.state_snapshot();
    let _ = form_builder.drain_patches();
    process(
        &mut form_builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut form_ctx,
        &resolver,
    );
    assert_eq!(
        form_builder.state_snapshot().form_element_pointer,
        Some(form_key)
    );
    assert_eq!(
        form_builder.state_snapshot().open_element_keys,
        before_duplicate.open_element_keys
    );
    assert!(form_builder.drain_patches().is_empty());
    assert_eq!(
        form_ctx.take_tree_parse_error_descriptions_for_test(),
        vec![
            "in-body-form-start-tag-with-active-form-pointer",
            "non-void-html-element-start-tag-with-trailing-solidus",
        ]
    );

    let (mut textarea_builder, mut textarea_ctx, resolver) = in_body_builder();
    let textarea = textarea_ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("textarea atom");
    process(
        &mut textarea_builder,
        Token::StartTag {
            name: textarea,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut textarea_ctx,
        &resolver,
    );
    let textarea_state = textarea_builder.state_snapshot();
    assert_eq!(textarea_state.insertion_mode, InsertionMode::Text);
    assert_eq!(
        textarea_state.original_insertion_mode,
        Some(InsertionMode::InBody)
    );
    assert_eq!(
        textarea_ctx.take_tree_parse_error_descriptions_for_test(),
        vec!["non-void-html-element-start-tag-with-trailing-solidus"]
    );

    let (mut button_builder, mut button_ctx, resolver) = in_body_builder();
    let button = button_ctx
        .atoms
        .intern_ascii_folded("button")
        .expect("button atom");
    process(
        &mut button_builder,
        Token::StartTag {
            name: button,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut button_ctx,
        &resolver,
    );
    let _ = button_ctx.take_tree_parse_error_descriptions_for_test();
    button_ctx.enable_tree_observations_for_test();
    process(
        &mut button_builder,
        Token::StartTag {
            name: button,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut button_ctx,
        &resolver,
    );
    assert_eq!(
        button_builder
            .state_snapshot()
            .open_element_names
            .iter()
            .filter(|name| **name == button)
            .count(),
        1
    );
    assert_eq!(
        button_ctx.take_tree_parse_error_descriptions_for_test(),
        vec![
            "in-body-button-start-tag-with-button-in-scope",
            "non-void-html-element-start-tag-with-trailing-solidus",
        ]
    );

    let (mut fieldset_builder, mut fieldset_ctx, resolver) = in_body_builder();
    let fieldset = fieldset_ctx
        .atoms
        .intern_ascii_folded("fieldset")
        .expect("fieldset atom");
    process(
        &mut fieldset_builder,
        Token::StartTag {
            name: fieldset,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut fieldset_ctx,
        &resolver,
    );
    assert_eq!(
        fieldset_builder.state_snapshot().open_element_names.last(),
        Some(&fieldset)
    );
    assert_eq!(
        fieldset_ctx.take_tree_parse_error_descriptions_for_test(),
        vec!["non-void-html-element-start-tag-with-trailing-solidus"]
    );
}

#[test]
fn in_table_ignored_form_and_ae9_void_tokens_finalize_self_closing_flags() {
    let (mut table_builder, mut table_ctx, resolver) = in_body_builder();
    let table = table_ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("table atom");
    let form = table_ctx
        .atoms
        .intern_ascii_folded("form")
        .expect("form atom");
    let _ = table_builder
        .insert_normal_html_element_for_test(table, &[], &mut table_ctx, &resolver)
        .expect("table insertion")
        .expect("table creation");
    table_builder.insertion_mode = InsertionMode::InTable;
    process(
        &mut table_builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut table_ctx,
        &resolver,
    );
    let pointer = table_builder
        .state_snapshot()
        .form_element_pointer
        .expect("first InTable form sets pointer");
    let before = table_builder.state_snapshot();
    let _ = table_builder.drain_patches();
    process(
        &mut table_builder,
        Token::StartTag {
            name: form,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut table_ctx,
        &resolver,
    );
    assert_eq!(
        table_builder.state_snapshot().form_element_pointer,
        Some(pointer)
    );
    assert_eq!(
        table_builder.state_snapshot().open_element_keys,
        before.open_element_keys
    );
    assert!(table_builder.drain_patches().is_empty());
    let events = table_ctx.take_tree_parse_errors_for_test();
    assert_eq!(
        events
            .iter()
            .filter_map(|event| event.description)
            .collect::<Vec<_>>(),
        vec![
            "in-table-form-start-tag",
            "in-table-form-start-tag",
            "in-table-form-start-tag-with-active-form-pointer",
            "non-void-html-element-start-tag-with-trailing-solidus",
        ]
    );
    assert!(events.iter().all(|event| {
        let context = event.context.as_ref().expect("tree event context");
        context.token_kind == Some(crate::html5::shared::ParserTokenKind::StartTag)
            && context.insertion_mode == Some(crate::html5::shared::ObservedInsertionMode::InTable)
            && event.position
                == crate::html5::shared::EventPosition::Unavailable(
                    crate::html5::shared::PositionUnavailableReason::ParserDidNotProvidePosition,
                )
    }));
    assert_eq!(
        events[1..]
            .iter()
            .map(|event| event.occurrence)
            .collect::<Vec<_>>(),
        vec![2, 3, 4],
        "multiple detections during one successfully completed token retain exact order"
    );
    assert_eq!(
        events[1..]
            .iter()
            .map(|event| event.code)
            .collect::<Vec<_>>(),
        vec![
            crate::html5::shared::ParseErrorCode::TreeConstruction(
                crate::html5::shared::TreeConstructionParseErrorCode::FormStartTagInTable,
            ),
            crate::html5::shared::ParseErrorCode::TreeConstruction(
                crate::html5::shared::TreeConstructionParseErrorCode::
                    FormStartTagWithActiveFormPointer,
            ),
            crate::html5::shared::ParseErrorCode::TreeConstruction(
                crate::html5::shared::TreeConstructionParseErrorCode::
                    UnacknowledgedSelfClosingFlag,
            ),
        ]
    );

    let (mut void_builder, mut void_ctx, resolver) = in_body_builder();
    let input = void_ctx
        .atoms
        .intern_ascii_folded("input")
        .expect("input atom");
    let keygen = void_ctx
        .atoms
        .intern_ascii_folded("keygen")
        .expect("keygen atom");
    let before_stats = void_builder.debug_perf_stats();
    for name in [input, keygen] {
        process(
            &mut void_builder,
            Token::StartTag {
                name,
                attrs: Vec::new(),
                self_closing: true,
            },
            &mut void_ctx,
            &resolver,
        );
    }
    let after_stats = void_builder.debug_perf_stats();
    assert_eq!(after_stats.soe_push_ops, before_stats.soe_push_ops + 2);
    assert_eq!(after_stats.soe_pop_ops, before_stats.soe_pop_ops + 2);
    assert!(
        void_ctx
            .take_tree_parse_error_descriptions_for_test()
            .is_empty()
    );
}

#[test]
fn textarea_suppresses_exactly_one_initial_line_feed_and_clears_pending_state() {
    let cases = [("", ""), ("\n", ""), ("\ntext", "text"), ("\n\n", "\n")];
    for (source, expected) in cases {
        let (mut builder, mut ctx, resolver) = in_body_builder();
        let textarea = ctx
            .atoms
            .intern_ascii_folded("textarea")
            .expect("textarea atom");
        process(
            &mut builder,
            Token::StartTag {
                name: textarea,
                attrs: Vec::new(),
                self_closing: false,
            },
            &mut ctx,
            &resolver,
        );
        assert!(
            builder
                .state_snapshot()
                .pending_textarea_initial_lf
                .is_some()
        );
        assert!(
            builder
                .progress_witness()
                .pending_textarea_initial_lf
                .is_some()
        );
        if !source.is_empty() {
            process(
                &mut builder,
                Token::Text {
                    text: TextValue::Owned(source.to_string()),
                },
                &mut ctx,
                &resolver,
            );
        }
        process(
            &mut builder,
            Token::EndTag { name: textarea },
            &mut ctx,
            &resolver,
        );
        assert_eq!(builder.state_snapshot().pending_textarea_initial_lf, None);
        let text: String = builder
            .drain_patches()
            .into_iter()
            .filter_map(|patch| match patch {
                DomPatch::CreateText { text, .. } | DomPatch::AppendText { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(text, expected, "textarea source {source:?}");
    }
}

#[test]
fn input_button_fieldset_and_keygen_use_their_supported_parser_categories() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let input = ctx.atoms.intern_ascii_folded("input").expect("input atom");
    let button = ctx
        .atoms
        .intern_ascii_folded("button")
        .expect("button atom");
    let fieldset = ctx
        .atoms
        .intern_ascii_folded("fieldset")
        .expect("fieldset atom");
    let keygen = ctx
        .atoms
        .intern_ascii_folded("keygen")
        .expect("keygen atom");
    let type_attr = ctx.atoms.intern_ascii_folded("type").expect("type atom");

    process(
        &mut builder,
        Token::StartTag {
            name: input,
            attrs: vec![Attribute {
                name: type_attr,
                value: AttributeValue::Owned("hidden".to_string()),
            }],
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    assert!(builder.state_snapshot().frameset_ok);
    assert!(!builder.state_snapshot().open_element_names.contains(&input));

    process(
        &mut builder,
        Token::StartTag {
            name: input,
            attrs: Vec::new(),
            self_closing: true,
        },
        &mut ctx,
        &resolver,
    );
    assert!(!builder.state_snapshot().frameset_ok);
    assert!(!builder.state_snapshot().open_element_names.contains(&input));

    process(
        &mut builder,
        Token::StartTag {
            name: fieldset,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    assert_eq!(
        builder.state_snapshot().open_element_names.last(),
        Some(&fieldset)
    );
    process(
        &mut builder,
        Token::EndTag { name: fieldset },
        &mut ctx,
        &resolver,
    );

    process(
        &mut builder,
        Token::StartTag {
            name: button,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    process(
        &mut builder,
        Token::StartTag {
            name: button,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    assert_eq!(
        builder
            .state_snapshot()
            .open_element_names
            .iter()
            .filter(|name| **name == button)
            .count(),
        1
    );
    assert!(
        ctx.take_tree_parse_error_descriptions_for_test()
            .contains(&"in-body-button-start-tag-with-button-in-scope")
    );

    process(
        &mut builder,
        Token::StartTag {
            name: keygen,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    assert!(
        !builder
            .state_snapshot()
            .open_element_names
            .contains(&keygen)
    );
}

#[test]
fn textarea_initial_lf_is_whole_and_chunked_input_equivalent() {
    let input = "<!doctype html><textarea>\n\ntext</textarea>";
    let whole = super::helpers::run_tree_builder_chunks(&[input]);
    for split in 1..input.len() {
        let chunked = super::helpers::run_tree_builder_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "textarea tree construction must be equivalent at source boundary {split}"
        );
    }
}

#[test]
fn form_pointer_and_button_recovery_are_whole_and_every_boundary_chunked_equivalent() {
    for input in [
        "<!doctype html><form><form><input></form></form>",
        "<!doctype html><form><div></form>",
        "<!doctype html><button>one<button>two</button>",
    ] {
        let whole = super::helpers::run_tree_builder_chunks(&[input]);
        for split in 1..input.len() {
            let chunked =
                super::helpers::run_tree_builder_chunks(&[&input[..split], &input[split..]]);
            assert_eq!(
                chunked, whole,
                "parser form/button state must be equivalent at source boundary {split} for {input:?}"
            );
        }
    }
}

#[test]
fn textarea_pending_initial_lf_clears_without_leaking_across_non_text_or_eof() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("textarea atom");
    for terminator in [
        Token::Comment {
            text: TextValue::Owned("comment".to_string()),
        },
        Token::Eof,
    ] {
        process(
            &mut builder,
            Token::StartTag {
                name: textarea,
                attrs: Vec::new(),
                self_closing: false,
            },
            &mut ctx,
            &resolver,
        );
        assert!(
            builder
                .state_snapshot()
                .pending_textarea_initial_lf
                .is_some()
        );
        process(&mut builder, terminator, &mut ctx, &resolver);
        assert_eq!(builder.state_snapshot().pending_textarea_initial_lf, None);
        if builder.state_snapshot().active_text_mode.is_some() {
            process(
                &mut builder,
                Token::EndTag { name: textarea },
                &mut ctx,
                &resolver,
            );
        }
    }
}

#[test]
fn fieldset_closes_paragraph_and_unmatched_button_end_is_recoverable() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let p = ctx.atoms.intern_ascii_folded("p").expect("p atom");
    let fieldset = ctx
        .atoms
        .intern_ascii_folded("fieldset")
        .expect("fieldset atom");
    let button = ctx
        .atoms
        .intern_ascii_folded("button")
        .expect("button atom");

    process(
        &mut builder,
        Token::StartTag {
            name: p,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    process(
        &mut builder,
        Token::StartTag {
            name: fieldset,
            attrs: Vec::new(),
            self_closing: false,
        },
        &mut ctx,
        &resolver,
    );
    let state = builder.state_snapshot();
    assert!(!state.open_element_names.contains(&p));
    assert_eq!(state.open_element_names.last().copied(), Some(fieldset));

    process(
        &mut builder,
        Token::EndTag { name: button },
        &mut ctx,
        &resolver,
    );
    assert!(
        ctx.take_tree_parse_error_descriptions_for_test()
            .contains(&"in-body-button-end-tag-not-in-scope")
    );
}
