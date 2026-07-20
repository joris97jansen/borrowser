use super::helpers::{
    EmptyResolver, enter_in_body, materialized_dom_lines, run_tree_builder_chunks,
};
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{DocumentParseContext, TextValue, Token};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{
    AfeDiagnosticEntry, AfeMarker, AfeMarkerKind, TemplateInsertionMode,
};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};
use std::num::NonZeroU32;

#[test]
fn template_start_creates_atomic_typed_contents_and_uses_it_as_insertion_target() {
    let patches = run_tree_builder_chunks(&["<template><div>x</div></template><p>after"]);
    let (host, contents) = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateTemplateContents { host, contents } => Some((*host, *contents)),
            _ => None,
        })
        .expect("template contents association patch");
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateElement { key, name, .. } if *key == host && name.is_html("template")))
    );
    let div = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. } if name.is_html("div") => Some(*key),
            _ => None,
        })
        .expect("div key");
    assert!(patches.iter().any(|patch| matches!(
        patch,
        DomPatch::AppendChild { parent, child } if *parent == contents && *child == div
    )));
    assert!(!patches.iter().any(|patch| matches!(
        patch,
        DomPatch::AppendChild { parent, child } if *parent == host && *child == div
    )));

    let lines = materialized_dom_lines(&["<template><div>x</div></template><p>after"]);
    let template_index = lines
        .iter()
        .position(|line| line.contains("local=\"template\""))
        .expect("template snapshot line");
    assert_eq!(lines[template_index + 1].trim(), "#template-contents");
    assert!(lines[template_index + 2].contains("local=\"div\""));
    assert!(lines.iter().any(|line| line.contains("local=\"p\"")));
}

#[test]
fn template_state_pushes_and_pops_owner_aware_mode_entry() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();

    let _ = builder
        .process(
            &Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();
    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTemplate);
    assert_eq!(state.template_modes.len(), 1);
    let owner = state.template_modes[0].0;
    assert_eq!(state.open_element_keys.last().copied(), Some(owner));
    assert_eq!(
        state.active_formatting_entries.last(),
        Some(&AfeDiagnosticEntry::Marker(AfeMarker::new(
            AfeMarkerKind::Template,
            Some(owner),
        )))
    );

    let _ = builder
        .process(&Token::EndTag { name: template }, &ctx.atoms, &resolver)
        .unwrap();
    let state = builder.state_snapshot();
    assert!(state.template_modes.is_empty());
    assert_eq!(state.insertion_mode, InsertionMode::InBody);
    assert!(!state.open_element_keys.contains(&owner));
}

#[test]
fn in_template_replaces_the_owner_mode_before_reprocessing_table_tokens() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    let col = ctx.atoms.intern_ascii_folded("col").unwrap();

    for name in [template, col] {
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
            .unwrap();
    }

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InColumnGroup);
    assert_eq!(state.template_modes.len(), 1);
    assert_eq!(
        state.template_modes[0].1,
        TemplateInsertionMode::InColumnGroup
    );
    assert_eq!(
        state.open_element_keys.last().copied(),
        state.template_modes.first().map(|entry| entry.0)
    );
}

#[test]
fn after_head_template_delegation_uses_the_saved_head_pointer() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let html = ctx.atoms.intern_ascii_folded("html").unwrap();
    let head = ctx.atoms.intern_ascii_folded("head").unwrap();
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();

    for token in [
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
        Token::EndTag { name: head },
        Token::StartTag {
            name: template,
            attrs: Vec::new(),
            self_closing: false,
        },
    ] {
        let _ = builder.process(&token, &ctx.atoms, &resolver).unwrap();
    }

    let state = builder.state_snapshot();
    let head_key = state.head_element_pointer.expect("saved head pointer");
    let template_key = state.template_modes[0].0;
    let patches = builder.drain_patches();
    assert!(patches.iter().any(|patch| matches!(
        patch,
        DomPatch::AppendChild { parent, child }
            if *parent == head_key && *child == template_key
    )));
    assert!(!state.open_element_keys.contains(&head_key));

    let _ = builder
        .process(&Token::EndTag { name: template }, &ctx.atoms, &resolver)
        .unwrap();
    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::AfterHead
    );
}

#[test]
fn eof_unwinds_more_than_twelve_nested_templates() {
    let depth = 32usize;
    let input = format!("{}x", "<template>".repeat(depth));
    let patches = run_tree_builder_chunks(&[&input]);
    assert_eq!(
        patches
            .iter()
            .filter(|patch| matches!(patch, DomPatch::CreateTemplateContents { .. }))
            .count(),
        depth
    );
}

