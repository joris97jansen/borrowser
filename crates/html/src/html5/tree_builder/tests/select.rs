use super::helpers::{
    EmptyResolver, enter_in_body, materialized_dom_lines, run_tree_builder_chunks,
};
use crate::dom_patch::DomPatch;
use crate::html5::shared::{DocumentParseContext, Token};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

#[derive(Debug, PartialEq, Eq)]
struct SelectRun {
    patches: Vec<DomPatch>,
    errors: Vec<&'static str>,
    state: crate::html5::tree_builder::api::TreeBuilderStateSnapshot,
    witness: crate::html5::tree_builder::TreeBuilderProgressWitness,
    dom: Vec<String>,
}

fn in_body_builder() -> (Html5TreeBuilder, DocumentParseContext, EmptyResolver) {
    let mut ctx = DocumentParseContext::new();
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
    ctx: &DocumentParseContext,
    resolver: &EmptyResolver,
) {
    let _ = builder
        .process(&token, &ctx.atoms, resolver)
        .expect("select-family token should remain recoverable");
}

fn start(name: crate::html5::shared::AtomId) -> Token {
    Token::StartTag {
        name,
        attrs: Vec::new(),
        self_closing: false,
    }
}

fn run_select_chunks(chunks: &[&str]) -> SelectRun {
    use crate::html5::shared::Input;
    use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).expect("builder init");
    let mut input = Input::new();

    for chunk in chunks {
        input.push_str(chunk);
        loop {
            let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                assert!(matches!(
                    result,
                    TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                ));
                break;
            }
            let resolver = batch.resolver();
            for token in batch.iter() {
                let step = builder
                    .process(token, &ctx.atoms, &resolver)
                    .expect("select parity input must remain recoverable");
                if let Some(control) = step.tokenizer_control {
                    tokenizer.apply_control(control);
                }
            }
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            let step = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("select parity EOF drain must remain recoverable");
            if let Some(control) = step.tokenizer_control {
                tokenizer.apply_control(control);
            }
        }
    }

    let state = builder.state_snapshot();
    let witness = builder.progress_witness();
    let errors = builder.take_parse_error_kinds_for_test();
    let patches = builder.drain_patches();
    let dom = crate::test_harness::materialize_patch_batches(std::slice::from_ref(&patches))
        .map(|dom| crate::html5::tree_builder::serialize_dom_for_test(&dom))
        .expect("select parity patches must materialize");
    SelectRun {
        patches,
        errors,
        state,
        witness,
        dom,
    }
}

#[test]
fn consecutive_option_and_optgroup_starts_use_shared_implied_end_tags() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    let optgroup = ctx.atoms.intern_ascii_folded("optgroup").expect("optgroup");

    for token in [start(select), start(option), start(option)] {
        process(&mut builder, token, &ctx, &resolver);
    }
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![
            builder.known_tags.html,
            builder.known_tags.body,
            select,
            option,
        ]
    );

    process(&mut builder, start(optgroup), &ctx, &resolver);
    process(&mut builder, start(option), &ctx, &resolver);
    process(&mut builder, start(optgroup), &ctx, &resolver);
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![
            builder.known_tags.html,
            builder.known_tags.body,
            select,
            optgroup,
        ]
    );
    assert!(
        builder.take_parse_error_kinds_for_test().is_empty(),
        "ordinary implied select-family closure is not itself a parse error"
    );
}

#[test]
fn option_start_preserves_an_open_optgroup_exception_target() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    let optgroup = ctx.atoms.intern_ascii_folded("optgroup").expect("optgroup");

    for token in [start(select), start(optgroup), start(option), start(option)] {
        process(&mut builder, token, &ctx, &resolver);
    }
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![
            builder.known_tags.html,
            builder.known_tags.body,
            select,
            optgroup,
            option,
        ]
    );
}

#[test]
fn select_family_start_recovery_does_not_pop_through_an_intervening_element() {
    for next_name in ["option", "optgroup"] {
        let (mut builder, mut ctx, resolver) = in_body_builder();
        let select = ctx.atoms.intern_ascii_folded("select").expect("select");
        let option = ctx.atoms.intern_ascii_folded("option").expect("option");
        let div = ctx.atoms.intern_ascii_folded("div").expect("div");
        let next = ctx.atoms.intern_ascii_folded(next_name).expect("next tag");
        for token in [start(select), start(option), start(div), start(next)] {
            process(&mut builder, token, &ctx, &resolver);
        }
        assert_eq!(
            builder.state_snapshot().open_element_names,
            vec![
                builder.known_tags.html,
                builder.known_tags.body,
                select,
                option,
                div,
                next,
            ],
            "next={next_name}"
        );
        assert_eq!(
            builder.take_parse_error_kinds_for_test(),
            vec![if next_name == "option" {
                "in-body-option-start-tag-open-option-remains"
            } else {
                "in-body-optgroup-start-tag-open-select-family-remains"
            }]
        );
    }
}

