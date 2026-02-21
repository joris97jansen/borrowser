use super::{
    Html5Tokenizer, MAX_STEPS_PER_PUMP, TextResolver, TokenFmt, TokenizeResult, TokenizerConfig,
};
use crate::html5::shared::{
    Attribute, AttributeValue, DocumentParseContext, Input, TextValue, Token,
};

fn drain_all_fmt(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        let fmt = TokenFmt::new(&ctx.atoms, &resolver);
        for token in batch.iter() {
            out.push(
                fmt.format_token(token)
                    .expect("token formatting in tests must be deterministic"),
            );
        }
    }
    out
}

fn assert_push_ok(res: TokenizeResult) {
    assert!(
        matches!(
            res,
            TokenizeResult::NeedMoreInput | TokenizeResult::Progress
        ),
        "unexpected push_input result: {res:?}"
    );
}

fn run_chunks(chunks: &[&str]) -> Vec<String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();
    for chunk in chunks {
        input.push_str(chunk);
        assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
        out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
    out
}

fn run_chunks_raw_tokens(chunks: &[&str]) -> Vec<Token> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();
    for chunk in chunks {
        input.push_str(chunk);
        assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
        loop {
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                break;
            }
            out.extend(batch.into_tokens());
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        out.extend(batch.into_tokens());
    }
    out
}

#[test]
fn tokenizer_api_compiles() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>Hello</div>");

    let res = tokenizer.push_input(&mut input, &mut ctx);
    assert_push_ok(res);

    // Keep API usage aligned with harnesses: push, then drain-until-empty.
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);

    let batch = tokenizer.next_batch(&mut input);
    assert!(batch.tokens().is_empty());
    let _ = batch.resolver();
}

#[test]
fn tokenizer_two_chunks_match_single_chunk_sequence() {
    let whole = run_chunks(&["<div>Hello</div>"]);
    let chunked = run_chunks(&["<div>", "Hello</div>"]);
    assert_eq!(whole, chunked, "token sequence must be chunk-invariant");
}

#[test]
fn finish_is_idempotent() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);

    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokens, vec!["EOF".to_string()]);
    assert!(drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty());
}

#[test]
#[should_panic(expected = "push_input called after finish")]
fn push_input_after_finish_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    input.push_str("late input");
    let _ = tokenizer.push_input(&mut input, &mut ctx);
}

#[test]
#[should_panic(expected = "finish called with non-final cursor")]
fn finish_with_unconsumed_input_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>");
    let _ = tokenizer.finish(&input);
}

#[test]
#[should_panic(expected = "next_batch input must match the tokenizer-bound Input instance")]
fn next_batch_with_foreign_input_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut owner_input = Input::new();
    owner_input.push_str("<div>");
    let _ = tokenizer.push_input(&mut owner_input, &mut ctx);
    let mut foreign_input = Input::new();
    let _ = tokenizer.next_batch(&mut foreign_input);
}

#[test]
#[should_panic(expected = "finish called with non-final cursor")]
fn finish_with_unconsumed_input_in_comment_family_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    input.push_str("tail-without-pump");
    let _ = tokenizer.finish(&input);
}

#[test]
#[should_panic(expected = "finish called with non-final cursor")]
fn finish_with_unconsumed_input_in_quoted_attribute_value_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=\"x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    input.push_str("tail-without-pump");
    let _ = tokenizer.finish(&input);
}

#[test]
fn tag_open_holds_on_lonely_lt_until_more_input() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<");

    let res = tokenizer.push_input(&mut input, &mut ctx);
    assert!(
        matches!(
            res,
            TokenizeResult::Progress | TokenizeResult::NeedMoreInput
        ),
        "unexpected first push result for lonely '<': {res:?}"
    );
    assert_eq!(tokenizer.cursor, 0, "cursor must stay on '<' while blocked");
    assert_eq!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput,
        "second pump with unchanged input must report NeedMoreInput"
    );
    assert_eq!(tokenizer.cursor, 0, "cursor must remain pinned on '<'");

    input.push_str("x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokens, vec!["EOF".to_string()]);
}

