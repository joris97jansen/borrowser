use tools::utf8::{finish_utf8, push_utf8_chunk};

pub(crate) fn reassemble_utf8_bytes_in_chunks(bytes: &[u8], boundaries: &[usize]) -> String {
    let mut text = String::new();
    let mut carry = Vec::new();
    let mut last = 0usize;
    for &idx in boundaries {
        assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
        push_utf8_chunk(&mut text, &mut carry, &bytes[last..idx]);
        last = idx;
    }
    if last < bytes.len() {
        push_utf8_chunk(&mut text, &mut carry, &bytes[last..]);
    }
    finish_utf8(&mut text, &mut carry);
    text
}

#[test]
fn utf8_chunk_assembly_smoke_test() {
    let input = "café 😀";
    let bytes = input.as_bytes();
    let boundaries = vec![1, bytes.len() - 1];
    let rebuilt = reassemble_utf8_bytes_in_chunks(bytes, &boundaries);
    assert_eq!(
        rebuilt, input,
        "expected UTF-8 roundtrip for boundaries={boundaries:?}"
    );
}