#[test]
fn deeply_nested_template_eof_does_not_retain_exact_state_per_template() {
    let depth = 256usize;
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();

    for _ in 0..depth {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: template,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap();
    }
    assert_eq!(builder.state_snapshot().template_modes.len(), depth);

    let _ = builder.process(&Token::Eof, &ctx.atoms, &resolver).unwrap();
    let state = builder.state_snapshot();
    assert!(state.template_modes.is_empty());
    assert!(
        builder.debug_perf_stats().max_same_token_cycle_states <= 3,
        "EOF recovery must not retain one cycle snapshot per open template"
    );
}

#[test]
fn template_eof_recovery_has_linear_aggregate_scan_bounds() {
    fn run(depth: usize, table_per_template: bool) {
        let mut ctx = DocumentParseContext::new();
        let resolver = EmptyResolver;
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
        let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
        let template = ctx.atoms.intern_ascii_folded("template").unwrap();
        let table = ctx.atoms.intern_ascii_folded("table").unwrap();

        for _ in 0..depth {
            let _ = builder
                .process(
                    &Token::StartTag {
                        name: template,
                        attrs: Vec::new(),
                        self_closing: false,
                    },
                    &ctx.atoms,
                    &resolver,
                )
                .unwrap();
            if table_per_template {
                let _ = builder
                    .process(
                        &Token::StartTag {
                            name: table,
                            attrs: Vec::new(),
                            self_closing: false,
                        },
                        &ctx.atoms,
                        &resolver,
                    )
                    .unwrap();
            }
        }
        let before = builder.debug_perf_stats();
        let _ = builder.process(&Token::Eof, &ctx.atoms, &resolver).unwrap();
        let after = builder.debug_perf_stats();

        let closes = after.template_close_ops - before.template_close_ops;
        let iterations =
            after.template_eof_unwind_iterations - before.template_eof_unwind_iterations;
        let pops = after.soe_pop_ops - before.soe_pop_ops;
        let reset_calls =
            after.reset_insertion_mode_scan_calls - before.reset_insertion_mode_scan_calls;
        let reset_steps =
            after.reset_insertion_mode_scan_steps - before.reset_insertion_mode_scan_steps;
        let owner_calls =
            after.template_recovery_owner_scan_calls - before.template_recovery_owner_scan_calls;
        let owner_steps =
            after.template_recovery_owner_scan_steps - before.template_recovery_owner_scan_steps;

        assert_eq!(closes, depth as u64);
        assert_eq!(iterations, depth as u64);
        assert_eq!(reset_calls, depth as u64);
        assert_eq!(owner_calls, depth as u64);
        assert_eq!(
            owner_steps, pops,
            "each owner-scan step removes that SOE entry"
        );
        let expected_max_pops = if table_per_template { 2 * depth } else { depth };
        assert!(pops <= expected_max_pops as u64);
        assert!(reset_steps <= depth as u64 + 1);
        assert_eq!(
            after.soe_scope_scan_calls, before.soe_scope_scan_calls,
            "EOF recovery uses exact template owners, not repeated scope scans"
        );
        assert_eq!(
            after.soe_scope_scan_steps, before.soe_scope_scan_steps,
            "EOF recovery adds no scope-scan work"
        );
        assert!(after.max_same_token_cycle_states <= 3);
    }

    run(16, false);
    run(256, false);
    run(16, true);
}

#[test]
fn template_eof_flushes_pending_table_text_before_depth_unwind() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    let table = ctx.atoms.intern_ascii_folded("table").unwrap();

    for name in [template, table] {
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
            .unwrap();
    }
    let _ = builder
        .process(
            &Token::Text {
                text: TextValue::Owned("pending".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();
    assert_eq!(builder.insertion_mode, InsertionMode::InTableText);
    assert!(builder.pending_table_text.is_some());

    let _ = builder.process(&Token::Eof, &ctx.atoms, &resolver).unwrap();
    assert!(builder.pending_table_text.is_none());
    assert!(builder.template_modes.is_empty());
    assert!(
        builder
            .drain_patches()
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { text, .. } if text == "pending"))
    );
}

