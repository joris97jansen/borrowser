use super::helpers::{EmptyResolver, enter_in_body};
use crate::html5::tree_builder::modes::InsertionMode;

#[test]
fn table_state_snapshot_tracks_current_table_and_pending_character_buffer() {
    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");
    let tbody = ctx
        .atoms
        .intern_ascii_folded("tbody")
        .expect("atom interning");
    let tr = ctx.atoms.intern_ascii_folded("tr").expect("atom interning");
    let td = ctx.atoms.intern_ascii_folded("td").expect("atom interning");

    let outer_table_key = builder
        .insert_element(table, &[], false, &ctx.atoms, &resolver)
        .expect("outer table insertion")
        .expect("outer table insertion should not hit resource limits");
    let _ = builder
        .insert_element(tbody, &[], false, &ctx.atoms, &resolver)
        .expect("tbody insertion")
        .expect("tbody insertion should not hit resource limits");
    let _ = builder
        .insert_element(tr, &[], false, &ctx.atoms, &resolver)
        .expect("tr insertion")
        .expect("tr insertion should not hit resource limits");
    let _ = builder
        .insert_element(td, &[], false, &ctx.atoms, &resolver)
        .expect("td insertion")
        .expect("td insertion should not hit resource limits");
    let inner_table_key = builder
        .insert_element(table, &[], false, &ctx.atoms, &resolver)
        .expect("inner table insertion")
        .expect("inner table insertion should not hit resource limits");

    builder
        .enter_in_table_text_mode(InsertionMode::InRow)
        .expect("test should enter table text mode");
    builder
        .buffer_pending_table_character_tokens(" \t")
        .expect("active table text state should accept whitespace");
    builder
        .buffer_pending_table_character_tokens("x")
        .expect("active table text state should accept text");

    let state = builder.state_snapshot();
    assert_eq!(
        state.current_table_key,
        Some(inner_table_key),
        "current table must resolve to the most recent open <table> on SOE"
    );
    assert_eq!(
        state.pending_table_character_tokens,
        vec![" \t".to_string(), "x".to_string()],
        "pending table character chunks must preserve source order"
    );
    assert!(
        state.pending_table_character_tokens_contains_non_space,
        "pending table character buffer should track non-space content"
    );
    assert_ne!(
        state.current_table_key,
        Some(outer_table_key),
        "nested tables must not leave the outer table as the current table"
    );

    let drained = builder
        .take_pending_table_text_state()
        .expect("active table text state should drain");
    assert_eq!(drained.original_insertion_mode(), InsertionMode::InRow);
    assert_eq!(drained.tokens().chunks(), [" \t", "x"]);
    assert!(drained.tokens().contains_non_space());
    assert!(
        builder
            .state_snapshot()
            .pending_table_character_tokens
            .is_empty()
    );
}

#[test]
fn in_table_text_without_pending_state_is_internal_invariant_error() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    builder.insertion_mode = InsertionMode::InTableText;
    builder.pending_table_text = None;

    let result = builder.process(&Token::Eof, &ctx.atoms, &resolver);

    assert!(
        result.is_err(),
        "an impossible InTableText state without pending table-text state must be an internal invariant error"
    );
    assert!(
        builder.take_parse_error_kinds_for_test().is_empty(),
        "internal table-text state corruption must not be reported as malformed HTML"
    );
}

#[test]
fn entering_in_table_text_twice_is_non_destructive_internal_invariant_error() {
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    builder
        .enter_in_table_text_mode(InsertionMode::InTable)
        .expect("first table-text entry should succeed");
    builder
        .buffer_pending_table_character_tokens("x")
        .expect("active table-text state should accept text");

    let before = builder.state_snapshot();
    let result = builder.enter_in_table_text_mode(InsertionMode::InRow);
    let after = builder.state_snapshot();

    assert!(
        result.is_err(),
        "entering table-text mode while pending table-text state is active must be an internal invariant error"
    );
    assert!(
        builder.take_parse_error_kinds_for_test().is_empty(),
        "internal table-text lifecycle violations must not be reported as malformed HTML"
    );
    assert_eq!(
        after, before,
        "failed table-text re-entry must not mutate the active pending state"
    );
    assert_eq!(after.insertion_mode, InsertionMode::InTableText);
    assert_eq!(
        after.table_text_original_insertion_mode,
        Some(InsertionMode::InTable)
    );
    assert_eq!(after.pending_table_character_tokens, vec!["x".to_string()]);
    assert!(
        after.pending_table_character_tokens_contains_non_space,
        "failed re-entry must preserve non-space classification"
    );
}

