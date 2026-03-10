use super::helpers::{assert_push_ok, drain_all_fmt, run_chunks};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

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
