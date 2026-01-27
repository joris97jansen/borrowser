use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use html::perf_fixtures::make_blocks;
use html::{Tokenizer, TreeBuilder, build_dom, tokenize};

const SMALL_BLOCKS: usize = 64;
const LARGE_BLOCKS: usize = 20_000;

fn expected_tokens_per_block() -> usize {
    // For "<div class=box><span>hello</span><img src=x></div>":
    // StartTag(div), StartTag(span), Text, EndTag(span), StartTag(img), EndTag(div).
    6
}

fn estimate_token_capacity(blocks: usize) -> usize {
    blocks
        .saturating_mul(expected_tokens_per_block())
        .saturating_add(1)
}

fn make_rawtext_adversarial(bytes: usize) -> String {
    let mut body = String::with_capacity(bytes + 32);
    body.push_str("<script>");
    while body.len() < bytes {
        body.push_str("</scri");
        body.push_str("<");
        body.push_str("pt");
    }
    body.push_str("</script>");
    body
}

fn bench_tokenize_small(c: &mut Criterion) {
    let input = make_blocks(SMALL_BLOCKS);
    c.bench_function("bench_tokenize_small", |b| {
        b.iter(|| {
            let stream = tokenize(black_box(&input));
            black_box(stream.tokens().len());
        });
    });
}

fn bench_tokenize_large(c: &mut Criterion) {
    let input = make_blocks(LARGE_BLOCKS);
    c.bench_function("bench_tokenize_large", |b| {
        b.iter(|| {
            let stream = tokenize(black_box(&input));
            black_box(stream.tokens().len());
        });
    });
}

fn bench_tree_build_large(c: &mut Criterion) {
    let input = make_blocks(LARGE_BLOCKS);
    let stream = tokenize(&input);
    c.bench_function("bench_tree_build_large", |b| {
        b.iter(|| {
            let dom = build_dom(black_box(&stream));
            black_box(dom);
        });
    });
}

fn bench_parse_large_end_to_end(c: &mut Criterion) {
    let input = make_blocks(LARGE_BLOCKS);
    c.bench_function("bench_parse_large_end_to_end", |b| {
        b.iter(|| {
            let stream = tokenize(black_box(&input));
            let dom = build_dom(black_box(&stream));
            black_box(dom);
        });
    });
}

fn bench_streaming_chunked(c: &mut Criterion) {
    let input = make_blocks(LARGE_BLOCKS);
    let bytes = input.as_bytes();
    let chunk_sizes = [1usize, 2, 3, 7, 64, 128, 256, 1024];
    c.bench_function("bench_streaming_chunked", |b| {
        b.iter_batched(
            || {
                (
                    Tokenizer::new(),
                    TreeBuilder::with_capacity(estimate_token_capacity(LARGE_BLOCKS)),
                )
            },
            |(mut tokenizer, mut builder)| {
                let mut tokens = Vec::new();
                let mut offset = 0usize;
                let mut size_idx = 0usize;
                while offset < bytes.len() {
                    let size = chunk_sizes[size_idx % chunk_sizes.len()];
                    let end = (offset + size).min(bytes.len());
                    tokenizer.feed(&bytes[offset..end]);
                    tokenizer.drain_into(&mut tokens);
                    for token in tokens.drain(..) {
                        builder
                            .push_token(&token, tokenizer.atoms(), &tokenizer)
                            .expect("streaming tree builder should accept tokens");
                    }
                    offset = end;
                    size_idx += 1;
                }
                tokenizer.finish();
                tokenizer.drain_into(&mut tokens);
                for token in tokens.drain(..) {
                    builder
                        .push_token(&token, tokenizer.atoms(), &tokenizer)
                        .expect("streaming tree builder should accept tokens");
                }
                builder
                    .finish()
                    .expect("streaming tree builder should finish");
                let dom = builder
                    .into_dom()
                    .expect("streaming tree builder should build");
                black_box(dom);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_tokenize_rawtext_adversarial(c: &mut Criterion) {
    let input = make_rawtext_adversarial(512 * 1024);
    c.bench_function("bench_tokenize_rawtext_adversarial", |b| {
        b.iter(|| {
            let stream = tokenize(black_box(&input));
            black_box(stream.tokens().len());
        });
    });
}

criterion_group!(
    benches,
    bench_tokenize_small,
    bench_tokenize_large,
    bench_tree_build_large,
    bench_parse_large_end_to_end,
    bench_streaming_chunked,
    bench_tokenize_rawtext_adversarial
);
criterion_main!(benches);
