use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use html::perf_fixtures::make_blocks;
use html::{HtmlParseOptions, HtmlParser, parse_document};

const SMALL_BLOCKS: usize = 64;
const LARGE_BLOCKS: usize = 20_000;

fn make_rawtext_adversarial(bytes: usize) -> String {
    let mut body = String::with_capacity(bytes + 32);
    body.push_str("<script>");
    while body.len() < bytes {
        body.push_str("</scri");
        body.push('<');
        body.push_str("pt");
    }
    body.push_str("</script>");
    body
}

fn bench_parse_small(c: &mut Criterion) {
    let input = make_blocks(SMALL_BLOCKS);
    c.bench_function("bench_parse_small", |b| {
        b.iter(|| {
            let output = parse_document(black_box(&input), HtmlParseOptions::default())
                .expect("small html5 parse should succeed");
            black_box(output.counters.tokens_processed);
        });
    });
}

fn bench_parse_large(c: &mut Criterion) {
    let input = make_blocks(LARGE_BLOCKS);
    c.bench_function("bench_parse_large", |b| {
        b.iter(|| {
            let output = parse_document(black_box(&input), HtmlParseOptions::default())
                .expect("large html5 parse should succeed");
            black_box((output.counters.tokens_processed, output.patches.len()));
        });
    });
}

fn bench_parse_rawtext_adversarial(c: &mut Criterion) {
    let input = make_rawtext_adversarial(512 * 1024);
    c.bench_function("bench_parse_rawtext_adversarial", |b| {
        b.iter(|| {
            let output = parse_document(black_box(&input), HtmlParseOptions::default())
                .expect("rawtext html5 parse should succeed");
            black_box((output.counters.tokens_processed, output.patches.len()));
        });
    });
}

fn bench_streaming_chunked(c: &mut Criterion) {
    let input = make_blocks(LARGE_BLOCKS);
    let bytes = input.as_bytes();
    let mut group = c.benchmark_group("bench_streaming_chunked");
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    for chunk_size in [1usize, 2, 3, 7, 64, 128, 256, 1024] {
        group.bench_with_input(
            BenchmarkId::from_parameter(chunk_size),
            &chunk_size,
            |b, &size| {
                b.iter_batched(
                    || HtmlParser::new(HtmlParseOptions::default()).expect("html5 parser init"),
                    |mut parser| {
                        let mut total_patches = 0usize;
                        for chunk in bytes.chunks(size) {
                            parser.push_bytes(chunk).expect("chunk push should succeed");
                            parser.pump().expect("chunk pump should succeed");
                            total_patches = total_patches.saturating_add(
                                parser
                                    .take_patches()
                                    .expect("chunk patch drain should succeed")
                                    .len(),
                            );
                        }
                        parser.finish().expect("streaming finish should succeed");
                        total_patches = total_patches.saturating_add(
                            parser
                                .take_patches()
                                .expect("final patch drain should succeed")
                                .len(),
                        );
                        let output = parser.into_output().expect("output should materialize");
                        black_box((
                            output.counters.tokens_processed,
                            total_patches + output.patches.len(),
                        ));
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_small,
    bench_parse_large,
    bench_parse_rawtext_adversarial,
    bench_streaming_chunked
);
criterion_main!(benches);