#[test]
fn unterminated_start_tag_at_eof_is_stable() {
    let whole = run_chunks(&["<div"]);
    let split = run_chunks(&["<d", "iv"]);
    assert_eq!(whole, split);
    assert_eq!(whole, vec!["EOF".to_string()]);
}

#[test]
fn delimiter_paths_are_chunk_invariant_and_lossless() {
    fn run(chunks: &[&str]) -> (Vec<String>, usize, usize, TokenizeResult) {
        let mut ctx = DocumentParseContext::new();
        let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
        let mut input = Input::new();
        let mut out = Vec::new();
        for chunk in chunks {
            input.push_str(chunk);
            let res = tokenizer.push_input(&mut input, &mut ctx);
            assert!(
                matches!(
                    res,
                    TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                ),
                "unexpected push result for chunks={chunks:?}: {res:?}"
            );
            out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
        }
        let cursor_before = tokenizer.cursor;
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let cursor_after = tokenizer.cursor;
        assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
        out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
        (out, cursor_before, cursor_after, res)
    }

    for sample in ["<x", "</x", "<!x", "&x"] {
        let whole = run(&[sample]);
        let split = run(&[&sample[..1], &sample[1..]]);
        assert_eq!(
            whole.0, split.0,
            "tokens must be chunk-invariant for '{sample}'"
        );
        assert_eq!(
            whole.3,
            TokenizeResult::NeedMoreInput,
            "whole-input run must settle to NeedMoreInput without extra input for '{sample}'"
        );
        assert_eq!(
            split.3,
            TokenizeResult::NeedMoreInput,
            "chunked run must settle to NeedMoreInput without extra input for '{sample}'"
        );
        assert_eq!(
            whole.1, whole.2,
            "whole-input cursor must not advance when no new input is appended for '{sample}'"
        );
        assert_eq!(
            split.1, split.2,
            "chunked cursor must not advance when no new input is appended for '{sample}'"
        );
    }
}

#[test]
fn partial_markup_prefix_splits_are_resume_safe() {
    let patterns = ["</", "<!", "<!--", "<!DOCTYPE"];
    for pattern in patterns {
        let whole = run_chunks(&[pattern]);
        for split in 1..pattern.len() {
            let mut ctx = DocumentParseContext::new();
            let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
            let mut input = Input::new();

            input.push_str(&pattern[..split]);
            let first = tokenizer.push_input(&mut input, &mut ctx);
            assert!(
                matches!(
                    first,
                    TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                ),
                "unexpected first result for pattern='{pattern}' split={split}: {first:?}"
            );
            let cursor_after_first = tokenizer.cursor;
            assert_eq!(
                tokenizer.push_input(&mut input, &mut ctx),
                TokenizeResult::NeedMoreInput,
                "second pump must block on incomplete prefix pattern='{pattern}' split={split}"
            );
            assert_eq!(
                tokenizer.cursor, cursor_after_first,
                "cursor must stay pinned while awaiting more input pattern='{pattern}' split={split}"
            );

            input.push_str(&pattern[split..]);
            assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
            assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
            let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
            assert_eq!(
                tokens, whole,
                "unexpected token output for pattern='{pattern}' split={split}"
            );
        }
    }
}

