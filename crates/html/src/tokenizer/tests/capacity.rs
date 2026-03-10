use super::super::Tokenizer;

#[test]
fn streaming_does_not_reallocate_internal_tokens_pathologically() {
    let input = "<a></a>".repeat(50_000);
    let mut tokenizer = Tokenizer::new();
    let mut grows = 0usize;
    let mut last_cap = tokenizer.tokens_capacity();

    for b in input.as_bytes() {
        tokenizer.feed(std::slice::from_ref(b));
        let cap = tokenizer.tokens_capacity();
        if cap != last_cap {
            grows += 1;
            last_cap = cap;
        }
    }

    tokenizer.finish();

    assert!(grows <= 32, "too many internal token vec growths: {grows}");
}

#[test]
fn streaming_does_not_reallocate_internal_tokens_with_drains_pathologically() {
    let input = "<a></a>".repeat(50_000);
    let mut tokenizer = Tokenizer::new();
    let mut sink = Vec::new();
    let mut grows = 0usize;
    let mut last_cap = tokenizer.tokens_capacity();

    for b in input.as_bytes() {
        tokenizer.feed(std::slice::from_ref(b));
        tokenizer.drain_into(&mut sink);
        let cap = tokenizer.tokens_capacity();
        if cap != last_cap {
            grows += 1;
            last_cap = cap;
        }
    }

    tokenizer.finish();
    tokenizer.drain_into(&mut sink);

    assert!(
        grows <= 32,
        "too many internal token vec growths with drains: {grows}"
    );
}