#[test]
fn clear_stack_to_table_context_pops_back_to_table_boundary() {
    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    for tag in ["table", "tbody", "tr", "td", "div"] {
        let atom = ctx.atoms.intern_ascii_folded(tag).expect("atom interning");
        let _ = builder
            .insert_element(atom, &[], false, &ctx.atoms, &resolver)
            .unwrap_or_else(|_| panic!("{tag} insertion should succeed"))
            .unwrap_or_else(|| panic!("{tag} insertion should not hit resource limits"));
    }

    let removed = builder.clear_stack_to_table_context();
    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");
    let state = builder.state_snapshot();

    assert_eq!(
        removed, 4,
        "tbody/tr/td/div should be popped to table context"
    );
    assert_eq!(
        state.open_element_names.last().copied(),
        Some(table),
        "table context clearing must leave <table> as the current node"
    );
}

#[test]
fn table_scope_checks_follow_table_boundaries() {
    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    let p = ctx.atoms.intern_ascii_folded("p").expect("atom interning");
    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");

    let _ = builder
        .insert_element(p, &[], false, &ctx.atoms, &resolver)
        .expect("p insertion")
        .expect("p insertion should not hit resource limits");
    assert!(
        builder.has_in_table_scope(p),
        "before a table boundary, the open <p> should still be visible in table scope"
    );

    let _ = builder
        .insert_element(table, &[], false, &ctx.atoms, &resolver)
        .expect("table insertion")
        .expect("table insertion should not hit resource limits");
    assert!(
        builder.has_in_table_scope(table),
        "the open <table> should be visible in table scope"
    );
    assert!(
        !builder.has_in_table_scope(p),
        "a table boundary must hide ancestors above the current table from table scope"
    );
}

#[test]
fn close_cell_pops_to_cell_boundary_clears_afe_and_switches_to_in_row() {
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    for tag in ["table", "tbody", "tr", "td"] {
        let atom = ctx.atoms.intern_ascii_folded(tag).expect("atom interning");
        let _ = builder
            .insert_element(atom, &[], false, &ctx.atoms, &resolver)
            .unwrap_or_else(|_| panic!("{tag} insertion should succeed"))
            .unwrap_or_else(|| panic!("{tag} insertion should not hit resource limits"));
    }

    builder.active_formatting.push_marker();

    let b = ctx.atoms.intern_ascii_folded("b").expect("atom interning");
    let b_key = builder
        .insert_element(b, &[], false, &ctx.atoms, &resolver)
        .expect("b insertion")
        .expect("b insertion should not hit resource limits");
    builder
        .push_active_formatting_element(b_key, b, &[], &resolver)
        .expect("AFE push for <b>");

    builder.insertion_mode = InsertionMode::InCell;
    assert!(
        !builder.active_formatting.entries().is_empty(),
        "test setup must leave an AFE marker + element to clear"
    );
    let before_scope_scan_calls = builder.open_elements.scope_scan_calls();
    let before_scope_scan_steps = builder.open_elements.scope_scan_steps();

    assert!(
        builder.close_cell(),
        "close_cell() should close the open cell"
    );

    let tr = ctx.atoms.intern_ascii_folded("tr").expect("atom interning");
    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InRow);
    assert_eq!(
        state.open_element_names.last().copied(),
        Some(tr),
        "closing the cell must leave the row as the current node"
    );
    assert!(
        builder.active_formatting.entries().is_empty(),
        "close_cell() must clear active formatting back to the last marker"
    );
    let after_scope_scan_calls = builder.open_elements.scope_scan_calls();
    let after_scope_scan_steps = builder.open_elements.scope_scan_steps();
    assert!(
        after_scope_scan_calls > before_scope_scan_calls,
        "close_cell() should account for the table-cell lookup as an SOE scan"
    );
    assert!(
        after_scope_scan_steps > before_scope_scan_steps,
        "close_cell() should account for the table-cell lookup in SOE scan steps"
    );
    assert!(
        after_scope_scan_steps >= after_scope_scan_calls,
        "scope-scan steps should grow monotonically with scan calls"
    );
}