#[test]
fn ordinary_tokens_use_constant_time_template_validation_after_closed_templates() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();

    for _ in 0..64 {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: template,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap();
        let _ = builder
            .process(&Token::EndTag { name: template }, &ctx.atoms, &resolver)
            .unwrap();
    }
    builder
        .validate_open_template_coordination_for_test()
        .unwrap();
    let before = builder.debug_perf_stats();

    for index in 0..128 {
        let _ = builder
            .process(
                &Token::Comment {
                    text: TextValue::Owned(format!("ordinary-{index}")),
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap();
    }
    let after = builder.debug_perf_stats();
    assert_eq!(
        after.template_validation_fast_path_tokens - before.template_validation_fast_path_tokens,
        128
    );
    assert_eq!(
        after.template_validation_transition_checks,
        before.template_validation_transition_checks
    );
    assert_eq!(
        after.template_full_audit_host_visits, before.template_full_audit_host_visits,
        "production token validation must not rescan historical closed templates"
    );
}

#[test]
fn no_template_tokens_take_the_parser_validation_fast_path() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let before = builder.debug_perf_stats();

    let _ = builder
        .process(
            &Token::Comment {
                text: TextValue::Owned("ordinary".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();
    let after = builder.debug_perf_stats();
    assert_eq!(
        after.template_validation_fast_path_tokens,
        before.template_validation_fast_path_tokens + 1
    );
    assert_eq!(
        after.template_validation_transition_checks,
        before.template_validation_transition_checks
    );
}

#[test]
fn nested_template_transitions_use_only_local_validation_checks() {
    let depth = 256usize;
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    let before = builder.debug_perf_stats();

    for _ in 0..depth {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: template,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap();
    }
    let after_starts = builder.debug_perf_stats();
    assert_eq!(
        after_starts.template_validation_transition_checks
            - before.template_validation_transition_checks,
        depth as u64
    );
    assert_eq!(
        after_starts.template_full_audit_host_visits, before.template_full_audit_host_visits,
        "nested starts must not invoke the heavy full-stack audit"
    );

    for _ in 0..depth {
        let _ = builder
            .process(&Token::EndTag { name: template }, &ctx.atoms, &resolver)
            .unwrap();
    }
    let after_closes = builder.debug_perf_stats();
    assert_eq!(
        after_closes.template_validation_transition_checks
            - after_starts.template_validation_transition_checks,
        depth as u64
    );
    assert_eq!(
        after_closes.template_full_audit_host_visits, before.template_full_audit_host_visits,
        "nested closes must not invoke the heavy full-stack audit"
    );
}

#[test]
fn template_acceptance_counter_overflow_is_atomic() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    let _ = builder.drain_patches();
    builder.set_template_validation_counters_for_test(7, u64::MAX);
    let state_before = builder.state_snapshot();
    let patch_count_before = builder.patches.len();

    assert!(
        builder
            .process(
                &Token::StartTag {
                    name: template,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .is_err()
    );
    assert_eq!(builder.state_snapshot(), state_before);
    assert_eq!(builder.patches.len(), patch_count_before);
    assert_eq!(
        builder.template_validation_counters_for_test(),
        (7, u64::MAX)
    );
}

#[test]
fn template_epoch_overflow_cannot_reuse_a_fast_path_identity() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    let _ = builder
        .process(
            &Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();
    let div = ctx.atoms.intern_ascii_folded("div").unwrap();
    let accepted = builder.template_validation_counters_for_test().1;
    builder.set_template_validation_counters_for_test(u64::MAX, accepted);
    let _ = builder.drain_patches();
    let state_before = builder.state_snapshot();
    let perf_before = builder.debug_perf_stats();

    assert!(
        builder
            .process(
                &Token::StartTag {
                    name: div,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .is_err()
    );
    assert_eq!(builder.state_snapshot(), state_before);
    assert!(builder.patches.is_empty());
    assert_eq!(
        builder.template_validation_counters_for_test(),
        (u64::MAX, accepted)
    );
    assert_eq!(
        builder
            .debug_perf_stats()
            .template_validation_fast_path_tokens,
        perf_before.template_validation_fast_path_tokens,
        "overflow must fail before token-boundary fast-path validation"
    );
}

#[test]
fn heavy_template_audit_detects_owner_marker_mode_and_association_corruption() {
    fn open_template_builder() -> (Html5TreeBuilder, DocumentParseContext) {
        let mut ctx = DocumentParseContext::new();
        let resolver = EmptyResolver;
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
        let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
        let template = ctx.atoms.intern_ascii_folded("template").unwrap();
        let _ = builder
            .process(
                &Token::StartTag {
                    name: template,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap();
        (builder, ctx)
    }

    let (mut owner, _) = open_template_builder();
    owner
        .template_modes
        .corrupt_current_owner_for_test(PatchKey(u32::MAX));
    assert!(owner.audit_html5_template_output_full().is_err());

    let (mut marker, _) = open_template_builder();
    marker
        .active_formatting
        .corrupt_last_template_marker_owner_for_test(Some(PatchKey(u32::MAX)));
    assert!(marker.audit_html5_template_output_full().is_err());

    let (mut mode, _) = open_template_builder();
    mode.insertion_mode = InsertionMode::Initial;
    assert!(mode.audit_html5_template_output_full().is_err());

    let (mut association, _) = open_template_builder();
    let host = association
        .template_modes
        .current()
        .expect("open template mode")
        .owner();
    association
        .live_tree
        .corrupt_template_association_for_test(host);
    assert!(association.audit_html5_template_output_full().is_err());
}

#[test]
fn unmatched_template_end_tag_is_ignored_without_state_corruption() {
    let patches = run_tree_builder_chunks(&["</template><p>ok"]);
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateTemplateContents { .. }))
    );
    assert!(
        materialized_dom_lines(&["</template><p>ok"])
            .iter()
            .any(|line| line.contains("local=\"p\""))
    );
}

#[test]
fn template_patch_keys_are_contiguous() {
    let patches = run_tree_builder_chunks(&["<template></template>"]);
    let (host, contents) = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateTemplateContents { host, contents } => Some((*host, *contents)),
            _ => None,
        })
        .unwrap();
    assert_eq!(contents, PatchKey(host.0 + 1));
}

#[test]
fn table_markup_inside_template_remains_beneath_the_contents_root() {
    let lines = materialized_dom_lines(&[
        "<template><table><tbody><tr><td>x</td></tr></tbody></table></template>",
    ]);
    let contents = lines
        .iter()
        .position(|line| line.trim() == "#template-contents")
        .expect("template contents boundary");
    let table = lines
        .iter()
        .position(|line| line.contains("local=\"table\""))
        .expect("table in template contents");
    assert!(table > contents);
    assert!(lines.iter().any(|line| line.contains("local=\"td\"")));
}

#[test]
fn select_family_subset_inside_template_stays_in_the_fragment() {
    let lines =
        materialized_dom_lines(&["<template><select><option>one<option>two</select></template>"]);
    let contents = lines
        .iter()
        .position(|line| line.trim() == "#template-contents")
        .expect("template contents boundary");
    let select = lines
        .iter()
        .position(|line| line.contains("local=\"select\""))
        .expect("select in template contents");
    assert!(select > contents);
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("local=\"option\""))
            .count(),
        2
    );
}

#[test]
fn template_foster_parenting_targets_the_contents_root_not_the_host() {
    let patches = run_tree_builder_chunks(&["<body><template><tr><div>x</div></tr></template>"]);
    let (host, contents) = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateTemplateContents { host, contents } => Some((*host, *contents)),
            _ => None,
        })
        .expect("template association");
    let div = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. } if name.is_html("div") => Some(*key),
            _ => None,
        })
        .expect("foster-parented div");
    assert!(
        patches.iter().any(|patch| match patch {
            DomPatch::AppendChild { parent, child }
            | DomPatch::InsertBefore {
                parent,
                child,
                before: _,
            } => *parent == contents && *child == div,
            _ => false,
        }),
        "fostered div did not target contents root: {patches:#?}"
    );
    assert!(!patches.iter().any(|patch| matches!(
        patch,
        DomPatch::AppendChild { parent, child }
            if *parent == host && *child == div
    )));
}

