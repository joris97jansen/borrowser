use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use css::{
    ParseOptions, Rule, RuleCollection, SelectorDomIndex, SelectorListParseResult,
    SelectorMatchingContext, SelectorMatchingEnvironment, StyleResolutionLimits,
    StylesheetCollectionInput, StylesheetConditionInput, StylesheetOrder, StylesheetSourceId,
    compute_document_styles, parse_stylesheet_with_options,
    try_resolve_document_styles_from_rule_collection_with_limits,
};

#[path = "../src/perf_fixtures.rs"]
#[allow(dead_code)]
mod perf_fixtures;

const SMALL_RULES: usize = 128;
const LARGE_RULES: usize = 2_048;
const SMALL_BLOCKS: usize = 256;
const LARGE_BLOCKS: usize = 2_048;
const HOST_LANGUAGE_MATCHES_PER_ITERATION: usize = 1_024;

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
            let index = SelectorDomIndex::try_from_document(dom)
                .expect("valid representative selector DOM");
            let context = SelectorMatchingContext::new(
                &index,
                SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
            );

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

fn bench_selector_matching_host_language_comparisons(c: &mut Criterion) {
    const SELECTOR: &str = concat!(
        "DIV#mixed-é.mixed-é",
        "[DISABLED]",
        "[ACCEPT=\"mixed-é\"]",
        "[REL~=\"mixed-é\"]",
        "[LANG|=\"mixed-é\"]",
        "[TYPE^=\"mixed-é\"]",
        "[TARGET$=\"mixed-é\"]",
        "[MEDIA*=\"mixed-é\"]",
        "[data-kind=\"Exact-é\"]",
    );

    let parsed = html::parse_document(
        concat!(
            "<html><body><div ",
            "id=\"MiXeD-é\" class=\"MiXeD-é\" ",
            "disabled ",
            "accept=\"MiXeD-é\" rel=\"left MiXeD-é right\" ",
            "lang=\"MiXeD-é-tail\" type=\"MiXeD-é-tail\" ",
            "target=\"head-MiXeD-é\" media=\"head-MiXeD-é-tail\" ",
            "data-kind=\"Exact-é\"></div></body></html>",
        ),
        html::HtmlParseOptions::default(),
    )
    .expect("host-language benchmark fixture should parse");
    assert_eq!(parsed.document_mode, html::DocumentMode::Quirks);

    let index = SelectorDomIndex::try_from_document(&parsed.document)
        .expect("host-language benchmark fixture should project");
    let context = SelectorMatchingContext::new(
        &index,
        SelectorMatchingEnvironment::new(parsed.document_mode),
    );
    let element = index
        .elements()
        .find(|element| context.element_local_name(*element) == "div")
        .expect("host-language benchmark fixture should contain its target div");

    let stylesheet = parse_stylesheet_with_options(
        &format!("{SELECTOR} {{ color: red; }}"),
        &ParseOptions::stylesheet(),
    );
    assert!(stylesheet.diagnostics.is_empty());
    let Rule::Style(rule) = &stylesheet.stylesheet.rules[0] else {
        panic!("host-language benchmark selector should parse as a style rule");
    };
    let selector = rule
        .selectors
        .parsed()
        .expect("host-language benchmark selector should be supported")
        .selectors()[0]
        .head();
    assert!(context.matches_compound_selector(element, selector));

    let mut group = c.benchmark_group("css_selector_matching_host_language_comparisons");
    group.throughput(Throughput::Elements(
        HOST_LANGUAGE_MATCHES_PER_ITERATION as u64,
    ));
    group.bench_function("quirks_html_compound", |b| {
        b.iter(|| {
            let mut matches = 0usize;
            for _ in 0..HOST_LANGUAGE_MATCHES_PER_ITERATION {
                let matched = black_box(&context)
                    .matches_compound_selector(black_box(element), black_box(selector));
                matches += usize::from(black_box(matched));
            }
            black_box(matches);
        });
    });
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
            let computed = compute_document_styles(
                black_box(&dom),
                SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks),
                black_box(&sheets),
            )
            .expect("style resolution should succeed");
            black_box(computed.entries().len());
        });
    });
}

fn bench_cascade_candidate_density(c: &mut Criterion) {
    const DECLARATIONS: usize = 256;
    let candidate_css = (0..DECLARATIONS)
        .map(|index| format!("color: {};", if index % 2 == 0 { "red" } else { "blue" }))
        .collect::<String>();
    let filtered_css = (0..DECLARATIONS)
        .map(|index| format!("future-{index}: value;"))
        .chain(std::iter::once("color: red;".to_string()))
        .collect::<String>();
    let candidate_sheet = parse_stylesheet_with_options(
        &format!("div {{ {candidate_css} }}"),
        &ParseOptions::stylesheet(),
    );
    let filtered_sheet = parse_stylesheet_with_options(
        &format!("div {{ {filtered_css} }}"),
        &ParseOptions::stylesheet(),
    );
    let limits = StyleResolutionLimits::default();
    let candidate_input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(0),
        StylesheetOrder::new(0),
        &candidate_sheet,
        StylesheetConditionInput::None,
    );
    let filtered_input = StylesheetCollectionInput::author(
        StylesheetSourceId::in_memory_generation_index(0),
        StylesheetOrder::new(0),
        &filtered_sheet,
        StylesheetConditionInput::None,
    );
    let candidate_collection = RuleCollection::try_new(&[candidate_input], &limits).unwrap();
    let filtered_collection = RuleCollection::try_new(&[filtered_input], &limits).unwrap();
    let dom = html::parse_document(
        "<!doctype html><html><body><div></div></body></html>",
        html::HtmlParseOptions::default(),
    )
    .unwrap()
    .document;
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::NoQuirks);
    let mut group = c.benchmark_group("css_cascade_candidate_density");
    for (label, collection) in [
        ("supported", &candidate_collection),
        ("filtered", &filtered_collection),
    ] {
        group.bench_function(label, |b| {
            b.iter(|| {
                let resolved = try_resolve_document_styles_from_rule_collection_with_limits(
                    black_box(&dom),
                    environment,
                    black_box(collection),
                    &limits,
                )
                .unwrap();
                black_box(resolved.entries().len());
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_representative_stylesheet,
    bench_selector_matching_representative_dom,
    bench_selector_matching_host_language_comparisons,
    bench_style_resolution_representative_page,
    bench_cascade_candidate_density
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