#[test]
fn nested_select_recovery_closes_existing_select_and_inserts_no_replacement() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    process(&mut builder, start(select), &ctx, &resolver);
    process(&mut builder, start(select), &ctx, &resolver);

    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body]
    );
    let patches = builder.drain_patches();
    assert_eq!(
        patches
            .iter()
            .filter(|patch| matches!(patch, DomPatch::CreateElement { name, .. } if name.as_ref() == "select"))
            .count(),
        1
    );
    assert!(
        patches
            .iter()
            .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. }))
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["in-body-select-start-tag-with-select-in-scope"]
    );
}

#[test]
fn dedicated_select_end_closes_intervening_stack_entries_without_dom_removal() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let div = ctx.atoms.intern_ascii_folded("div").expect("div");
    for token in [start(select), start(div), Token::EndTag { name: select }] {
        process(&mut builder, token, &ctx, &resolver);
    }

    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body]
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["in-body-select-end-tag-implied-close-mismatch"]
    );
    assert!(
        builder
            .drain_patches()
            .iter()
            .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. }))
    );

    process(
        &mut builder,
        Token::EndTag { name: select },
        &ctx,
        &resolver,
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["in-body-select-end-tag-not-in-scope"]
    );
}

#[test]
fn generic_option_and_optgroup_ends_stop_at_special_barriers() {
    for target_name in ["option", "optgroup"] {
        for barrier_name in ["div", "object", "noscript"] {
            let (mut builder, mut ctx, resolver) = in_body_builder();
            let select = ctx.atoms.intern_ascii_folded("select").expect("select");
            let target = ctx.atoms.intern_ascii_folded(target_name).expect("target");
            let barrier = ctx
                .atoms
                .intern_ascii_folded(barrier_name)
                .expect("barrier");
            for token in [start(select), start(target), start(barrier)] {
                process(&mut builder, token, &ctx, &resolver);
            }
            let before = builder.state_snapshot().open_element_keys;
            process(
                &mut builder,
                Token::EndTag { name: target },
                &ctx,
                &resolver,
            );
            assert_eq!(
                builder.state_snapshot().open_element_keys,
                before,
                "barrier={barrier_name}, target={target_name}"
            );
            assert_eq!(
                builder.take_parse_error_kinds_for_test(),
                vec!["in-body-any-other-end-tag-blocked-by-special"],
                "barrier={barrier_name}, target={target_name}"
            );
        }
    }
}

#[test]
fn generic_option_and_optgroup_ends_close_matches_and_ignore_unmatched_tokens() {
    for target_name in ["option", "optgroup"] {
        let (mut builder, mut ctx, resolver) = in_body_builder();
        let select = ctx.atoms.intern_ascii_folded("select").expect("select");
        let target = ctx.atoms.intern_ascii_folded(target_name).expect("target");
        for token in [start(select), start(target), Token::EndTag { name: target }] {
            process(&mut builder, token, &ctx, &resolver);
        }
        assert_eq!(
            builder.state_snapshot().open_element_names,
            vec![builder.known_tags.html, builder.known_tags.body, select]
        );
        assert!(builder.take_parse_error_kinds_for_test().is_empty());
        let before = builder.state_snapshot().open_element_keys;
        process(
            &mut builder,
            Token::EndTag { name: target },
            &ctx,
            &resolver,
        );
        assert_eq!(builder.state_snapshot().open_element_keys, before);
        assert_eq!(
            builder.take_parse_error_kinds_for_test(),
            vec!["in-body-any-other-end-tag-blocked-by-special"]
        );
    }
}

#[test]
fn generic_option_end_can_cross_an_ordinary_non_special_element() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    let span = ctx.atoms.intern_ascii_folded("span").expect("span");
    for token in [start(select), start(option), start(span)] {
        process(&mut builder, token, &ctx, &resolver);
    }
    process(
        &mut builder,
        Token::EndTag { name: option },
        &ctx,
        &resolver,
    );
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, select,]
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["in-body-end-tag-implied-close-mismatch"]
    );
}

