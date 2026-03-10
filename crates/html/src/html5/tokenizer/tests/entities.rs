use super::helpers::{assert_push_ok, drain_all_fmt, run_chunks};
use crate::html5::shared::{DocumentParseContext, Input, TextValue, Token};
use crate::html5::tokenizer::{
    Html5Tokenizer, TextResolveError, TextResolver, TokenFmt, TokenizeResult, TokenizerConfig,
};

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
            "EOF".to_string()
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
            "EOF".to_string()
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
        ) -> Result<&str, TextResolveError> {
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
    assert_eq!(tokens, vec![expected_line, "EOF".to_string()]);
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
