use super::super::UnsupportedSelectorFeature;
use super::support::{parse_selector_result, unsupported_selector};

#[test]
fn parser_reports_unsupported_selector_features_without_string_splitting() {
    let pseudo = parse_selector_result("a:is(.x, .y)");
    let namespace = parse_selector_result("svg|a");
    let case_modifier = parse_selector_result("[lang=\"en\" i]");

    assert_eq!(
        pseudo.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..13\n",
            "feature[0]: functional-pseudo-class\n",
            "feature[1]: forgiving-selector-list\n",
        )
    );
    assert_eq!(
        namespace.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..6\n",
            "feature[0]: namespace\n",
        )
    );
    assert_eq!(
        case_modifier.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..14\n",
            "feature[0]: attribute-case-modifier\n",
        )
    );
}

#[test]
fn unsupported_selectors_do_not_corrupt_surrounding_selector_parsing() {
    let pseudo_tail = unsupported_selector("div:hover.class > span");
    let column = unsupported_selector("div || span");
    let nesting = unsupported_selector("& > main.card");

    assert_eq!(
        pseudo_tail.features(),
        &[UnsupportedSelectorFeature::PseudoClass]
    );
    assert_eq!(
        column.features(),
        &[UnsupportedSelectorFeature::ColumnCombinator]
    );
    assert_eq!(
        nesting.features(),
        &[UnsupportedSelectorFeature::NestingSelector]
    );
}

#[test]
fn unsupported_feature_aggregation_is_stable_across_selector_lists() {
    let result = parse_selector_result(
        "a:is(.x), svg|a, [lang=\"en\" i], div::before, & > span, div || span",
    );

    assert_eq!(
        result.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..67\n",
            "feature[0]: functional-pseudo-class\n",
            "feature[1]: forgiving-selector-list\n",
            "feature[2]: namespace\n",
            "feature[3]: attribute-case-modifier\n",
            "feature[4]: pseudo-element\n",
            "feature[5]: nesting-selector\n",
            "feature[6]: column-combinator\n",
        )
    );
}