#[test]
fn form_pointer_remains_owned_by_the_outer_document_across_template_forms() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let form = ctx.atoms.intern_ascii_folded("form").unwrap();
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();

    let start = |name| Token::StartTag {
        name,
        attrs: Vec::new(),
        self_closing: false,
    };
    let _ = builder
        .process(&start(form), &ctx.atoms, &resolver)
        .unwrap();
    let outer_pointer = builder
        .state_snapshot()
        .form_element_pointer
        .expect("outer form pointer");
    let _ = builder
        .process(&start(template), &ctx.atoms, &resolver)
        .unwrap();
    let _ = builder
        .process(&start(form), &ctx.atoms, &resolver)
        .unwrap();
    assert_eq!(
        builder.state_snapshot().form_element_pointer,
        Some(outer_pointer),
        "a form inserted in template context must not replace the document form pointer"
    );

    let _ = builder
        .process(&Token::EndTag { name: form }, &ctx.atoms, &resolver)
        .unwrap();
    assert_eq!(
        builder.state_snapshot().form_element_pointer,
        Some(outer_pointer)
    );
    let _ = builder
        .process(&Token::EndTag { name: template }, &ctx.atoms, &resolver)
        .unwrap();
    assert_eq!(
        builder.state_snapshot().form_element_pointer,
        Some(outer_pointer)
    );
    let _ = builder
        .process(&Token::EndTag { name: form }, &ctx.atoms, &resolver)
        .unwrap();
    assert_eq!(builder.state_snapshot().form_element_pointer, None);
}

