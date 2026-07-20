use super::helpers::{materialized_dom_lines, run_tree_builder_chunks};
use crate::dom_patch::DomPatch;
use crate::dom_patch::PatchKey;

fn first_created_element_key(patches: &[DomPatch], expected_name: &str) -> PatchKey {
    patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. } if name.is_html(expected_name) => Some(*key),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected CreateElement patch for <{expected_name}>"))
}

#[test]
fn in_table_body_text_flush_restores_table_body_mode_before_reprocessing_tr() {
    use super::helpers::{EmptyResolver, enter_in_body};
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

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

    for token in [
        Token::StartTag {
            name: table,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: tbody,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("x".to_string()),
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("table body text setup should remain recoverable");
    }

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTableText);
    assert_eq!(
        state.table_text_original_insertion_mode,
        Some(InsertionMode::InTableBody)
    );
    assert_eq!(state.pending_table_character_tokens, vec!["x".to_string()]);

    let _ = builder
        .process(
            &Token::StartTag {
                name: tr,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("tr reprocessing after table text should remain recoverable");

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InRow);
    assert_eq!(state.open_element_names.last().copied(), Some(tr));
    assert_eq!(state.table_text_original_insertion_mode, None);
    assert!(state.pending_table_character_tokens.is_empty());
    assert!(
        builder
            .take_parse_error_kinds_for_test()
            .contains(&"in-table-text-non-space-foster-parented"),
        "non-space table-body text must record deterministic foster-parenting recovery"
    );
}

#[test]
fn in_row_text_flush_restores_row_mode_before_reprocessing_cell() {
    use super::helpers::{EmptyResolver, enter_in_body};
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    for tag in ["table", "tbody", "tr"] {
        let atom = ctx.atoms.intern_ascii_folded(tag).expect("atom interning");
        let _ = builder
            .process(
                &Token::StartTag {
                    name: atom,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap_or_else(|_| panic!("{tag} start tag should remain recoverable"));
    }

    let _ = builder
        .process(
            &Token::Text {
                text: TextValue::Owned("x".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("row text should enter table text mode");
    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTableText);
    assert_eq!(
        state.table_text_original_insertion_mode,
        Some(InsertionMode::InRow)
    );

    let td = ctx.atoms.intern_ascii_folded("td").expect("atom interning");
    let _ = builder
        .process(
            &Token::StartTag {
                name: td,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("td reprocessing after row text should remain recoverable");

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InCell);
    assert_eq!(state.open_element_names.last().copied(), Some(td));
    assert_eq!(state.table_text_original_insertion_mode, None);
    assert!(state.pending_table_character_tokens.is_empty());
}

#[test]
fn eof_flushes_pending_table_text_and_clears_return_mode() {
    use super::helpers::{EmptyResolver, enter_in_body};
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

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

    for token in [
        Token::StartTag {
            name: table,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("x".to_string()),
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("table text EOF setup should remain recoverable");
    }

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTableText);
    assert_eq!(
        state.table_text_original_insertion_mode,
        Some(InsertionMode::InTable)
    );
    assert_eq!(state.pending_table_character_tokens, vec!["x".to_string()]);
    let table_key = state
        .current_table_key
        .expect("table should remain open before EOF flush");
    let _ = builder.drain_patches();

    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("EOF must flush pending table text");

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTable);
    assert_eq!(state.table_text_original_insertion_mode, None);
    assert!(state.pending_table_character_tokens.is_empty());
    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { text, .. } if text == "x")),
        "EOF flush must create the pending table text"
    );
    assert!(
        patches.iter().any(|patch| matches!(
            patch,
            DomPatch::InsertBefore {
                parent: _,
                child: _,
                before
            } if *before == table_key
        )),
        "EOF-flushed non-space table text must be inserted before the live table"
    );
}

#[test]
fn in_table_text_non_space_is_foster_parented_before_table() {
    let dom = materialized_dom_lines(&["<!doctype html><table>a</table>"]);

    assert_eq!(
        dom,
        vec![
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  <!doctype html>".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      \"a\"".to_string(),
            "      element ns=html local=\"table\" attrs=[]".to_string(),
        ]
    );
}

#[test]
fn in_table_text_whitespace_stays_inside_table_and_is_chunk_invariant() {
    let whole = materialized_dom_lines(&["<!doctype html><table> \n\t</table>"]);
    let chunked = materialized_dom_lines(&["<!doctype html><table>", " \n", "\t</table>"]);

    let expected = vec![
        "#dom-snapshot-v2".to_string(),
        "#document".to_string(),
        "  <!doctype html>".to_string(),
        "  element ns=html local=\"html\" attrs=[]".to_string(),
        "    element ns=html local=\"head\" attrs=[]".to_string(),
        "    element ns=html local=\"body\" attrs=[]".to_string(),
        "      element ns=html local=\"table\" attrs=[]".to_string(),
        "        \" \\n\\t\"".to_string(),
    ];

    assert_eq!(whole, expected);
    assert_eq!(
        chunked, whole,
        "chunk boundaries must not change the table-text whitespace result"
    );
}

#[test]
fn in_table_anything_else_uses_canonical_insert_before_without_remove_node() {
    let patches = run_tree_builder_chunks(&["<!doctype html><table><div>x</div></table>"]);
    let table_key = first_created_element_key(&patches, "table");

    assert!(
        patches.iter().any(|patch| {
            matches!(
                patch,
                DomPatch::InsertBefore {
                    parent: _,
                    child: _,
                    before,
                } if *before == table_key
            )
        }),
        "misplaced table content should be foster-parented with InsertBefore relative to the live <table>"
    );
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "foster-parented insertion must not use RemoveNode detaches"
    );
}

#[test]
fn in_body_table_start_tag_enters_in_table_mode() {
    use super::helpers::{EmptyResolver, enter_in_body};
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

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
        .expect("table start tag should remain recoverable");

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTable);
    assert_eq!(state.open_element_names.last().copied(), Some(table));
}

#[test]
fn in_table_tbody_start_tag_switches_to_in_table_body() {
    use super::helpers::{EmptyResolver, enter_in_body};
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

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
        .expect("table start tag should remain recoverable");
    let _ = builder
        .process(
            &Token::StartTag {
                name: tbody,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("tbody start tag should remain recoverable");

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTableBody);
    assert_eq!(state.open_element_names.last().copied(), Some(tbody));
}
