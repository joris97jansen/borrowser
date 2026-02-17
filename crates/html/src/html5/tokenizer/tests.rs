use super::{Html5Tokenizer, TokenBatch, TokenFmt, TokenizeResult, TokenizerConfig};
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

    let batch: TokenBatch<'_> = tokenizer.next_batch(&mut input);
    assert!(batch.tokens().is_empty());
    let _ = batch.resolver();
}

#[test]
fn tokenizer_two_chunks_match_single_chunk_sequence() {
    fn run(chunks: &[&str]) -> Vec<String> {
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

    let whole = run(&["<div>Hello</div>"]);
    let chunked = run(&["<div>", "Hello</div>"]);
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
    let patterns = ["</", "<!--"];
    fn run(chunks: &[&str]) -> Vec<String> {
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
    for pattern in patterns {
        let whole = run(&[pattern]);
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
            "DOCTYPE name=doctype html public_id=null system_id=null force_quirks=false"
                .to_string(),
            "CHAR text=\"\\n\"".to_string(),
            "START name=html attrs=[] self_closing=false".to_string(),
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
    fn run(chunks: &[&str]) -> Vec<String> {
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

    let whole = run(&["<div>t</div>"]);
    let split_lt = run(&["<", "div>t</div>"]);
    let split_end = run(&["<div>t<", "/div>"]);
    let split_name = run(&["<di", "v>t</d", "iv>"]);
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
    impl super::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, super::TextResolveError> {
            Ok("")
        }
    }

    let mut ctx = DocumentParseContext::new();
    let tag = ctx.atoms.intern_ascii_folded("div");
    let attr_z = ctx.atoms.intern_ascii_folded("zeta");
    let attr_a = ctx.atoms.intern_ascii_folded("alpha");
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
    impl super::TextResolver for Resolver {
        fn resolve_span(
            &self,
            _span: crate::html5::shared::TextSpan,
        ) -> Result<&str, super::TextResolveError> {
            Ok("hello")
        }
    }

    let span_token = Token::Text {
        text: TextValue::Span(crate::html5::shared::Span::new(0, 0)),
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