#[test]
fn generic_option_end_cannot_reach_through_a_select_boundary() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    for token in [start(option), start(select)] {
        process(&mut builder, token, &ctx, &resolver);
    }
    let before = builder.state_snapshot().open_element_keys;
    process(
        &mut builder,
        Token::EndTag { name: option },
        &ctx,
        &resolver,
    );
    assert_eq!(builder.state_snapshot().open_element_keys, before);
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["in-body-any-other-end-tag-blocked-by-special"]
    );
}

#[test]
fn input_closes_select_before_reconstruction_and_void_insertion() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    let input = ctx.atoms.intern_ascii_folded("input").expect("input");
    for token in [start(select), start(option), start(input)] {
        process(&mut builder, token, &ctx, &resolver);
    }
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body]
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["in-body-input-start-tag-closes-select"]
    );
    let patches = builder.drain_patches();
    assert!(patches.iter().any(
        |patch| matches!(patch, DomPatch::CreateElement { name, .. } if name.as_ref() == "input")
    ));
    assert!(
        patches
            .iter()
            .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. }))
    );
}

#[test]
fn hr_inside_select_closes_supported_implied_option_and_remains_void() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    let hr = ctx.atoms.intern_ascii_folded("hr").expect("hr");
    for token in [start(select), start(option), start(hr)] {
        process(&mut builder, token, &ctx, &resolver);
    }
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, select,]
    );
    assert!(!builder.state_snapshot().frameset_ok);
    assert!(builder.take_parse_error_kinds_for_test().is_empty());
}

#[test]
fn hr_reports_select_family_entry_left_below_an_intervening_element() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    let div = ctx.atoms.intern_ascii_folded("div").expect("div");
    let hr = ctx.atoms.intern_ascii_folded("hr").expect("hr");
    for token in [start(select), start(option), start(div), start(hr)] {
        process(&mut builder, token, &ctx, &resolver);
    }
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![
            builder.known_tags.html,
            builder.known_tags.body,
            select,
            option,
            div,
        ]
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["in-body-hr-start-tag-open-select-family-remains"]
    );
}

#[test]
fn dispatch_finalizes_each_select_family_self_closing_flag_once() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let option = ctx.atoms.intern_ascii_folded("option").expect("option");
    let optgroup = ctx.atoms.intern_ascii_folded("optgroup").expect("optgroup");
    for name in [select, option, optgroup] {
        process(
            &mut builder,
            Token::StartTag {
                name,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx,
            &resolver,
        );
    }
    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(
        errors
            .iter()
            .filter(|kind| **kind == "non-void-html-element-start-tag-with-trailing-solidus")
            .count(),
        3
    );
}

#[test]
fn dispatch_finalizes_ignored_nested_select_and_void_input_hr_exactly_once() {
    let (mut builder, mut ctx, resolver) = in_body_builder();
    let select = ctx.atoms.intern_ascii_folded("select").expect("select");
    let input = ctx.atoms.intern_ascii_folded("input").expect("input");
    let hr = ctx.atoms.intern_ascii_folded("hr").expect("hr");
    process(&mut builder, start(select), &ctx, &resolver);
    for name in [select, input, hr] {
        process(
            &mut builder,
            Token::StartTag {
                name,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx,
            &resolver,
        );
    }
    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(
        errors,
        vec![
            "in-body-select-start-tag-with-select-in-scope",
            "non-void-html-element-start-tag-with-trailing-solidus",
        ],
        "the ignored nested select finalizes once after its recovery error; void input/hr remain acknowledged"
    );
}

#[test]
fn fostered_select_keeps_later_option_as_its_child() {
    let input = "<table><select><option>3</select></table>";
    assert_eq!(
        materialized_dom_lines(&[input]),
        vec![
            "#document",
            "  <html>",
            "    <head>",
            "    <body>",
            "      <select>",
            "        <option>",
            "          \"3\"",
            "      <table>",
        ]
    );
    let patches = run_tree_builder_chunks(&[input]);
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::InsertBefore { .. })),
        "the fostered select must use its final InsertBefore location"
    );
    assert!(
        patches
            .iter()
            .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. })),
        "stack-only recovery must not repair the materialized DOM"
    );
}

