use crate::{
    ParseOptions, compute_document_styles_from_resolved_styles_with_reuse_stats,
    parse_stylesheet_with_options, perf_fixtures, resolve_document_styles,
};

const HEAVY_RULES: usize = 1_024;
const HEAVY_BLOCKS: usize = 1_024;
const MAX_HEAVY_COMPUTED_MISS_RATIO: f64 = 0.60;

#[test]
fn perf_guard_large_css_style_resolution_remains_bounded_and_reusable() {
    let css = perf_fixtures::representative_stylesheet(HEAVY_RULES);
    let sheets = vec![parse_stylesheet_with_options(
        &css,
        &ParseOptions::stylesheet(),
    )];
    let dom = perf_fixtures::representative_dom(HEAVY_BLOCKS);

    assert!(sheets[0].diagnostics.is_empty());
    assert_eq!(sheets[0].stats.rules_emitted, HEAVY_RULES);
    assert_eq!(
        sheets[0].stats.declarations_emitted,
        HEAVY_RULES * perf_fixtures::declarations_per_generated_rule()
    );

    let resolved =
        resolve_document_styles(&dom, &sheets).expect("heavy style resolution should work");
    let computed = compute_document_styles_from_resolved_styles_with_reuse_stats(&dom, &resolved)
        .expect("heavy computed style materialization should work");
    let entries = perf_fixtures::representative_element_count(HEAVY_BLOCKS);

    assert_eq!(resolved.entries().len(), entries);
    assert_eq!(computed.computed.entries().len(), entries);

    let miss_ratio = computed.reuse_stats.misses as f64 / entries as f64;
    assert!(
        miss_ratio <= MAX_HEAVY_COMPUTED_MISS_RATIO,
        "computed-style reuse miss ratio {miss_ratio:.4} exceeded guard {MAX_HEAVY_COMPUTED_MISS_RATIO}"
    );
}