#[test]
fn pinned_table_cell_template_case_uses_existing_last_marker_recovery() {
    let whole = run_tree_builder_chunks(&["<table><thead><template><td></template></table>"]);
    let chunked =
        run_tree_builder_chunks(&["<table><thead><tem", "plate><td></tem", "plate></table>"]);
    assert_eq!(whole, chunked);
    assert_eq!(
        whole
            .iter()
            .filter(|patch| matches!(patch, DomPatch::CreateTemplateContents { .. }))
            .count(),
        1
    );
}

#[test]
fn pinned_table_cell_case_clears_only_the_last_afe_marker_at_each_algorithm_step() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let table = ctx.atoms.intern_ascii_folded("table").unwrap();
    let thead = ctx.atoms.intern_ascii_folded("thead").unwrap();
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    let td = ctx.atoms.intern_ascii_folded("td").unwrap();

    for name in [table, thead, template, td] {
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
            .unwrap();
    }
    let _ = builder
        .process(&Token::EndTag { name: template }, &ctx.atoms, &resolver)
        .unwrap();
    let state = builder.state_snapshot();
    let template_owner = state.template_modes[0].0;
    let cell_owner = state.open_element_keys.last().copied().unwrap();
    assert_eq!(state.template_modes.len(), 1);
    assert_eq!(
        state.active_formatting_entries,
        vec![
            AfeDiagnosticEntry::Marker(AfeMarker::new(
                AfeMarkerKind::Template,
                Some(template_owner),
            )),
            AfeDiagnosticEntry::Marker(AfeMarker::new(AfeMarkerKind::TableCell, Some(cell_owner),)),
        ]
    );

    let _ = builder
        .process(&Token::EndTag { name: table }, &ctx.atoms, &resolver)
        .unwrap();
    let state = builder.state_snapshot();
    assert_eq!(state.template_modes.len(), 1);
    assert_eq!(
        state.active_formatting_entries,
        vec![
            AfeDiagnosticEntry::Marker(AfeMarker::new(
                AfeMarkerKind::Template,
                Some(template_owner),
            )),
            AfeDiagnosticEntry::Marker(AfeMarker::new(AfeMarkerKind::TableCell, Some(cell_owner),)),
        ],
        "the table end tag is scope-blocked while the template context remains open"
    );

    let _ = builder.process(&Token::Eof, &ctx.atoms, &resolver).unwrap();
    let state = builder.state_snapshot();
    assert!(state.template_modes.is_empty());
    assert_eq!(
        state.active_formatting_entries,
        vec![AfeDiagnosticEntry::Marker(AfeMarker::new(
            AfeMarkerKind::Template,
            Some(template_owner),
        ))],
        "template EOF recovery must call clear_to_last_marker exactly once, leaving the older template marker as diagnostic state"
    );
}

