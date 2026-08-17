use super::super::{InvalidSelectorReason, parse_selector_list};
use super::support::{invalid_selector, parse_selector_result, parse_selector_result_with_limits};
use crate::syntax::{
    CssComponentValue, CssFunction, CssInput, CssToken, CssTokenKind, CssTokenText, SyntaxLimits,
};

#[test]
fn parser_reports_invalid_selector_shapes_deterministically() {
    let leading = parse_selector_result("> div");
    let trailing = parse_selector_result("div >");
    let repeated = parse_selector_result("div > + span");

    assert_eq!(
        leading.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..1\n",
            "reason: leading-combinator\n",
        )
    );
    assert_eq!(
        trailing.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @4..5\n",
            "reason: trailing-combinator\n",
        )
    );
    assert_eq!(
        repeated.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @6..7\n",
            "reason: repeated-combinator\n",
        )
    );
}

#[test]
fn parser_rejects_representative_invalid_selector_categories() {
    let empty_input = CssInput::from("");
    let empty_result = parse_selector_list(&empty_input, &[]);
    let Some(empty) = empty_result.invalid() else {
        panic!("expected empty selector list to be invalid");
    };
    let multiple_types = invalid_selector("div*");
    let missing_attribute_name = invalid_selector("[]");
    let missing_attribute_value = invalid_selector("[lang=]");
    let malformed_class = invalid_selector("div.");
    let malformed_pseudo = invalid_selector(":");
    let malformed_double_colon = invalid_selector("::");

    assert_eq!(empty.reason(), InvalidSelectorReason::EmptySelectorList);
    assert_eq!(
        multiple_types.reason(),
        InvalidSelectorReason::MultipleTypeSelectors
    );
    assert_eq!(
        missing_attribute_name.reason(),
        InvalidSelectorReason::MissingAttributeName
    );
    assert_eq!(
        missing_attribute_value.reason(),
        InvalidSelectorReason::MissingAttributeValue
    );
    assert_eq!(
        malformed_class.reason(),
        InvalidSelectorReason::UnexpectedComponentValue
    );
    assert_eq!(
        malformed_pseudo.reason(),
        InvalidSelectorReason::UnexpectedComponentValue
    );
    assert_eq!(
        malformed_double_colon.reason(),
        InvalidSelectorReason::UnexpectedComponentValue
    );
}

#[test]
fn invalid_selector_snapshots_are_stable_for_representative_malformed_inputs() {
    let multiple_types = parse_selector_result("div*");
    let missing_attribute_name = parse_selector_result("[]");
    let missing_attribute_value = parse_selector_result("[lang=]");

    assert_eq!(
        multiple_types.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @3..4\n",
            "reason: multiple-type-selectors\n",
        )
    );
    assert_eq!(
        missing_attribute_name.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..2\n",
            "reason: missing-attribute-name\n",
        )
    );
    assert_eq!(
        missing_attribute_value.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..7\n",
            "reason: missing-attribute-value\n",
        )
    );
}

#[test]
fn parser_applies_selector_list_failure_policy_deterministically() {
    let unsupported = parse_selector_result("div, a:is(.x)");
    let invalid = parse_selector_result("a:is(.x), > span");

    assert_eq!(
        unsupported.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: unsupported\n",
            "span: @0..14\n",
            "feature[0]: functional-pseudo-class\n",
            "feature[1]: forgiving-selector-list\n",
        )
    );
    assert_eq!(
        invalid.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @10..11\n",
            "reason: leading-combinator\n",
        )
    );
}

#[test]
fn parser_rejects_empty_selector_list_segments() {
    let trailing = parse_selector_result("div,");
    let leading = parse_selector_result(",div");
    let repeated = parse_selector_result("div,,span");

    assert_eq!(
        trailing.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @4..5\n",
            "reason: empty-compound-selector\n",
        )
    );
    assert_eq!(
        leading.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..5\n",
            "reason: empty-compound-selector\n",
        )
    );
    assert_eq!(
        repeated.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..10\n",
            "reason: empty-compound-selector\n",
        )
    );
}

#[test]
fn invalid_selector_lists_do_not_partially_recover() {
    let mixed = parse_selector_result("div, [lang=], span");

    assert_eq!(
        mixed.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @5..12\n",
            "reason: missing-attribute-value\n",
        )
    );
}

