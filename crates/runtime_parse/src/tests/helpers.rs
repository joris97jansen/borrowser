use html::Tokenizer;

pub(crate) fn tokenize_bytes_in_chunks(bytes: &[u8], boundaries: &[usize]) -> String {
    let mut tokenizer = Tokenizer::new();
    let mut last = 0usize;
    for &idx in boundaries {
        assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
        tokenizer.feed(&bytes[last..idx]);
        last = idx;
    }
    if last < bytes.len() {
        tokenizer.feed(&bytes[last..]);
    }
    tokenizer.finish();
    let stream = tokenizer.into_stream();
    let text = stream.iter().find_map(|t| stream.text(t)).unwrap_or("");
    text.to_string()
}

#[test]
fn utf8_chunk_assembly_smoke_test() {
    let input = "café 😀";
    let bytes = input.as_bytes();
    let boundaries = vec![1, bytes.len() - 1];
    let rebuilt = tokenize_bytes_in_chunks(bytes, &boundaries);
    assert_eq!(
        rebuilt, input,
        "expected UTF-8 roundtrip for boundaries={boundaries:?}"
    );
}