#[test]
fn two_node_resource_rejection_is_atomic_across_all_template_parser_state() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    builder.config.limits.max_nodes_created = builder.non_document_nodes_created + 1;
    let _ = builder.drain_patches();
    let _ = builder.take_parse_error_kinds_for_test();
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    let p = ctx.atoms.intern_ascii_folded("p").unwrap();

    let state_before = builder.state_snapshot();
    let next_key_before = builder.next_patch_key;
    let node_count_before = builder.non_document_nodes_created;
    let frameset_before = builder.document_state.frameset_ok;
    let text_state_before = builder.last_text_patch.clone();
    let _ = builder
        .process(
            &Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();

    assert!(builder.drain_patches().is_empty());
    assert_eq!(builder.state_snapshot(), state_before);
    assert_eq!(builder.next_patch_key, next_key_before);
    assert_eq!(builder.non_document_nodes_created, node_count_before);
    assert_eq!(builder.document_state.frameset_ok, frameset_before);
    assert_eq!(
        builder.last_text_patch.as_ref().map(|last| last.text_key),
        text_state_before.as_ref().map(|last| last.text_key)
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["resource-limit-node-count"]
    );

    let _ = builder
        .process(
            &Token::StartTag {
                name: p,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();
    assert!(builder.drain_patches().iter().any(|patch| matches!(
        patch,
        DomPatch::CreateElement { key, name, .. }
            if *key == PatchKey(next_key_before.get()) && name.is_html("p")
    )));
}

#[test]
fn template_start_depth_and_parent_capacity_rejections_are_atomic() {
    for limit_kind in ["soe", "children"] {
        let mut ctx = DocumentParseContext::new();
        let resolver = EmptyResolver;
        let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
        let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
        let _ = builder.drain_patches();
        let template = ctx.atoms.intern_ascii_folded("template").unwrap();
        match limit_kind {
            "soe" => {
                builder.config.limits.max_open_elements_depth = builder.open_elements.len();
            }
            "children" => builder.config.limits.max_children_per_node = 0,
            _ => unreachable!(),
        }
        let state_before = builder.state_snapshot();
        let key_before = builder.next_patch_key;
        let count_before = builder.non_document_nodes_created;
        let text_before = builder.last_text_patch.clone();

        let _ = builder
            .process(
                &Token::StartTag {
                    name: template,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap();

        assert!(builder.drain_patches().is_empty(), "{limit_kind}");
        assert_eq!(builder.state_snapshot(), state_before, "{limit_kind}");
        assert_eq!(builder.next_patch_key, key_before, "{limit_kind}");
        assert_eq!(
            builder.non_document_nodes_created, count_before,
            "{limit_kind}"
        );
        assert_eq!(
            builder.last_text_patch.as_ref().map(|last| last.text_key),
            text_before.as_ref().map(|last| last.text_key),
            "{limit_kind}"
        );
    }
}

#[test]
fn template_parent_child_reservation_failure_is_atomic() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let _ = builder.drain_patches();
    let _ = builder.take_parse_error_kinds_for_test();
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();

    let parser_state_before = builder.state_snapshot();
    let dom_state_before = builder.dom_invariant_state();
    let key_before = builder.next_patch_key;
    let node_count_before = builder.non_document_nodes_created;
    let frameset_before = builder.document_state.frameset_ok;
    let text_state_before = builder.last_text_patch.clone();
    builder.live_tree.fail_next_child_reservation_for_test(
        crate::html5::tree_builder::live_tree::ChildInsertionReservationError::AllocationFailure,
    );

    let _ = builder
        .process(
            &Token::StartTag {
                name: template,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .unwrap();

    assert!(builder.drain_patches().is_empty());
    assert_eq!(builder.next_patch_key, key_before);
    assert_eq!(builder.non_document_nodes_created, node_count_before);
    assert_eq!(builder.dom_invariant_state(), dom_state_before);
    assert_eq!(builder.state_snapshot(), parser_state_before);
    assert_eq!(builder.document_state.frameset_ok, frameset_before);
    assert_eq!(
        builder.last_text_patch.as_ref().map(|last| last.text_key),
        text_state_before.as_ref().map(|last| last.text_key)
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["resource-limit-template-parent-child-reservation"]
    );
}

#[test]
fn two_key_overflow_rejects_template_start_without_partial_commit() {
    let mut ctx = DocumentParseContext::new();
    let resolver = EmptyResolver;
    let mut builder = Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).unwrap();
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let _ = builder.drain_patches();
    let template = ctx.atoms.intern_ascii_folded("template").unwrap();
    builder.next_patch_key = NonZeroU32::new(u32::MAX).unwrap();
    let state_before = builder.state_snapshot();
    let count_before = builder.non_document_nodes_created;

    assert!(
        builder
            .process(
                &Token::StartTag {
                    name: template,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .is_err()
    );
    assert!(builder.drain_patches().is_empty());
    assert_eq!(builder.state_snapshot(), state_before);
    assert_eq!(builder.next_patch_key.get(), u32::MAX);
    assert_eq!(builder.non_document_nodes_created, count_before);
}
