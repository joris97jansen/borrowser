use super::helpers::run_chunks;

#[test]
fn tokenizer_normalizes_crlf_and_lone_cr_in_text_input() {
    let tokens = run_chunks(&["a\r\nb\rc\nd"]);
    assert_eq!(
        tokens,
        vec!["CHAR text=\"a\\nb\\nc\\nd\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn tokenizer_preprocessing_is_chunk_equivalent_for_split_crlf() {
    let whole = run_chunks(&["a\r\nb"]);
    let split = run_chunks(&["a\r", "\nb"]);
    assert_eq!(split, whole);
    assert_eq!(
        split,
        vec!["CHAR text=\"a\\nb\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn tokenizer_flushes_lone_trailing_cr_at_eof_boundary() {
    let tokens = run_chunks(&["a\r"]);
    assert_eq!(
        tokens,
        vec!["CHAR text=\"a\\n\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn tokenizer_empty_input_still_emits_deterministic_eof() {
    let tokens = run_chunks(&[]);
    assert_eq!(tokens, vec!["EOF".to_string()]);
}