#[test]
fn parser_reports_resource_limit_selector_list_failures_deterministically() {
    let too_many_selectors = parse_selector_result_with_limits(
        "div, span",
        &SyntaxLimits {
            max_selectors_per_rule: 1,
            ..SyntaxLimits::default()
        },
    );
    let too_many_segments = parse_selector_result_with_limits(
        "main div",
        &SyntaxLimits {
            max_selector_segments_per_selector: 1,
            ..SyntaxLimits::default()
        },
    );
    let too_many_simple_selectors = parse_selector_result_with_limits(
        "div.card",
        &SyntaxLimits {
            max_simple_selectors_per_compound: 1,
            ..SyntaxLimits::default()
        },
    );

    assert_eq!(
        too_many_selectors.invalid().map(|invalid| invalid.reason()),
        Some(InvalidSelectorReason::ResourceLimitExceeded)
    );
    assert_eq!(
        too_many_segments.invalid().map(|invalid| invalid.reason()),
        Some(InvalidSelectorReason::ResourceLimitExceeded)
    );
    assert_eq!(
        too_many_simple_selectors
            .invalid()
            .map(|invalid| invalid.reason()),
        Some(InvalidSelectorReason::ResourceLimitExceeded)
    );
    assert_eq!(
        too_many_selectors.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @0..10\n",
            "reason: resource-limit-exceeded\n",
        )
    );
}

#[test]
fn parser_reports_invariant_violations_for_non_monotonic_selector_spans() {
    let input = CssInput::from("foo.");
    let values = vec![
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Delim('.'),
            input.span(3, 4).expect("dot span"),
        )),
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Ident(CssTokenText::Span(
                input.span(0, 3).expect("ident payload span"),
            )),
            input.span(0, 3).expect("ident span"),
        )),
    ];

    let result = parse_selector_list(&input, &values);
    let Some(invalid) = result.invalid() else {
        panic!("expected non-monotonic selector spans to be invalid");
    };

    assert_eq!(invalid.reason(), InvalidSelectorReason::InvariantViolation);
    assert_eq!(
        result.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "selector-parse\n",
            "result: invalid\n",
            "span: @3..3\n",
            "reason: invariant-violation\n",
        )
    );
}

#[test]
fn pseudo_keyword_text_from_another_input_is_invalid_not_unsupported() {
    let input = CssInput::from(":root");
    let foreign = CssInput::from("root");
    let values = vec![
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Colon,
            input.span(0, 1).expect("colon span"),
        )),
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Ident(CssTokenText::Span(
                foreign.span(0, 4).expect("foreign ident span"),
            )),
            input.span(1, 5).expect("ident token span"),
        )),
    ];

    let result = parse_selector_list(&input, &values);
    let invalid = result
        .invalid()
        .expect("foreign pseudo keyword payload must be invalid");
    assert_eq!(
        invalid.reason(),
        InvalidSelectorReason::UnexpectedComponentValue
    );
}

#[test]
fn pseudo_identifier_span_from_another_input_is_an_invariant_violation() {
    let input = CssInput::from(":root");
    let foreign = CssInput::from("root");
    let values = vec![
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Colon,
            input.span(0, 1).expect("colon span"),
        )),
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Ident(CssTokenText::Owned("root".into())),
            foreign.span(0, 4).expect("foreign ident span"),
        )),
    ];

    assert_pseudo_span_invariant_violation(&input, &values);
}

#[test]
fn non_monotonic_pseudo_identifier_span_is_an_invariant_violation() {
    let input = CssInput::from("abc :");
    let values = vec![
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Colon,
            input.span(4, 5).expect("colon span"),
        )),
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Ident(CssTokenText::Owned("root".into())),
            input.span(0, 3).expect("ident span"),
        )),
    ];

    assert_pseudo_span_invariant_violation(&input, &values);
}

#[test]
fn functional_pseudo_span_from_another_input_is_an_invariant_violation() {
    let input = CssInput::from(":is()");
    let foreign = CssInput::from("is()");
    let values = vec![
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Colon,
            input.span(0, 1).expect("colon span"),
        )),
        CssComponentValue::Function(CssFunction {
            span: foreign.span(0, 4).expect("foreign function span"),
            name: CssTokenText::Owned("is".into()),
            value: Vec::new(),
        }),
    ];

    assert_pseudo_span_invariant_violation(&input, &values);
}

#[test]
fn non_monotonic_functional_pseudo_span_is_an_invariant_violation() {
    let input = CssInput::from("abc :");
    let values = vec![
        CssComponentValue::PreservedToken(CssToken::new(
            CssTokenKind::Colon,
            input.span(4, 5).expect("colon span"),
        )),
        CssComponentValue::Function(CssFunction {
            span: input.span(0, 3).expect("function span"),
            name: CssTokenText::Owned("is".into()),
            value: Vec::new(),
        }),
    ];

    assert_pseudo_span_invariant_violation(&input, &values);
}

fn assert_pseudo_span_invariant_violation(input: &CssInput, values: &[CssComponentValue]) {
    let result = parse_selector_list(input, values);
    let invalid = result
        .invalid()
        .expect("structurally impossible pseudo spans must be invalid");
    assert_eq!(invalid.reason(), InvalidSelectorReason::InvariantViolation);
    assert!(result.unsupported().is_none());
}
