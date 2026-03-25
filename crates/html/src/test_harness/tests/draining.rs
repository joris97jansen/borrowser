use crate::tokenizer::Tokenizer;

#[test]
fn chunked_draining_leaves_no_tokens_behind() {
    let input = "<div>ok</div><!--x-->";
    let bytes = input.as_bytes();
    let sizes = [2, 3, 1];
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::new();
    let mut offset = 0usize;

    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        tokenizer.drain_into(&mut tokens);
        offset = end;
    }
    if offset < bytes.len() {
        tokenizer.feed(&bytes[offset..]);
    }
    tokenizer.finish();
    tokenizer.drain_into(&mut tokens);

    assert!(
        tokenizer.drain_tokens().is_empty(),
        "expected tokenizer to have no buffered tokens after draining"
    );

    let (atoms, source, text_pool) = tokenizer.into_parts();
    let stream = crate::TokenStream::new(tokens, atoms, source, text_pool);
    let expected = crate::tokenize(input);
    assert_eq!(
        crate::test_utils::token_snapshot(&expected),
        crate::test_utils::token_snapshot(&stream),
        "expected drained tokens to match full tokenize() snapshot"
    );
}