#[test]
fn basic_tag_states_emit_expected_tokens() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<DiV>Hello</DIV>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[] self_closing=false".to_string(),
            "CHAR text=\"Hello\"".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn core_v0_attribute_states_parse_expected_forms() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a b=foo c=\"\" d='' e=></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a b=\"foo\" c=\"\" d=\"\" e=\"\"] self_closing=false"
                .to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn duplicate_attributes_are_dropped_first_wins() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=1 a=2 A=3 b=4 b=5></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"1\" b=\"4\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn duplicate_attribute_drop_preserves_other_order() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div z=1 a=1 a=2 y=1></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[z=\"1\" a=\"1\" y=\"1\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn self_closing_start_tag_state_sets_flag() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<input a=\"x\" />");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=input attrs=[a=\"x\"] self_closing=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn tokenizer_self_closing_flag_reflects_syntax_not_voidness() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<br><br/>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=br attrs=[] self_closing=false".to_string(),
            "START name=br attrs=[] self_closing=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn unquoted_attribute_value_terminates_on_invalid_delimiters() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=foo\"bar></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    // Core v0 recovery policy: an invalid delimiter terminates the unquoted
    // value, then remaining bytes continue through attribute parsing.
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"foo\" bar] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn unquoted_invalid_delimiter_split_is_invariant() {
    let whole = run_chunks(&["<div a=foo\"bar></div>"]);
    let split = run_chunks(&["<div a=foo", "\"bar></div>"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec![
            "START name=div attrs=[a=\"foo\" bar] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn unquoted_attribute_value_terminates_on_question_mark() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=foo?bar></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"foo\" bar] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn quoted_attribute_value_split_at_closing_quote_is_invariant() {
    let whole = run_chunks(&["<div a=\"hello\" b=1>"]);
    let split = run_chunks(&["<div a=\"hello", "\" b=1>"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec![
            "START name=div attrs=[a=\"hello\" b=\"1\"] self_closing=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn attribute_chunk_splits_are_invariant_across_boundaries() {
    let sample = "<div a=\"hello\" b='x y' c=foo d e=\"\"/>";
    let whole = run_chunks(&[sample]);
    for split in 1..sample.len() {
        let chunked = run_chunks(&[&sample[..split], &sample[split..]]);
        assert_eq!(
            whole, chunked,
            "attribute token stream must be chunk-invariant for split={split}"
        );
    }
}

#[test]
fn markup_declaration_open_emits_doctype_token() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!doctype html>\n<html>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=false".to_string(),
            "CHAR text=\"\\n\"".to_string(),
            "START name=html attrs=[] self_closing=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_public_and_system_literals_are_parsed() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str(
        "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">",
    );

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=\"-//W3C//DTD HTML 4.01//EN\" system_id=\"http://www.w3.org/TR/html4/strict.dtd\" force_quirks=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn malformed_doctype_without_name_forces_quirks() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!DOCTYPE>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=null public_id=null system_id=null force_quirks=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn malformed_doctype_public_without_quoted_id_forces_quirks() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!DOCTYPE html PUBLIC nope>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_chunk_splits_inside_keyword_and_name_are_invariant() {
    let whole = run_chunks(&["<!DOCTYPE html>"]);
    let split_keyword = run_chunks(&["<!DOC", "TYPE html>"]);
    let split_name = run_chunks(&["<!DOCTYPE h", "tml>"]);
    assert_eq!(whole, split_keyword);
    assert_eq!(whole, split_name);
    assert_eq!(
        whole,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_keyword_mixed_case_chunk_splits_are_invariant() {
    let whole = run_chunks(&["<!DoCtYpE html>"]);
    let split_1 = run_chunks(&["<!DO", "cTyPe html>"]);
    let split_2 = run_chunks(&["<!D", "oCtYpE html>"]);
    assert_eq!(whole, split_1);
    assert_eq!(whole, split_2);
    assert_eq!(
        whole,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_public_system_chunk_splits_inside_ids_and_before_gt_are_invariant() {
    let sample = "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">";
    let whole = run_chunks(&[sample]);
    let split_quote = run_chunks(&[
        "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN",
        "\" \"http://www.w3.org/TR/html4/strict.dtd\">",
    ]);
    let split_before_gt = run_chunks(&[
        "<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\"",
        ">",
    ]);
    assert_eq!(whole, split_quote);
    assert_eq!(whole, split_before_gt);
    assert_eq!(
        whole,
        vec![
            "DOCTYPE name=html public_id=\"-//W3C//DTD HTML 4.01//EN\" system_id=\"http://www.w3.org/TR/html4/strict.dtd\" force_quirks=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_public_id_eof_mid_quote_forces_quirks() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!DOCTYPE html PUBLIC \"x");

    loop {
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_finish_with_unconsumed_buffered_tail_in_doctype_family_forces_quirks() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!DOCTYPE html PUBLIC \"x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));

    // Intentionally append bytes without another pump; finish() in doctype
    // family should accept this Core-v0 shortcut and finalize quirks doctype.
    input.push_str("tail-without-pump");
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_public_keyword_chunk_split_is_invariant() {
    let whole = run_chunks(&["<!DOCTYPE html PUBLIC \"x\" \"y\">"]);
    let split = run_chunks(&["<!DOCTYPE html PUB", "LIC \"x\" \"y\">"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec![
            "DOCTYPE name=html public_id=\"x\" system_id=\"y\" force_quirks=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn doctype_system_keyword_chunk_split_is_invariant() {
    let whole = run_chunks(&["<!DOCTYPE html SYSTEM \"sys\">"]);
    let split = run_chunks(&["<!DOCTYPE html SYS", "TEM \"sys\">"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec![
            "DOCTYPE name=html public_id=null system_id=\"sys\" force_quirks=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn data_text_decodes_minimal_character_references() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("Tom&amp;Jerry &lt;x&gt; &#65; &#x41;");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "CHAR text=\"Tom&Jerry <x> A A\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn data_text_missing_semicolon_entities_remain_literal() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("&amp &#65 &#x41");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "CHAR text=\"&amp &#65 &#x41\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn data_text_numeric_reference_edge_cases_are_deterministic() {
    struct NoResolve;
    impl TextResolver for NoResolve {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, super::TextResolveError> {
            panic!("resolver must not be used for Owned text in this test");
        }
    }

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let raw = "&#0; &#xD800; &#x110000; &#9999999999;";
    input.push_str(raw);

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    let expected_text = crate::entities::decode_entities(raw).into_owned();
    let fmt = TokenFmt::new(&ctx.atoms, &NoResolve);
    let expected_line = fmt
        .format_token(&Token::Text {
            text: TextValue::Owned(expected_text),
        })
        .expect("token fmt should succeed");
    assert_eq!(tokens, vec![expected_line, "EOF".to_string(),]);
}

#[test]
fn data_text_entity_chunk_split_is_invariant() {
    let whole = run_chunks(&["Tom &amp; Jerry"]);
    let split = run_chunks(&["Tom &am", "p; Jerry"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec!["CHAR text=\"Tom & Jerry\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn attribute_values_decode_minimal_character_references() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<p a=\"Tom&amp;Jerry\" b='&#65;' c=&#x41; d='x&amp'></p>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=p attrs=[a=\"Tom&Jerry\" b=\"A\" c=\"A\" d=\"x&amp\"] self_closing=false"
                .to_string(),
            "END name=p".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn attribute_entity_chunk_split_is_invariant() {
    let whole = run_chunks(&["<p a=\"Tom&amp;Jerry\" b=&#x41;></p>"]);
    let split = run_chunks(&["<p a=\"Tom&am", "p;Jerry\" b=&#x4", "1;></p>"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec![
            "START name=p attrs=[a=\"Tom&Jerry\" b=\"A\"] self_closing=false".to_string(),
            "END name=p".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn markup_declaration_open_emits_comment_token() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--x-->tail");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "COMMENT text=\"x\"".to_string(),
            "CHAR text=\"tail\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn comment_entry_split_between_lt_and_bang_is_invariant() {
    let whole = run_chunks(&["<!--x-->"]);
    let split = run_chunks(&["<", "!--x-->"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec!["COMMENT text=\"x\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_entry_split_between_lt_and_bang_preserves_raw_token_kinds() {
    let whole = run_chunks_raw_tokens(&["<!--x-->"]);
    let split = run_chunks_raw_tokens(&["<", "!--x-->"]);
    assert_eq!(whole, split);
    assert!(
        matches!(whole.as_slice(), [Token::Comment { .. }, Token::Eof]),
        "expected comment then EOF, got: {whole:?}"
    );
}

#[test]
fn comment_entry_split_inside_opening_dashes_is_invariant() {
    let whole = run_chunks(&["<!--x-->"]);
    let split = run_chunks(&["<!-", "-x-->"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec!["COMMENT text=\"x\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn markup_declaration_open_malformed_enters_bogus_comment() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!oops>tail");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "COMMENT text=\"oops\"".to_string(),
            "CHAR text=\"tail\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn bogus_comment_emits_on_eof_without_closing_gt() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!oops");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"oops\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_chunk_splits_across_dashes_and_gt_are_invariant() {
    let whole = run_chunks(&["<!--xy-->"]);
    let split_dash = run_chunks(&["<!--xy-", "->"]);
    let split_gt = run_chunks(&["<!--xy--", ">"]);
    let split_three = run_chunks(&["<!--xy", "--", ">"]);
    assert_eq!(whole, split_dash);
    assert_eq!(whole, split_gt);
    assert_eq!(whole, split_three);
    assert_eq!(
        whole,
        vec!["COMMENT text=\"xy\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn malformed_comment_terminator_dash_variants_are_stable() {
    let three_dash = run_chunks(&["<!--a--->"]);
    let four_dash = run_chunks(&["<!--a---->"]);
    let bang_variant = run_chunks(&["<!--a--!>"]);

    assert_eq!(
        three_dash,
        vec!["COMMENT text=\"a-\"".to_string(), "EOF".to_string()]
    );
    assert_eq!(
        four_dash,
        vec!["COMMENT text=\"a--\"".to_string(), "EOF".to_string()]
    );
    assert_eq!(
        bang_variant,
        vec!["COMMENT text=\"a--!>\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_emits_on_eof_without_closing_terminator() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--oops");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"oops\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_emits_on_eof_from_comment_end_dash_state() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--oops-");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"oops-\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn long_comment_processing_is_linearish() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let payload = "x".repeat(20_000);
    let source = format!("<!--{payload}-->");
    let expected_max_steps = (source.len() as u64) * 3;

    let mut input = Input::new();
    input.push_str(&source);
    loop {
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert!(
        tokenizer.stats().steps <= expected_max_steps,
        "comment processing appears non-linear: steps={} expected_max={}",
        tokenizer.stats().steps,
        expected_max_steps
    );
    assert!(
        tokenizer.stats().bytes_consumed <= source.len() as u64,
        "bytes_consumed must never exceed input length: bytes_consumed={} input_len={}",
        tokenizer.stats().bytes_consumed,
        source.len()
    );
    assert!(
        tokenizer.stats().bytes_consumed <= tokenizer.cursor as u64,
        "bytes_consumed must never exceed cursor: bytes_consumed={} cursor={}",
        tokenizer.stats().bytes_consumed,
        tokenizer.cursor
    );
}

#[test]
fn long_doctype_tail_processing_is_linearish() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let public_id = "P".repeat(16_000);
    let system_id = "S".repeat(16_000);
    let source = format!("<!DOCTYPE html PUBLIC \"{public_id}\" \"{system_id}\">");
    let expected_max_steps = (source.len() as u64) * 4;

    let mut input = Input::new();
    input.push_str(&source);
    loop {
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert!(
        tokenizer.stats().steps <= expected_max_steps,
        "doctype tail processing appears non-linear: steps={} expected_max={}",
        tokenizer.stats().steps,
        expected_max_steps
    );
    assert!(
        tokenizer.stats().bytes_consumed <= source.len() as u64,
        "bytes_consumed must never exceed input length: bytes_consumed={} input_len={}",
        tokenizer.stats().bytes_consumed,
        source.len()
    );
}

#[test]
fn steady_state_text_and_comment_tokens_use_spans() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let source = format!(
        "{}<!--{}-->{}",
        "a".repeat(1024),
        "b".repeat(1024),
        "c".repeat(1024)
    );
    input.push_str(&source);

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);

    let mut saw_text = 0usize;
    let mut saw_comment = 0usize;
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        for token in batch.iter() {
            match token {
                Token::Text { text } => {
                    saw_text += 1;
                    assert!(
                        matches!(text, TextValue::Span(_)),
                        "steady-state text should use spans, got {text:?}"
                    );
                }
                Token::Comment { text } => {
                    saw_comment += 1;
                    assert!(
                        matches!(text, TextValue::Span(_)),
                        "steady-state comments should use spans, got {text:?}"
                    );
                }
                _ => {}
            }
        }
    }
    assert!(saw_text >= 2, "expected surrounding text tokens");
    assert_eq!(saw_comment, 1, "expected one comment token");
}

#[test]
fn tokenizer_stats_counters_are_sane_and_observable() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let source = "<!DOCTYPE html><!--x--><div a=1>b</div>";
    input.push_str(source);

    loop {
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);

    let stats = tokenizer.stats();
    assert!(stats.steps > 0, "steps counter should be non-zero");
    assert!(
        stats.state_transitions > 0,
        "state_transitions counter should be non-zero"
    );
    assert!(
        stats.tokens_emitted >= 5,
        "tokens_emitted should include doctype/comment/tag/text/eof, got {}",
        stats.tokens_emitted
    );
    assert!(
        stats.bytes_consumed <= source.len() as u64,
        "bytes_consumed must not exceed input length: {} > {}",
        stats.bytes_consumed,
        source.len()
    );
}

#[test]
fn large_mixed_input_smoke_completes_and_emits_eof() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();

    let unit = "<!DOCTYPE html><div a=1>hello<!--x-->world</div>";
    let source = unit.repeat(5_000);
    input.push_str(&source);

    let mut pump_count = 0usize;
    loop {
        pump_count += 1;
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(res, TokenizeResult::NeedMoreInput) {
            break;
        }
        assert!(
            pump_count < 100_000,
            "large-input smoke exceeded pump safety budget"
        );
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tail = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert!(
        tail.last().map(String::as_str) == Some("EOF"),
        "expected final EOF in large-input smoke output"
    );
    assert!(
        tokenizer.stats().steps <= (source.len() as u64) * 12,
        "large-input smoke appears non-linear: steps={} input_len={}",
        tokenizer.stats().steps,
        source.len()
    );
    assert!(
        tokenizer.stats().bytes_consumed == source.len() as u64,
        "large-input smoke must consume full input: consumed={} input_len={}",
        tokenizer.stats().bytes_consumed,
        source.len()
    );
    let expected_max_budget_exhaustions =
        (source.len() as u64 / MAX_STEPS_PER_PUMP as u64).saturating_add(8);
    assert!(
        tokenizer.stats().budget_exhaustions <= expected_max_budget_exhaustions,
        "budget exhaustions too high for large-input smoke: got={} bound={}",
        tokenizer.stats().budget_exhaustions,
        expected_max_budget_exhaustions
    );
}

#[test]
fn data_flushes_text_before_tag_in_same_pump() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("Hello<div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    let first_batch = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        first_batch,
        vec![
            "CHAR text=\"Hello\"".to_string(),
            "START name=div attrs=[] self_closing=false".to_string(),
        ]
    );
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tail = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tail, vec!["EOF".to_string()]);
}

#[test]
fn tag_state_chunk_splits_inside_lt_slash_and_name_are_invariant() {
    let whole = run_chunks(&["<div>t</div>"]);
    let split_lt = run_chunks(&["<", "div>t</div>"]);
    let split_end = run_chunks(&["<div>t<", "/div>"]);
    let split_name = run_chunks(&["<di", "v>t</d", "iv>"]);
    assert_eq!(whole, split_lt);
    assert_eq!(whole, split_end);
    assert_eq!(whole, split_name);
}

#[test]
fn end_tag_open_non_alpha_reconsumes_current_char_without_loss() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("</🙂>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "CHAR text=\"</\"".to_string(),
            "CHAR text=\"🙂>\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn token_fmt_is_deterministic_and_preserves_attribute_order() {
    struct Resolver;
    impl TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, super::TextResolveError> {
            Ok("")
        }
    }

    let mut ctx = DocumentParseContext::new();
    let tag = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");
    let attr_z = ctx
        .atoms
        .intern_ascii_folded("zeta")
        .expect("atom interning");
    let attr_a = ctx
        .atoms
        .intern_ascii_folded("alpha")
        .expect("atom interning");
    let token = Token::StartTag {
        name: tag,
        attrs: vec![
            Attribute {
                name: attr_z,
                value: Some(AttributeValue::Owned("1".to_string())),
            },
            Attribute {
                name: attr_a,
                value: Some(AttributeValue::Owned("2".to_string())),
            },
        ],
        self_closing: false,
    };

    let fmt = TokenFmt::new(&ctx.atoms, &Resolver);
    let first = fmt.format_token(&token).expect("token fmt should succeed");
    let second = fmt.format_token(&token).expect("token fmt should succeed");
    assert_eq!(first, second);
    assert_eq!(
        first,
        "START name=div attrs=[zeta=\"1\" alpha=\"2\"] self_closing=false"
    );
}

#[test]
fn token_fmt_text_is_storage_model_agnostic() {
    struct Resolver;
    impl TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, super::TextResolveError> {
            Ok("hello")
        }
    }

    let span_token = Token::Text {
        text: TextValue::Span(crate::html5::shared::TextSpan::new(0, 0)),
    };
    let owned_token = Token::Text {
        text: TextValue::Owned("hello".to_string()),
    };

    let ctx = DocumentParseContext::new();
    let fmt = TokenFmt::new(&ctx.atoms, &Resolver);
    let span_rendered = fmt
        .format_token(&span_token)
        .expect("span text token fmt should succeed");
    let owned_rendered = fmt
        .format_token(&owned_token)
        .expect("owned text token fmt should succeed");
    assert_eq!(span_rendered, owned_rendered);
    assert_eq!(span_rendered, "CHAR text=\"hello\"");
}

#[test]
fn resolver_rejects_invalid_span() {
    struct Resolver<'a>(&'a str);
    impl<'a> TextResolver for Resolver<'a> {
        fn resolve_span(
            &self,
            span: crate::html5::shared::TextSpan,
        ) -> Result<&str, super::TextResolveError> {
            let text = self.0;
            if !(span.start <= span.end
                && span.end <= text.len()
                && text.is_char_boundary(span.start)
                && text.is_char_boundary(span.end))
            {
                return Err(super::TextResolveError::InvalidSpan { span });
            }
            Ok(&text[span.start..span.end])
        }
    }
    let resolver = Resolver("hi");
    let err = resolver
        .resolve_span(crate::html5::shared::TextSpan::new(0, 999))
        .expect_err("resolver must reject out-of-bounds span");
    assert!(matches!(err, super::TextResolveError::InvalidSpan { .. }));
}

#[test]
#[should_panic(expected = "tokenizer atom table mismatch")]
fn tokenizer_rejects_foreign_atom_table_context() {
    let mut owner_ctx = DocumentParseContext::new();
    let mut foreign_ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut owner_ctx);
    let mut input = Input::new();
    input.push_str("<div>");
    let _ = tokenizer.push_input(&mut input, &mut foreign_ctx);
}
