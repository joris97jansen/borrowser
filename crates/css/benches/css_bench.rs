use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use css::{
    ParseOptions, Rule, SelectorDomIndex, SelectorListParseResult, SelectorMatchingContext,
    compute_document_styles, parse_stylesheet_with_options,
};

#[path = "../src/perf_fixtures.rs"]
#[allow(dead_code)]
mod perf_fixtures;

const SMALL_RULES: usize = 128;
const LARGE_RULES: usize = 2_048;
const SMALL_BLOCKS: usize = 256;
const LARGE_BLOCKS: usize = 2_048;

fn bench_parse_representative_stylesheet(c: &mut Criterion) {
    let mut group = c.benchmark_group("css_parse_representative_stylesheet");

    for rules in [SMALL_RULES, LARGE_RULES] {
        let input = perf_fixtures::representative_stylesheet(rules);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(rules), &input, |b, css| {
            b.iter(|| {
                let parsed =
                    parse_stylesheet_with_options(black_box(css), &ParseOptions::stylesheet());
                black_box((
                    parsed.stats.rules_emitted,
                    parsed.stats.declarations_emitted,
                ));
            });
        });
    }

    group.finish();
}

fn bench_selector_matching_representative_dom(c: &mut Criterion) {
    let selectors = representative_selector_parse();
    let mut group = c.benchmark_group("css_selector_matching_representative_dom");

    for blocks in [SMALL_BLOCKS, LARGE_BLOCKS] {
        let dom = perf_fixtures::representative_dom(blocks);
        group.throughput(Throughput::Elements(
            perf_fixtures::representative_element_count(blocks) as u64,
        ));
        group.bench_with_input(BenchmarkId::from_parameter(blocks), &dom, |b, dom| {
            let index = SelectorDomIndex::from_root(dom);
            let context = SelectorMatchingContext::new(&index);

            b.iter(|| {
                let matches = context
                    .dom()
                    .elements()
                    .filter(|element| {
                        context
                            .match_selector_list(*element, black_box(&selectors))
                            .expect("selector matching should not exceed default limits")
                            .matched_any()
                    })
                    .count();
                black_box(matches);
            });
        });
    }

    group.finish();
}

fn bench_style_resolution_representative_page(c: &mut Criterion) {
    let css = perf_fixtures::representative_stylesheet(LARGE_RULES);
    let dom = perf_fixtures::representative_dom(LARGE_BLOCKS);
    let sheets = vec![parse_stylesheet_with_options(
        &css,
        &ParseOptions::stylesheet(),
    )];

    c.bench_function("css_style_resolution_representative_page", |b| {
        b.iter(|| {
            let computed = compute_document_styles(black_box(&dom), black_box(&sheets))
                .expect("style resolution should succeed");
            black_box(computed.entries().len());
        });
    });
}

criterion_group!(
    benches,
    bench_parse_representative_stylesheet,
    bench_selector_matching_representative_dom,
    bench_style_resolution_representative_page
);
criterion_main!(benches);

fn representative_selector_parse() -> SelectorListParseResult {
    let parse = parse_stylesheet_with_options(
        &perf_fixtures::representative_selector_rule(),
        &ParseOptions::stylesheet(),
    );
    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("representative selector fixture should parse as a style rule");
    };
    rule.selectors.clone()
}
