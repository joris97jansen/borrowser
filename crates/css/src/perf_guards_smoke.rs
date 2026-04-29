use crate::{
    ParseOptions, Rule, SelectorDomIndex, SelectorListParseResult, SelectorMatchingContext,
    compute_document_styles, compute_document_styles_from_resolved_styles_with_reuse_stats,
    parse_stylesheet_with_options, perf_fixtures, resolve_document_styles,
};

const SMOKE_RULES: usize = 128;
const SMOKE_BLOCKS: usize = 256;
const MAX_PARSE_BYTES_PER_RULE_SMOKE: usize = 192;
const MAX_STYLE_ENTRIES_PER_RULE_SMOKE: usize = 9;

#[test]
fn perf_guard_css_parse_counts_are_bounded() {
    let css = perf_fixtures::representative_stylesheet(SMOKE_RULES);
    let parsed = parse_stylesheet_with_options(&css, &ParseOptions::stylesheet());

    assert!(
        parsed.diagnostics.is_empty(),
        "representative CSS should parse without diagnostics"
    );
    assert_eq!(parsed.stats.rules_emitted, SMOKE_RULES);
    assert_eq!(
        parsed.stats.declarations_emitted,
        SMOKE_RULES * perf_fixtures::declarations_per_generated_rule()
    );
    assert!(
        parsed.stats.input_bytes <= SMOKE_RULES * MAX_PARSE_BYTES_PER_RULE_SMOKE,
        "representative CSS parse bytes grew unexpectedly: bytes={} rules={} max_per_rule={}",
        parsed.stats.input_bytes,
        SMOKE_RULES,
        MAX_PARSE_BYTES_PER_RULE_SMOKE
    );
}

#[test]
fn perf_guard_selector_matching_work_is_deterministic() {
    let dom = perf_fixtures::representative_dom(SMOKE_BLOCKS);
    let selectors = representative_selector_parse();
    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);

    let first = count_matches(&context, &selectors);
    let second = count_matches(&context, &selectors);

    assert_eq!(first, second, "selector matching drifted across runs");
    assert_eq!(
        first,
        perf_fixtures::expected_representative_selector_matches(SMOKE_BLOCKS),
        "representative selector matched an unexpected number of elements"
    );
}

#[test]
fn perf_guard_style_resolution_counts_and_reuse_are_bounded() {
    let css = perf_fixtures::representative_stylesheet(SMOKE_RULES);
    let sheets = vec![parse_stylesheet_with_options(
        &css,
        &ParseOptions::stylesheet(),
    )];
    let dom = perf_fixtures::representative_dom(SMOKE_BLOCKS);

    let resolved = resolve_document_styles(&dom, &sheets).expect("style resolution should work");
    let computed = compute_document_styles_from_resolved_styles_with_reuse_stats(&dom, &resolved)
        .expect("computed style materialization should work");
    let integrated = compute_document_styles(&dom, &sheets).expect("integrated style pass works");

    let expected_entries = perf_fixtures::representative_element_count(SMOKE_BLOCKS);
    assert_eq!(resolved.entries().len(), expected_entries);
    assert_eq!(computed.computed.entries().len(), expected_entries);
    assert_eq!(integrated.entries().len(), expected_entries);
    assert!(
        expected_entries <= SMOKE_RULES * MAX_STYLE_ENTRIES_PER_RULE_SMOKE,
        "style fixture size changed enough to invalidate smoke guard thresholds"
    );
    assert!(
        computed.reuse_stats.hits > 0,
        "representative computed pass should exercise safe pass-local style reuse"
    );
    assert!(
        computed.reuse_stats.misses < expected_entries,
        "computed-style reuse stopped reducing materialization work: misses={} entries={}",
        computed.reuse_stats.misses,
        expected_entries
    );
}

fn count_matches(
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    selectors: &SelectorListParseResult,
) -> usize {
    context
        .dom()
        .elements()
        .filter(|element| {
            context
                .match_selector_list(*element, selectors)
                .expect("selector matching should not exceed smoke limits")
                .matched_any()
        })
        .count()
}

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