#[test]
fn table_token_while_select_is_open_uses_existing_stack_clearing_and_reprocessing() {
    let input = "<table><select><table></table></select></table>";
    assert_eq!(
        materialized_dom_lines(&[input]),
        vec![
            "#document",
            "  <html>",
            "    <head>",
            "    <body>",
            "      <select>",
            "      <table>",
            "      <table>",
        ]
    );
    let patches = run_tree_builder_chunks(&[input]);
    assert!(
        patches
            .iter()
            .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. }))
    );
}

#[test]
fn row_recovery_while_option_and_select_are_open_preserves_upstream_tree() {
    let input = "<table><select><option>A<tr><td>B</td></tr></table>";
    let expected = vec![
        "#document",
        "  <html>",
        "    <head>",
        "    <body>",
        "      <select>",
        "        <option>",
        "          \"A\"",
        "      <table>",
        "        <tbody>",
        "          <tr>",
        "            <td>",
        "              \"B\"",
    ];
    assert_eq!(materialized_dom_lines(&[input]), expected);
    assert_eq!(
        materialized_dom_lines(&["<table><select><option>A", "<tr><td>B</td></tr></table>"]),
        expected,
        "table/select recovery must be chunk invariant"
    );
}

#[test]
fn input_table_paths_keep_hidden_direct_in_table_distinct_from_in_body_recovery() {
    assert_eq!(
        materialized_dom_lines(&["<table><select><input type=hidden>X</select></table>"]),
        vec![
            "#document",
            "  <html>",
            "    <head>",
            "    <body>",
            "      <select>",
            "        <input type=\"hidden\">",
            "        \"X\"",
            "      <table>",
        ],
        "the dedicated direct-InTable hidden input branch must not use select-aware InBody closure"
    );

    let non_hidden = run_tree_builder_chunks(&["<table><select><input>X</table>"]);
    let dom = crate::test_harness::materialize_patch_batches(std::slice::from_ref(&non_hidden))
        .map(|dom| crate::html5::tree_builder::serialize_dom_for_test(&dom))
        .expect("materialize direct table non-hidden input");
    assert_eq!(
        dom,
        vec![
            "#document",
            "  <html>",
            "    <head>",
            "    <body>",
            "      <select>",
            "      <input>",
            "      \"X\"",
            "      <table>",
        ]
    );
    assert!(
        non_hidden
            .iter()
            .filter(|patch| matches!(patch, DomPatch::InsertBefore { .. }))
            .count()
            >= 2,
        "fostered select and non-hidden input must each choose InsertBefore before emission"
    );
}

#[test]
fn select_aware_input_inside_table_cell_uses_ordinary_in_body_parenting() {
    assert_eq!(
        materialized_dom_lines(&["<table><tr><td><select><option>A<input>B</td></tr></table>",]),
        vec![
            "#document",
            "  <html>",
            "    <head>",
            "    <body>",
            "      <table>",
            "        <tbody>",
            "          <tr>",
            "            <td>",
            "              <select>",
            "                <option>",
            "                  \"A\"",
            "              <input>",
            "              \"B\"",
        ]
    );
}

#[test]
fn select_recovery_is_whole_and_chunked_equivalent_across_all_observable_surfaces() {
    for (whole, chunks) in [
        (
            "<!doctype html><select><select><div>x</div>",
            vec!["<!doctype html><sel", "ect><select><div>x", "</div>"],
        ),
        (
            "<!doctype html><select><option>A<option>B<optgroup><option>C",
            vec![
                "<!doctype html><select><option>A",
                "<option>B<opt",
                "group><option>C",
            ],
        ),
        (
            "<!doctype html><select><button>B</button><div>D</div><hr>",
            vec![
                "<!doctype html><select><button>B",
                "</button><div>D</div>",
                "<hr>",
            ],
        ),
        (
            "<table><select><option>A<tr><td>B</td></tr></table>",
            vec![
                "<table><select><option>A",
                "<tr><td>B</td>",
                "</tr></table>",
            ],
        ),
        (
            "<!doctype html><select><option>A<input>B",
            vec!["<!doctype html><select><option>", "A<input>", "B"],
        ),
    ] {
        let whole_run = run_select_chunks(&[whole]);
        let chunked_run = run_select_chunks(&chunks);
        assert_eq!(chunked_run, whole_run, "input={whole}");
        assert!(
            whole_run
                .patches
                .iter()
                .all(|patch| !matches!(patch, DomPatch::RemoveNode { .. })),
            "select stack recovery must not create DOM-repair patches"
        );
    }
}
